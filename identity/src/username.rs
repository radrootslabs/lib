#![forbid(unsafe_code)]

#[cfg(not(feature = "std"))]
use alloc::string::String;

pub const RADROOTS_USERNAME_MIN_LEN: usize = 3;
pub const RADROOTS_USERNAME_MAX_LEN: usize = 30;
pub const RADROOTS_USERNAME_REGEX: &str = r"^(?!.*\.\.)(?!\.)(?!.*\.$)[a-z0-9._-]{3,30}$";

pub fn radroots_username_is_valid(username: &str) -> bool {
    if !username.is_ascii() {
        return false;
    }
    let len = username.len();
    if len < RADROOTS_USERNAME_MIN_LEN || len > RADROOTS_USERNAME_MAX_LEN {
        return false;
    }
    let bytes = username.as_bytes();
    if bytes.first() == Some(&b'.') || bytes.last() == Some(&b'.') {
        return false;
    }
    let mut prev_dot = false;
    for &byte in bytes {
        if byte == b'.' {
            if prev_dot {
                return false;
            }
            prev_dot = true;
            continue;
        }
        prev_dot = false;
        let is_alpha = byte.is_ascii_lowercase();
        let is_digit = byte.is_ascii_digit();
        let is_allowed = is_alpha || is_digit || byte == b'_' || byte == b'-';
        if !is_allowed {
            return false;
        }
    }
    true
}

pub fn radroots_username_normalize(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    let normalized = trimmed.to_ascii_lowercase();
    if radroots_username_is_valid(&normalized) {
        Some(normalized)
    } else {
        None
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn valid_usernames() {
        for name in ["radroots", "radroots_1", "radroots.test", "rr-01"] {
            assert!(radroots_username_is_valid(name));
        }
    }

    #[test]
    fn invalid_usernames() {
        for name in [
            "ra",
            ".radroots",
            "radroots.",
            "radroots..test",
            "radroots!",
            "RADROOTS",
        ] {
            assert!(!radroots_username_is_valid(name));
        }
    }

    #[test]
    fn normalize_usernames() {
        assert_eq!(
            radroots_username_normalize("  RadRoots  "),
            Some("radroots".to_string())
        );
        assert_eq!(radroots_username_normalize("ra"), None);
    }
}

#[cfg(all(test, feature = "ts-rs", feature = "std"))]
mod constants_tests {
    use super::*;
    use std::{fs, path::Path};

    #[test]
    fn export_username_constants() {
        let path = Path::new("./bindings").join("constants.ts");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create ts export dir");
        }
        let content = format!(
            "export const RADROOTS_USERNAME_MIN_LEN = {min_len};\nexport const RADROOTS_USERNAME_MAX_LEN = {max_len};\nexport const RADROOTS_USERNAME_REGEX = \"{regex}\";\n",
            min_len = RADROOTS_USERNAME_MIN_LEN,
            max_len = RADROOTS_USERNAME_MAX_LEN,
            regex = RADROOTS_USERNAME_REGEX
        );
        fs::write(&path, content).expect("write constants");
    }
}
