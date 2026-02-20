use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn workspace_root(manifest_dir: &Path) -> PathBuf {
    let parent = manifest_dir.parent().unwrap_or(manifest_dir);
    if parent.file_name().and_then(|name| name.to_str()) == Some("crates") {
        parent.parent().unwrap_or(parent).to_path_buf()
    } else {
        parent.to_path_buf()
    }
}

fn export_dir(crate_name: &str) -> PathBuf {
    if let Some(export_dir) = env::var_os("RADROOTS_TS_RS_EXPORT_DIR") {
        return PathBuf::from(export_dir);
    }
    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("missing required env var CARGO_MANIFEST_DIR"),
    );
    workspace_root(&manifest_dir)
        .join("target")
        .join("ts-rs")
        .join(crate_name)
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
