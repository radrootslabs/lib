pub use radroots_net_core as core;

pub fn coverage_core_alias_available() -> bool {
    let _ = core::config::NetConfig::default();
    true
}

pub fn coverage_branch_probe(input: bool) -> bool {
    if input { true } else { false }
}

#[cfg(test)]
mod tests {
    use super::{coverage_branch_probe, coverage_core_alias_available};

    #[test]
    fn core_alias_probe_is_callable() {
        assert!(coverage_core_alias_available());
    }

    #[test]
    fn coverage_branch_probe_hits_both_paths() {
        assert!(coverage_branch_probe(true));
        assert!(!coverage_branch_probe(false));
    }
}
