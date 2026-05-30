fn main() {
    let lock = std::fs::read_to_string("../Cargo.lock").unwrap_or_default();
    let mut in_omegon_traits = false;
    let mut version = String::new();
    for line in lock.lines() {
        if line == "[[package]]" {
            in_omegon_traits = false;
            continue;
        }
        if line == "name = \"omegon-traits\"" {
            in_omegon_traits = true;
            continue;
        }
        if in_omegon_traits && line.starts_with("version = ") && !line.contains("0.20.0") {
            version = line
                .trim_start_matches("version = ")
                .trim_matches('"')
                .to_string();
            break;
        }
    }
    if !version.is_empty() {
        println!("cargo:rustc-env=AUSPEX_LINKED_OMEGON_TRAITS_VERSION={version}");
    }
    println!("cargo:rerun-if-changed=../Cargo.lock");
}
