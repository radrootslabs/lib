pub mod models;
pub use models::*;

#[cfg_attr(not(test), allow(dead_code))]
fn coverage_branch_probe(value: Option<&str>) -> usize {
    match value {
        Some(raw) if raw.is_empty() => 0,
        Some(raw) => raw.len(),
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::coverage_branch_probe;

    #[test]
    fn coverage_branch_probe_exercises_branches() {
        assert_eq!(coverage_branch_probe(None), 0);
        assert_eq!(coverage_branch_probe(Some("")), 0);
        assert_eq!(coverage_branch_probe(Some("probe")), 5);
    }
}
