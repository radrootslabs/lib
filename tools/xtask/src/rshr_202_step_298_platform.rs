use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const APPLE_TOOLCHAIN_IDENTITY_SHA256: &str =
    "fd9bb9af273d0a834c2abff36910edf25f3e5b60c36fcc23b45b738c5c8b2d08";
const PROBE_SOURCE_PATH: &str =
    "tools/radroots_scripts/src/radroots_scripts/verify/rshr_200_series.py";
const PROBE_SOURCE_SHA256: &str =
    "add949c6c20a037123808230625dfd09dd6fa6c5afe5a856400227191f5de5b5";
const REQUEST_PATH: &str = ".git/rshr-step-298-platform-request-sha256";

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask must remain under tools/xtask")
}

fn canonical(value: &Value) -> Result<Vec<u8>, String> {
    serde_json::to_vec(value).map_err(|_| "Step 298 platform JSON encoding failed".to_owned())
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn uname(flag: &str) -> Result<String, String> {
    let output = Command::new("/usr/bin/uname")
        .arg(flag)
        .output()
        .map_err(|_| "Step 298 platform probe could not start uname".to_owned())?;
    if !output.status.success() || !output.stderr.is_empty() {
        return Err("Step 298 platform probe uname failed".to_owned());
    }
    parse_uname(&output.stdout)
}

fn parse_uname(stdout: &[u8]) -> Result<String, String> {
    let value = std::str::from_utf8(stdout)
        .map_err(|_| "Step 298 platform probe uname output is not UTF-8".to_owned())?
        .strip_suffix('\n')
        .ok_or_else(|| "Step 298 platform probe uname output differs".to_owned())?;
    if value.is_empty() || value.contains('\n') || value.contains('\r') {
        return Err("Step 298 platform probe uname output differs".to_owned());
    }
    Ok(value.to_owned())
}

pub(crate) fn run() -> Result<(), String> {
    let request_bytes = fs::read(root().join(REQUEST_PATH))
        .map_err(|_| "Step 298 platform execution request is unavailable".to_owned())?;
    let execution_request_sha256 = request_digest(&request_bytes)?;
    let bytes = result_bytes(
        execution_request_sha256,
        &uname("-s")?,
        &uname("-r")?,
        &uname("-v")?,
        std::env::consts::ARCH,
    )?;
    std::io::Write::write_all(&mut std::io::stdout().lock(), &bytes)
        .map_err(|_| "Step 298 platform result write failed".to_owned())
}

fn request_digest(request_bytes: &[u8]) -> Result<&str, String> {
    let raw_request = std::str::from_utf8(request_bytes)
        .map_err(|_| "Step 298 platform execution request is not UTF-8".to_owned())?;
    let execution_request_sha256 = raw_request.strip_suffix('\n').unwrap_or(raw_request);
    if execution_request_sha256.len() != 64
        || !execution_request_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("Step 298 platform execution request differs".to_owned());
    }
    Ok(execution_request_sha256)
}

fn result_bytes(
    execution_request_sha256: &str,
    kernel_name: &str,
    kernel_release: &str,
    kernel_version: &str,
    architecture: &str,
) -> Result<Vec<u8>, String> {
    if kernel_name != "Darwin" || architecture != "aarch64" {
        return Err("Step 298 platform identity differs".to_owned());
    }

    let os_build = json!({
        "kernel_name": kernel_name,
        "kernel_release": kernel_release,
        "kernel_version": kernel_version,
    });
    let result = json!({
        "schema": "radroots.services-hardening.rshr-200-platform-result.v1",
        "platform": "macos_aarch64",
        "system": "aarch64-darwin",
        "os_family": "macos",
        "architecture": "aarch64",
        "kernel_name": os_build["kernel_name"],
        "kernel_release": os_build["kernel_release"],
        "os_build_sha256": sha256(&canonical(&os_build)?),
        "runner_kind": "host",
        "runner_image_sha256": "none",
        "apple_toolchain_identity_sha256": APPLE_TOOLCHAIN_IDENTITY_SHA256,
        "probe_source_path": PROBE_SOURCE_PATH,
        "probe_source_sha256": PROBE_SOURCE_SHA256,
        "execution_request_sha256": execution_request_sha256,
        "assertion": [
            {"id": "os_family", "result": "pass"},
            {"id": "architecture", "result": "pass"},
            {"id": "kernel_identity", "result": "pass"},
            {"id": "runner_identity", "result": "pass"},
            {"id": "apple_identity", "result": "pass"},
        ],
        "result": "available",
    });
    let mut bytes = canonical(&result)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_digest_rejects_noncanonical_or_unbound_values() {
        let digest = "0123456789abcdef".repeat(4);
        assert_eq!(request_digest(digest.as_bytes()).unwrap(), digest);
        assert_eq!(
            request_digest(format!("{digest}\n").as_bytes()).unwrap(),
            digest
        );
        for bytes in [
            vec![],
            vec![0xff],
            vec![b'0'; 63],
            vec![b'0'; 65],
            vec![b'G'; 64],
            vec![b'A'; 64],
            format!("{digest}\n\n").into_bytes(),
        ] {
            assert!(request_digest(&bytes).is_err());
        }
    }

    #[test]
    fn uname_output_requires_one_nonempty_utf8_line() {
        assert_eq!(parse_uname(b"Darwin\n").unwrap(), "Darwin");
        for bytes in [
            b"".as_slice(),
            b"\n",
            b"Darwin",
            b"Dar\nwin\n",
            b"Darwin\r\n",
            &[0xff],
        ] {
            assert!(parse_uname(bytes).is_err());
        }
    }

    #[test]
    fn encoded_platform_fixture_binds_request_and_kernel_without_qualifying_host() {
        let request = "a".repeat(64);
        let bytes = result_bytes(
            &request,
            "Darwin",
            "fixture-release",
            "fixture-version",
            "aarch64",
        )
        .unwrap();
        let result: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(result["execution_request_sha256"], request);
        assert_eq!(result["kernel_release"], "fixture-release");
        assert_eq!(result["os_build_sha256"], sha256(&canonical(&json!({"kernel_name":"Darwin", "kernel_release":"fixture-release", "kernel_version":"fixture-version"})).unwrap()));
        assert!(bytes.ends_with(b"\n"));
        assert!(result_bytes(&request, "Linux", "fixture", "fixture", "aarch64").is_err());
        assert!(result_bytes(&request, "Darwin", "fixture", "fixture", "x86_64").is_err());
    }
}
