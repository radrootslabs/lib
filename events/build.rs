use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if env::var_os("CARGO_FEATURE_TS_RS").is_some() {
        println!("cargo:rustc-env=TS_RS_EXPORT_DIR=./bindings/ts/src");
        let out_dir = Path::new("bindings/ts/src");
        if !out_dir.exists() {
            fs::create_dir_all(out_dir).expect("create TS export dir");
        }
        println!("cargo:rerun-if-changed=src");
    }
}
