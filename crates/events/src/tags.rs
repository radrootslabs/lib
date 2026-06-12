pub const TAG_A: &str = "a";
pub const TAG_A_ROOT: &str = "A";
pub const TAG_E: &str = "e";
pub const TAG_E_ROOT_NIP22: &str = "E";
pub const TAG_E_ROOT: &str = "e_root";
pub const TAG_E_PREV: &str = "e_prev";
pub const TAG_I: &str = "i";
pub const TAG_I_ROOT: &str = "I";
pub const TAG_K: &str = "k";
pub const TAG_K_ROOT: &str = "K";
pub const TAG_D: &str = "d";
pub const TAG_D_DAY: &str = "D";
pub const TAG_G: &str = "g";
pub const TAG_H: &str = "h";
pub const TAG_P: &str = "p";
pub const TAG_P_ROOT: &str = "P";
pub const TAG_Q: &str = "q";
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
pub const TAG_SIZE: &str = "size";
pub const TAG_DIMENSIONS: &str = "dim";
pub const TAG_BLURHASH: &str = "blurhash";
pub const TAG_THUMBNAIL: &str = "thumb";
pub const TAG_IMAGE: &str = "image";
pub const TAG_SUMMARY: &str = "summary";
pub const TAG_ALT: &str = "alt";
pub const TAG_FALLBACK: &str = "fallback";
pub const TAG_MAGNET: &str = "magnet";
pub const TAG_SERVICE: &str = "service";
pub const TAG_RELAY: &str = "relay";
pub const TAG_CHALLENGE: &str = "challenge";
pub const TAG_TITLE: &str = "title";
pub const TAG_PUBLISHED_AT: &str = "published_at";
pub const TAG_START: &str = "start";
pub const TAG_END: &str = "end";
pub const TAG_START_TZID: &str = "start_tzid";
pub const TAG_END_TZID: &str = "end_tzid";
pub const TAG_LOCATION: &str = "location";
pub const TAG_STATUS: &str = "status";
pub const TAG_FREE_BUSY: &str = "fb";
pub const TAG_DESCRIPTION: &str = "description";
pub const TAG_AMOUNT: &str = "amount";
pub const TAG_PRICE: &str = "price";
pub const TAG_CURRENCY: &str = "currency";
pub const TAG_SERVER: &str = "server";
pub const TAG_SUBJECT: &str = "subject";
pub const TAG_IMETA: &str = "imeta";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_shared_nostr_tag_keys() {
        assert_eq!(TAG_A, "a");
        assert_eq!(TAG_A_ROOT, "A");
        assert_eq!(TAG_D, "d");
        assert_eq!(TAG_D_DAY, "D");
        assert_eq!(TAG_E, "e");
        assert_eq!(TAG_E_ROOT_NIP22, "E");
        assert_eq!(TAG_G, "g");
        assert_eq!(TAG_H, "h");
        assert_eq!(TAG_I, "i");
        assert_eq!(TAG_I_ROOT, "I");
        assert_eq!(TAG_K, "k");
        assert_eq!(TAG_K_ROOT, "K");
        assert_eq!(TAG_P, "p");
        assert_eq!(TAG_P_ROOT, "P");
        assert_eq!(TAG_Q, "q");
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
        assert_eq!(TAG_SIZE, "size");
        assert_eq!(TAG_DIMENSIONS, "dim");
        assert_eq!(TAG_BLURHASH, "blurhash");
        assert_eq!(TAG_THUMBNAIL, "thumb");
        assert_eq!(TAG_IMAGE, "image");
        assert_eq!(TAG_SUMMARY, "summary");
        assert_eq!(TAG_ALT, "alt");
        assert_eq!(TAG_FALLBACK, "fallback");
        assert_eq!(TAG_MAGNET, "magnet");
        assert_eq!(TAG_SERVICE, "service");
        assert_eq!(TAG_RELAY, "relay");
        assert_eq!(TAG_CHALLENGE, "challenge");
    }

    #[test]
    fn exposes_social_event_tag_keys() {
        assert_eq!(TAG_TITLE, "title");
        assert_eq!(TAG_PUBLISHED_AT, "published_at");
        assert_eq!(TAG_START, "start");
        assert_eq!(TAG_END, "end");
        assert_eq!(TAG_START_TZID, "start_tzid");
        assert_eq!(TAG_END_TZID, "end_tzid");
        assert_eq!(TAG_LOCATION, "location");
        assert_eq!(TAG_STATUS, "status");
        assert_eq!(TAG_FREE_BUSY, "fb");
        assert_eq!(TAG_DESCRIPTION, "description");
        assert_eq!(TAG_AMOUNT, "amount");
        assert_eq!(TAG_PRICE, "price");
        assert_eq!(TAG_CURRENCY, "currency");
        assert_eq!(TAG_SERVER, "server");
        assert_eq!(TAG_SUBJECT, "subject");
        assert_eq!(TAG_IMETA, "imeta");
    }
}
