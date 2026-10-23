use std::path::Path;

pub(crate) mod data_notices;
pub(crate) mod protocol;

pub(crate) fn run(args: &[String], workspace_root: &Path) -> Result<(), String> {
    match args {
        [target, mode] if target == "protocol" => protocol::run(mode, workspace_root),
        [target, mode] if target == "data-notices" => data_notices::run(mode, workspace_root),
        _ => Err("usage: cargo xtask generate protocol|data-notices --check|--write".to_owned()),
    }
}
