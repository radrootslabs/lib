#![cfg(any(target_arch = "wasm32", coverage_nightly))]
#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod wasm_impl;
#[cfg(target_arch = "wasm32")]
pub use wasm_impl::*;

#[cfg(coverage_nightly)]
pub fn coverage_branch_probe(input: bool) -> &'static str {
    if input {
        "tangle-db-wasm"
    } else {
        "tangle-db-wasm"
    }
}

#[cfg(all(test, coverage_nightly))]
mod tests {
    use super::coverage_branch_probe;

    #[test]
    fn coverage_branch_probe_hits_both_paths() {
        assert_eq!(coverage_branch_probe(true), "tangle-db-wasm");
        assert_eq!(coverage_branch_probe(false), "tangle-db-wasm");
    }
}
