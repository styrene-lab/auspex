//! auspex-keygen — derive SSH keys from a StyreneIdentity root secret.
//!
//! Usage:
//!   auspex-keygen init                     # Generate new root, save to ~/.styrene/identity
//!   auspex-keygen ssh <label>              # Derive SSH keypair for <label>
//!   auspex-keygen ssh <label> --export     # Write private key to ~/.ssh/styrene-<label>
//!   auspex-keygen pubkey                   # Show root identity public key

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use ed25519_dalek::SigningKey;
use rand_core::RngCore;
use ssh_key::private::{Ed25519Keypair, KeypairData};
use ssh_key::public::Ed25519PublicKey;
use styrene_identity::{KeyDeriver, KeyPurpose, pubkey};
use zeroize::Zeroize;

fn identity_dir() -> PathBuf {
    let home = std::env::var("HOME").expect("HOME not set");
    PathBuf::from(home).join(".styrene").join("identity")
}

fn identity_path() -> PathBuf {
    identity_dir().join("root-secret")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage:");
        eprintln!("  auspex-keygen init                  Generate new root identity");
        eprintln!("  auspex-keygen ssh <label>           Show SSH public key for label");
        eprintln!("  auspex-keygen ssh <label> --export  Write SSH keypair to ~/.ssh/");
        eprintln!("  auspex-keygen pubkey                Show root Ed25519 public key");
        std::process::exit(1);
    }

    match args[1].as_str() {
        "init" => cmd_init(),
        "ssh" => {
            if args.len() < 3 {
                eprintln!("Usage: auspex-keygen ssh <label> [--export]");
                std::process::exit(1);
            }
            let export = args.get(3).is_some_and(|a| a == "--export");
            cmd_ssh(&args[2], export);
        }
        "pubkey" => cmd_pubkey(),
        other => {
            eprintln!("Unknown command: {other}");
            std::process::exit(1);
        }
    }
}

fn cmd_init() {
    let path = identity_path();
    if path.exists() {
        eprintln!("Identity already exists at {}", path.display());
        eprintln!("Remove it first if you want to regenerate.");
        std::process::exit(1);
    }

    // Generate 32 bytes of cryptographic randomness.
    let mut root = [0u8; 32];
    rand_core::OsRng.fill_bytes(&mut root);

    // Write atomically.
    let dir = identity_dir();
    fs::create_dir_all(&dir).expect("could not create identity directory");

    // Restrictive permissions (owner-only).
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&path)
        .expect("could not create identity file");
    file.write_all(&root).expect("could not write root secret");
    root.zeroize();

    eprintln!("Identity created at {}", path.display());
    eprintln!("Back this up securely. Loss = loss of all derived keys.");

    // Show the public key.
    cmd_pubkey();
}

fn load_root() -> [u8; 32] {
    let path = identity_path();
    let bytes = fs::read(&path).unwrap_or_else(|e| {
        eprintln!("Could not read {}: {e}", path.display());
        eprintln!("Run 'auspex-keygen init' first.");
        std::process::exit(1);
    });
    bytes.try_into().unwrap_or_else(|_| {
        eprintln!("Root secret must be exactly 32 bytes");
        std::process::exit(1);
    })
}

fn cmd_pubkey() {
    let mut root = load_root();
    let deriver = KeyDeriver::new(&root);
    root.zeroize();

    let signing_seed = deriver.derive(KeyPurpose::Signing);
    let pubkey = pubkey::ed25519_verifying_key(&signing_seed);

    let hex: String = pubkey
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    println!("StyreneID (Ed25519): {hex}");
}

fn cmd_ssh(label: &str, export: bool) {
    let mut root = load_root();
    let deriver = KeyDeriver::new(&root);
    root.zeroize();

    let mut seed = deriver.derive_ssh_user_key(label).unwrap_or_else(|e| {
        eprintln!("Derivation failed: {e}");
        std::process::exit(1);
    });

    // Build Ed25519 keypair from the derived seed.
    let signing_key = SigningKey::from_bytes(&seed);
    let verifying_key = signing_key.verifying_key();

    // Format as OpenSSH public key.
    let ssh_pubkey = ssh_key::public::KeyData::Ed25519(Ed25519PublicKey(verifying_key.to_bytes()));
    let comment = format!("styrene-{label}");
    let public_key = ssh_key::PublicKey::new(ssh_pubkey, &comment);
    let pubkey_str = public_key
        .to_openssh()
        .expect("could not format public key");

    println!("{pubkey_str}");

    if export {
        // Build OpenSSH private key.
        let keypair = Ed25519Keypair {
            public: Ed25519PublicKey(verifying_key.to_bytes()),
            private: ssh_key::private::Ed25519PrivateKey::from_bytes(&seed),
        };
        seed.zeroize();

        let private_key = ssh_key::PrivateKey::new(KeypairData::Ed25519(keypair), &comment)
            .expect("could not build private key");

        let private_openssh = private_key
            .to_openssh(ssh_key::LineEnding::LF)
            .expect("could not serialize private key");

        let home = std::env::var("HOME").expect("HOME not set");
        let priv_path = PathBuf::from(&home)
            .join(".ssh")
            .join(format!("styrene-{label}"));
        let pub_path = PathBuf::from(&home)
            .join(".ssh")
            .join(format!("styrene-{label}.pub"));

        // Write private key (mode 600).
        {
            let mut f = fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .mode(0o600)
                .open(&priv_path)
                .expect("could not write private key");
            f.write_all(private_openssh.as_bytes())
                .expect("could not write private key");
        }

        // Write public key.
        fs::write(&pub_path, pubkey_str.as_bytes()).expect("could not write public key");

        eprintln!("Private key: {}", priv_path.display());
        eprintln!("Public key:  {}", pub_path.display());
        eprintln!();
        eprintln!("Add to remote authorized_keys:");
        eprintln!("  {pubkey_str}");
    } else {
        seed.zeroize();
        eprintln!();
        eprintln!("To export the private key: auspex-keygen ssh {label} --export");
    }
}

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

#[cfg(not(unix))]
trait OpenOptionsExt {
    fn mode(&mut self, _mode: u32) -> &mut Self;
}

#[cfg(not(unix))]
impl OpenOptionsExt for fs::OpenOptions {
    fn mode(&mut self, _mode: u32) -> &mut Self {
        self
    }
}
