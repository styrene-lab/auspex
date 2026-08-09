use std::path::PathBuf;

fn main() {
    let mut args = std::env::args_os();
    let program = args.next().unwrap_or_default();
    let Some(binary) = args.next() else {
        eprintln!(
            "usage: {} <runtime/omegon-headless>",
            PathBuf::from(program).display()
        );
        std::process::exit(2);
    };
    if args.next().is_some() {
        eprintln!("expected exactly one sidecar path");
        std::process::exit(2);
    }
    let binary = PathBuf::from(binary);
    match auspex_core::bootstrap::verify_packaged_omegon(&binary) {
        Ok(()) => println!("verified {}", binary.display()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
