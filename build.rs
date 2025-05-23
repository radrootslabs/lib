use std::process::{Command, Stdio};

fn main() {
    let status_typeshare = Command::new("typeshare")
        .args(["--lang", "typescript",  "--output-file", "bindings/ts/types.ts", "src"])
        .status()
        .expect("failed to run typeshare");

    if !status_typeshare.success() {
        panic!("typeshare generation failed");
    }

 let status_ts_build = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir("bindings/ts")
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("failed to run `npm run build`");

    if !status_ts_build.success() {
        panic!("typescript bindings build failed");
    }

    println!("cargo:rerun-if-changed=src/");
}
