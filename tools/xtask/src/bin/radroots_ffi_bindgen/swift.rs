//! Source-owned observer cancellation over UniFFI's thread-safe cancel ABI.
use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};

const START: &str = "fileprivate func uniffiRustCallAsync<F, T>(";
const END: &str = "// Callback handlers for an async calls.";
const UPSTREAM_SHA256: &str = "f05b2a0d1b6d335801874c67de4b25ad50d2995ccdf6650da0562a4b8d943518";
const CANCELLED: &str =
    "        case CALL_CANCELLED:\n            fatalError(\"Cancellation not supported yet\")";

pub(super) fn cancellation(source: &str, header: &str) -> Result<String> {
    ensure!(
        source.matches(START).count() == 1 && source.matches(END).count() == 1,
        "unexpected Swift async helper layout"
    );
    let start = source.find(START).expect("counted helper");
    let end = source.find(END).expect("counted boundary");
    ensure!(end > start, "invalid Swift helper boundaries");
    let original = &source[start..end];
    let initializers: Vec<_> = original
        .lines()
        .filter(|line| line.starts_with("    uniffiEnsure") && line.ends_with("Initialized()"))
        .collect();
    ensure!(initializers.len() == 1, "unexpected Swift initializer");
    let normalized = original.replace(initializers[0], "    {{ ensure_init_fn_name }}()");
    ensure!(
        format!("{:x}", Sha256::digest(normalized.as_bytes())) == UPSTREAM_SHA256,
        "upstream Swift async helper changed"
    );
    ensure!(
        source.matches(CANCELLED).count() == 1,
        "unexpected Swift cancellation error branch"
    );
    let helper = format!("{}\n", include_str!("async.swift"))
        .replace("    __RADROOTS_ENSURE_INIT__", initializers[0]);
    let mut output = format!("{}{}{}", &source[..start], helper, &source[end..]);
    output = output.replacen(
        CANCELLED,
        "        case CALL_CANCELLED:\n            throw CancellationError()",
        1,
    );
    let mut rewritten = String::with_capacity(output.len());
    let mut count = 0;
    for line in output.split_inclusive('\n') {
        if let Some(symbol) = line.strip_prefix("            freeFunc: ") {
            let symbol = symbol
                .strip_suffix(",\n")
                .ok_or_else(|| anyhow::anyhow!("unexpected Swift free callback"))?;
            ensure!(
                symbol
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                    && symbol.matches("_rust_future_free_").count() == 1,
                "unexpected Swift callback symbol"
            );
            let cancel = symbol.replacen("_rust_future_free_", "_rust_future_cancel_", 1);
            ensure!(
                header
                    .matches(&format!("void {cancel}(uint64_t handle"))
                    .count()
                    == 1,
                "missing native cancel ABI"
            );
            rewritten.push_str(&format!("            cancelFunc: {cancel},\n"));
            count += 1;
        }
        rewritten.push_str(line);
    }
    ensure!(count > 0, "no Swift async calls found");
    Ok(rewritten)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_unknown_layout_and_prepatched_input() {
        assert!(cancellation("", "").is_err());
        assert!(cancellation(&format!("{START}\nchanged\n{END}"), "").is_err());
        assert!(cancellation(include_str!("async.swift"), "").is_err());
    }
}
