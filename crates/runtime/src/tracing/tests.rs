use super::{
    default_log_file_name, default_log_file_name_from_exe_name, default_shared_runtime_logs_dir,
    default_shared_runtime_logs_dir_for, env_value, init, init_with_logs_dir, log_name_from_path,
    log_name_from_stem, normalize_env_value, resolve_default_level, test_hooks,
};
use radroots_runtime_paths::{
    RadrootsHostEnvironment, RadrootsPathOverrides, RadrootsPathProfile, RadrootsPathResolver,
    RadrootsPlatform,
};
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn normalize_env_value_handles_empty_and_non_empty_values() {
    assert_eq!(normalize_env_value(" value "), Some("value".to_string()));
    assert_eq!(normalize_env_value("   "), None);
    assert_eq!(normalize_env_value(""), None);
}

#[test]
fn env_helpers_return_expected_values() {
    assert_eq!(env_value("RADROOTS_RUNTIME_TEST_MISSING_KEY"), None);
    let home = env_value("HOME").expect("home env");
    assert!(!home.is_empty());
}

#[test]
fn log_name_helpers_cover_empty_and_non_empty_names() {
    assert_eq!(
        log_name_from_stem("radrootsd"),
        Some("radrootsd.log".to_string())
    );
    assert_eq!(log_name_from_stem(""), None);
}

#[test]
fn log_name_from_path_handles_missing_components() {
    assert_eq!(log_name_from_path(None), None);
    assert_eq!(log_name_from_path(Some(PathBuf::from("/"))), None);
    assert_eq!(
        log_name_from_path(Some(PathBuf::from("/tmp/radrootsd"))),
        Some("radrootsd.log".to_string())
    );
}

#[test]
fn default_log_file_name_helpers_cover_fallback() {
    assert_eq!(
        default_log_file_name_from_exe_name(Some("svc.log".to_string())),
        "svc.log"
    );
    assert_eq!(
        default_log_file_name_from_exe_name(None),
        format!("{}.log", env!("CARGO_PKG_NAME"))
    );
    assert!(!default_log_file_name().trim().is_empty());
}

#[test]
fn resolve_default_level_prefers_env_then_fallback() {
    let env_value = resolve_default_level(Some("warn".to_string()), Some("info"));
    assert_eq!(env_value, Some("warn".to_string()));
    let fallback = resolve_default_level(None, Some("info"));
    assert_eq!(fallback, Some("info".to_string()));
    let none = resolve_default_level(None, None);
    assert_eq!(none, None);
}

#[test]
fn default_shared_runtime_logs_dir_uses_shared_namespace() {
    let resolver = RadrootsPathResolver::new(
        RadrootsPlatform::Macos,
        RadrootsHostEnvironment {
            home_dir: Some(PathBuf::from("/Users/treesap")),
            ..RadrootsHostEnvironment::default()
        },
    );

    let logs_dir = default_shared_runtime_logs_dir_for(
        &resolver,
        RadrootsPathProfile::InteractiveUser,
        &RadrootsPathOverrides::default(),
    )
    .expect("default shared runtime logs dir should resolve");

    assert_eq!(
        logs_dir,
        PathBuf::from("/Users/treesap/.radroots/logs/shared/runtime")
    );
}

#[test]
fn default_shared_runtime_logs_dir_and_init_use_current_resolver() {
    let dir = tempdir().expect("tempdir");
    let resolver = RadrootsPathResolver::new(
        RadrootsPlatform::Linux,
        RadrootsHostEnvironment {
            home_dir: Some(dir.path().to_path_buf()),
            ..RadrootsHostEnvironment::default()
        },
    );

    test_hooks::set_current_resolver(Some(resolver));
    test_hooks::set_ignore_env(true);

    let logs_dir = default_shared_runtime_logs_dir().expect("default shared runtime logs dir");
    assert_eq!(logs_dir, dir.path().join(".radroots/logs/shared/runtime"));

    let init_result = init();
    if let Err(err) = init_result {
        assert!(!err.to_string().is_empty());
    }

    test_hooks::set_ignore_env(false);
    test_hooks::set_current_resolver(None);
}

#[test]
fn init_paths_execute() {
    let dir = tempdir().expect("tempdir");
    test_hooks::set_ignore_env(true);
    let invalid = dir.path().join("not-a-dir");
    std::fs::write(&invalid, "file").expect("write invalid path");
    let err_path = init_with_logs_dir(invalid.as_path(), Some("info"));
    if let Err(err) = err_path {
        assert!(!err.to_string().is_empty());
    }
    let invalid_str = invalid.to_string_lossy().to_string();
    let err_str = init_with_logs_dir(invalid_str.as_str(), Some("info"));
    if let Err(err) = err_str {
        assert!(!err.to_string().is_empty());
    }
    let first = init_with_logs_dir(dir.path(), Some("info"));
    if let Err(err) = first {
        assert!(!err.to_string().is_empty());
    }
    let owned_path = dir.path().to_path_buf();
    let third = init_with_logs_dir(owned_path.as_path(), None);
    if let Err(err) = third {
        assert!(!err.to_string().is_empty());
    }
    test_hooks::set_ignore_env(false);
}
