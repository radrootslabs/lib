use std::{fs, path::Path};

fn main() {
    println!("cargo:rustc-env=TS_RS_EXPORT_DIR=./bindings/ts/src");

    let out_dir = Path::new("bindings/ts/src");
    if !out_dir.exists() {
        fs::create_dir_all(out_dir).expect("Failed to create TS export directory");
    }

    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=build.rs");
}
