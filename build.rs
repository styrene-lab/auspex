use std::{env, process::Command};

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?;
    let s = s.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn main() {
    let sha = env::var("AUSPEX_GIT_SHA")
        .ok()
        .or_else(|| git(&["rev-parse", "--short=8", "HEAD"]))
        .unwrap_or_else(|| "unknown".into());
    let dirty = env::var("AUSPEX_GIT_DIRTY").unwrap_or_else(|_| {
        git(&["status", "--porcelain"])
            .filter(|status| !status.is_empty())
            .map(|_| "+dirty".into())
            .unwrap_or_default()
    });
    let build_time = env::var("AUSPEX_BUILD_TIME")
        .ok()
        .or_else(|| {
            env::var("SOURCE_DATE_EPOCH")
                .ok()
                .map(|epoch| format!("unix:{epoch}"))
        })
        .unwrap_or_else(|| "unknown".into());

    println!("cargo:rustc-env=AUSPEX_GIT_SHA={sha}");
    println!("cargo:rustc-env=AUSPEX_GIT_DIRTY={}", dirty);
    println!("cargo:rustc-env=AUSPEX_BUILD_TIME={build_time}");

    // Re-stamp when the git head moves.
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");
    println!("cargo:rerun-if-env-changed=AUSPEX_GIT_SHA");
    println!("cargo:rerun-if-env-changed=AUSPEX_GIT_DIRTY");
    println!("cargo:rerun-if-env-changed=AUSPEX_BUILD_TIME");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
}
