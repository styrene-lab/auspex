//! Local control-plane TLS configuration shared by HTTP and WebSocket clients.

use std::fs::File;
use std::io::{self, BufReader};
use std::path::PathBuf;
use std::sync::Arc;

pub const CONTROL_TLS_CERT_ENV: &str = "AUSPEX_OMEGON_CONTROL_TLS_CERT";
pub const CONTROL_TLS_KEY_ENV: &str = "AUSPEX_OMEGON_CONTROL_TLS_KEY";
pub const CONTROL_TLS_CLIENT_CA_ENV: &str = "AUSPEX_OMEGON_CONTROL_TLS_CLIENT_CA";
pub const CONTROL_TLS_CLIENT_CERT_ENV: &str = "AUSPEX_OMEGON_CONTROL_TLS_CLIENT_CERT";
pub const CONTROL_TLS_CLIENT_KEY_ENV: &str = "AUSPEX_OMEGON_CONTROL_TLS_CLIENT_KEY";
pub const CONTROL_TLS_ROOT_ENV: &str = "AUSPEX_OMEGON_CONTROL_TLS_ROOT";

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn omegon_control_tls_args_from_env() -> Result<Vec<String>, String> {
    let cert = non_empty_env(CONTROL_TLS_CERT_ENV);
    let key = non_empty_env(CONTROL_TLS_KEY_ENV);
    let client_ca = non_empty_env(CONTROL_TLS_CLIENT_CA_ENV);

    let mut args = Vec::new();
    match (cert, key) {
        (None, None) => {}
        (Some(cert), Some(key)) => {
            args.push("--control-tls-cert".into());
            args.push(cert);
            args.push("--control-tls-key".into());
            args.push(key);
        }
        _ => {
            return Err(format!(
                "{CONTROL_TLS_CERT_ENV} and {CONTROL_TLS_KEY_ENV} must be set together"
            ));
        }
    }

    if let Some(client_ca) = client_ca {
        if args.is_empty() {
            return Err(format!(
                "{CONTROL_TLS_CLIENT_CA_ENV} requires {CONTROL_TLS_CERT_ENV} and {CONTROL_TLS_KEY_ENV}"
            ));
        }
        args.push("--control-tls-client-ca".into());
        args.push(client_ca);
    }

    Ok(args)
}

fn configured_root_path() -> Option<PathBuf> {
    non_empty_env(CONTROL_TLS_ROOT_ENV)
        .or_else(|| non_empty_env(CONTROL_TLS_CLIENT_CA_ENV))
        .or_else(|| non_empty_env(CONTROL_TLS_CERT_ENV))
        .map(PathBuf::from)
}

pub fn apply_reqwest_roots(
    mut builder: reqwest::ClientBuilder,
) -> Result<reqwest::ClientBuilder, String> {
    let Some(path) = configured_root_path() else {
        return Ok(builder);
    };
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("could not read TLS root {}: {error}", path.display()))?;
    let certificate = reqwest::Certificate::from_pem(&bytes)
        .map_err(|error| format!("invalid TLS root PEM {}: {error}", path.display()))?;
    builder = builder.add_root_certificate(certificate);
    if let Some(identity) = reqwest_identity_from_env()? {
        builder = builder.identity(identity);
    }
    Ok(builder)
}

pub fn websocket_connector_from_env() -> Result<Option<tokio_tungstenite::Connector>, String> {
    let Some(path) = configured_root_path() else {
        return Ok(None);
    };

    let roots = load_root_store(&path).map_err(|error| {
        format!(
            "could not load WebSocket TLS root {}: {error}",
            path.display()
        )
    })?;
    let builder = rustls::ClientConfig::builder().with_root_certificates(roots);
    let config = match client_cert_from_env()? {
        Some((certs, key)) => builder
            .with_client_auth_cert(certs, key)
            .map_err(|error| format!("invalid WebSocket client TLS identity: {error}"))?,
        None => builder.with_no_client_auth(),
    };
    Ok(Some(tokio_tungstenite::Connector::Rustls(Arc::new(config))))
}

fn reqwest_identity_from_env() -> Result<Option<reqwest::Identity>, String> {
    let (cert, key) = match (
        non_empty_env(CONTROL_TLS_CLIENT_CERT_ENV),
        non_empty_env(CONTROL_TLS_CLIENT_KEY_ENV),
    ) {
        (None, None) => return Ok(None),
        (Some(cert), Some(key)) => (cert, key),
        _ => {
            return Err(format!(
                "{CONTROL_TLS_CLIENT_CERT_ENV} and {CONTROL_TLS_CLIENT_KEY_ENV} must be set together"
            ));
        }
    };

    let mut pem = std::fs::read(&cert)
        .map_err(|error| format!("could not read TLS client cert {cert}: {error}"))?;
    pem.extend_from_slice(b"\n");
    pem.extend(
        std::fs::read(&key)
            .map_err(|error| format!("could not read TLS client key {key}: {error}"))?,
    );
    reqwest::Identity::from_pem(&pem)
        .map(Some)
        .map_err(|error| format!("invalid TLS client identity PEM: {error}"))
}

fn load_root_store(path: &PathBuf) -> io::Result<rustls::RootCertStore> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let certificates = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse PEM certs: {error}"),
            )
        })?;
    if certificates.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "no certificates found in PEM bundle",
        ));
    }

    let mut roots = rustls::RootCertStore::empty();
    let (added, _ignored) = roots.add_parsable_certificates(certificates);
    if added == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "no valid root certificates found in PEM bundle",
        ));
    }

    Ok(roots)
}

fn client_cert_from_env() -> Result<
    Option<(
        Vec<rustls::pki_types::CertificateDer<'static>>,
        rustls::pki_types::PrivateKeyDer<'static>,
    )>,
    String,
> {
    let (cert, key) = match (
        non_empty_env(CONTROL_TLS_CLIENT_CERT_ENV),
        non_empty_env(CONTROL_TLS_CLIENT_KEY_ENV),
    ) {
        (None, None) => return Ok(None),
        (Some(cert), Some(key)) => (cert, key),
        _ => {
            return Err(format!(
                "{CONTROL_TLS_CLIENT_CERT_ENV} and {CONTROL_TLS_CLIENT_KEY_ENV} must be set together"
            ));
        }
    };

    let certs = load_cert_chain(&PathBuf::from(&cert))
        .map_err(|error| format!("could not load TLS client cert {cert}: {error}"))?;
    let key = load_private_key(&PathBuf::from(&key))
        .map_err(|error| format!("could not load TLS client key {key}: {error}"))?;
    Ok(Some((certs, key)))
}

fn load_cert_chain(path: &PathBuf) -> io::Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let certificates = rustls_pemfile::certs(&mut reader)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse PEM certs: {error}"),
            )
        })?;
    if certificates.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "no certificates found in PEM bundle",
        ));
    }
    Ok(certificates)
}

fn load_private_key(path: &PathBuf) -> io::Result<rustls::pki_types::PrivateKeyDer<'static>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    rustls_pemfile::private_key(&mut reader)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse private key: {error}"),
            )
        })?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no private key found"))
}
