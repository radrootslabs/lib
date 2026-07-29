use std::path::Path;

pub(crate) mod protocol;

pub(crate) fn run(args: &[String], workspace_root: &Path) -> Result<(), String> {
    match args {
        [target, mode] if target == "protocol" => protocol::run(mode, workspace_root),
        _ => Err("usage: cargo xtask generate protocol --check|--write".to_owned()),
    }
}
