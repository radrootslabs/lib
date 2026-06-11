pub const TAG_A: &str = "a";
pub const TAG_E: &str = "e";
pub const TAG_E_ROOT: &str = "e_root";
pub const TAG_E_PREV: &str = "e_prev";
pub const TAG_D: &str = "d";
pub const TAG_G: &str = "g";
pub const TAG_H: &str = "h";
pub const TAG_P: &str = "p";
pub const TAG_R: &str = "r";
pub const TAG_T: &str = "t";
pub const TAG_U: &str = "u";
pub const TAG_URL: &str = "url";
pub const TAG_URL_AUTH: &str = TAG_U;
pub const TAG_METHOD: &str = "method";
pub const TAG_MIME: &str = "m";
pub const TAG_PAYLOAD: &str = "payload";
pub const TAG_SHA256: &str = "x";
pub const TAG_ORIGINAL_SHA256: &str = "ox";
pub const TAG_RELAY: &str = "relay";
pub const TAG_CHALLENGE: &str = "challenge";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_shared_nostr_tag_keys() {
        assert_eq!(TAG_A, "a");
        assert_eq!(TAG_D, "d");
        assert_eq!(TAG_E, "e");
        assert_eq!(TAG_G, "g");
        assert_eq!(TAG_H, "h");
        assert_eq!(TAG_P, "p");
        assert_eq!(TAG_R, "r");
        assert_eq!(TAG_T, "t");
        assert_eq!(TAG_U, "u");
        assert_eq!(TAG_URL_AUTH, TAG_U);
    }

    #[test]
    fn exposes_field_file_and_auth_tag_keys() {
        assert_eq!(TAG_URL, "url");
        assert_eq!(TAG_METHOD, "method");
        assert_eq!(TAG_MIME, "m");
        assert_eq!(TAG_PAYLOAD, "payload");
        assert_eq!(TAG_SHA256, "x");
        assert_eq!(TAG_ORIGINAL_SHA256, "ox");
        assert_eq!(TAG_RELAY, "relay");
        assert_eq!(TAG_CHALLENGE, "challenge");
    }
}
