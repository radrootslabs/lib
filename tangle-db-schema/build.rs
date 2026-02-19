use std::{env, fs, path::PathBuf};

fn export_dir(crate_name: &str) -> PathBuf {
    if let Some(export_dir) = env::var_os("RADROOTS_TS_RS_EXPORT_DIR") {
        return PathBuf::from(export_dir);
    }
    PathBuf::from(format!("../target/ts-rs/{crate_name}"))
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var_os("CARGO_FEATURE_TS_RS").is_some() {
        let out_dir = export_dir("tangle-db-schema");
        println!("cargo:rustc-env=TS_RS_EXPORT_DIR={}", out_dir.display());
        println!("cargo:rerun-if-env-changed=RADROOTS_TS_RS_EXPORT_DIR");
        if !out_dir.exists() {
            fs::create_dir_all(&out_dir).expect("create TS export dir");
        }
        println!("cargo:rerun-if-changed=src");
    }
}
