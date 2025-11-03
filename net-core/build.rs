use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());

    let mut dir = PathBuf::from(&manifest_dir);
    let git_dir = loop {
        if dir.join(".git").exists() {
            break dir.join(".git");
        }
        if !dir.pop() {
            break PathBuf::from(".git");
        }
    };

    if git_dir.exists() {
        println!("cargo:rerun-if-changed={}", git_dir.join("HEAD").display());
        println!(
            "cargo:rerun-if-changed={}",
            git_dir.join("refs/heads").display()
        );
        println!("cargo:rerun-if-changed={}", git_dir.join("index").display());
    }

    let build_time_unix = env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
        });
    println!("cargo:rustc-env=BUILD_TIME_UNIX={}", build_time_unix);

    let rustc_bin = env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    if let Ok(out) = Command::new(rustc_bin).arg("--version").output() {
        if out.status.success() {
            if let Ok(ver) = String::from_utf8(out.stdout) {
                println!("cargo:rustc-env=RUSTC_VERSION={}", ver.trim());
            }
        }
    }

    let git_hash = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout).ok()
            } else {
                None
            }
        })
        .map(|s| s.trim().to_string());

    let dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false);

    if let Some(mut h) = git_hash {
        if dirty {
            h.push_str("-dirty");
        }
        println!("cargo:rustc-env=GIT_HASH={}", h);
    }

    if let Ok(profile) = env::var("PROFILE") {
        println!("cargo:rustc-env=PROFILE={}", profile);
    }
}
