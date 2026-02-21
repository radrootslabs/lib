#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT};
use radroots_events_codec::job::error::JobParseError;

#[inline]
fn push_tag(tags: &mut Vec<Vec<String>>, name: &'static str, value: impl Into<String>) {
    let mut tag = Vec::with_capacity(2);
    tag.push(name.to_owned());
    tag.push(value.into());
    tags.push(tag);
}

#[inline]
pub fn trade_listing_dvm_tags<P, A, D>(
    recipient_pubkey: P,
    listing_addr: A,
    order_id: Option<D>,
) -> Vec<Vec<String>>
where
    P: Into<String>,
    A: Into<String>,
    D: Into<String>,
{
    let mut tags = Vec::with_capacity(2 + usize::from(order_id.is_some()));
    push_tag(&mut tags, "p", recipient_pubkey);
    push_tag(&mut tags, "a", listing_addr);
    if let Some(order_id) = order_id {
        push_tag(&mut tags, TAG_D, order_id);
    }
    tags
}

#[inline]
pub fn push_trade_listing_chain_tags(
    tags: &mut Vec<Vec<String>>,
    e_root_id: impl Into<String>,
    e_prev_id: Option<impl Into<String>>,
    trade_id: Option<impl Into<String>>,
) {
    let mut reserve = 1;
    if e_prev_id.is_some() {
        reserve += 1;
    }
    if trade_id.is_some() {
        reserve += 1;
    }
    tags.reserve(reserve);

    push_tag(tags, TAG_E_ROOT, e_root_id);
    if let Some(prev) = e_prev_id {
        push_tag(tags, TAG_E_PREV, prev);
    }
    if let Some(d) = trade_id {
        push_tag(tags, TAG_D, d);
    }
}

#[inline]
pub fn validate_trade_listing_chain(tags: &[Vec<String>]) -> Result<(), JobParseError> {
    let mut has_root = false;
    let mut has_d = false;

    for tag in tags {
        match tag.as_slice() {
            [key, value, ..] if key == TAG_E_ROOT => {
                if value.trim().is_empty() {
                    return Err(JobParseError::InvalidTag(TAG_E_ROOT));
                }
                has_root = true;
            }
            [key] if key == TAG_E_ROOT => {
                return Err(JobParseError::InvalidTag(TAG_E_ROOT));
            }
            [key, value, ..] if key == TAG_D => {
                if value.trim().is_empty() {
                    return Err(JobParseError::InvalidTag(TAG_D));
                }
                has_d = true;
            }
            [key] if key == TAG_D => {
                return Err(JobParseError::InvalidTag(TAG_D));
            }
            _ => {}
        }
    }

    if !has_root {
        Err(JobParseError::MissingChainTag(TAG_E_ROOT))
    } else if !has_d {
        Err(JobParseError::MissingChainTag(TAG_D))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        push_trade_listing_chain_tags, trade_listing_dvm_tags, validate_trade_listing_chain,
    };
    use radroots_events::kinds::KIND_LISTING;
    use radroots_events::tags::{TAG_D, TAG_E_PREV, TAG_E_ROOT};

    #[test]
    fn validate_trade_listing_chain_ok() {
        let tags = vec![
            vec![TAG_E_ROOT.into(), "root".into()],
            vec![TAG_D.into(), "trade".into()],
        ];
        assert!(validate_trade_listing_chain(&tags).is_ok());
    }

    #[test]
    fn validate_trade_listing_chain_rejects_missing_root() {
        let tags = vec![vec![TAG_D.into(), "trade".into()]];
        let err = validate_trade_listing_chain(&tags).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("missing required chain tag: {TAG_E_ROOT}")
        );
    }

    #[test]
    fn validate_trade_listing_chain_rejects_empty_root_value() {
        let tags = vec![
            vec![TAG_E_ROOT.into(), " ".into()],
            vec![TAG_D.into(), "trade".into()],
        ];
        let err = validate_trade_listing_chain(&tags).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("invalid tag structure for '{TAG_E_ROOT}'")
        );
    }

    #[test]
    fn validate_trade_listing_chain_rejects_root_without_value() {
        let tags = vec![vec![TAG_E_ROOT.into()], vec![TAG_D.into(), "trade".into()]];
        let err = validate_trade_listing_chain(&tags).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("invalid tag structure for '{TAG_E_ROOT}'")
        );
    }

    #[test]
    fn validate_trade_listing_chain_rejects_missing_trade_id() {
        let tags = vec![vec![TAG_E_ROOT.into(), "root".into()]];
        let err = validate_trade_listing_chain(&tags).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("missing required chain tag: {TAG_D}")
        );
    }

    #[test]
    fn validate_trade_listing_chain_rejects_empty_trade_id() {
        let tags = vec![
            vec![TAG_E_ROOT.into(), "root".into()],
            vec![TAG_D.into(), " ".into()],
        ];
        let err = validate_trade_listing_chain(&tags).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("invalid tag structure for '{TAG_D}'")
        );
    }

    #[test]
    fn validate_trade_listing_chain_rejects_trade_id_without_value() {
        let tags = vec![vec![TAG_E_ROOT.into(), "root".into()], vec![TAG_D.into()]];
        let err = validate_trade_listing_chain(&tags).unwrap_err();
        assert_eq!(
            err.to_string(),
            format!("invalid tag structure for '{TAG_D}'")
        );
    }

    #[test]
    fn validate_trade_listing_chain_accepts_trade_id_before_root() {
        let tags = vec![
            vec![TAG_D.into(), "trade".into()],
            vec!["x".into(), "ignore".into()],
            vec![TAG_E_ROOT.into(), "root".into()],
        ];
        assert!(validate_trade_listing_chain(&tags).is_ok());
    }

    #[test]
    fn validate_trade_listing_chain_ignores_unknown_single_key_tag() {
        let tags = vec![
            vec!["x".into()],
            vec![TAG_E_ROOT.into(), "root".into()],
            vec![TAG_D.into(), "trade".into()],
        ];
        assert!(validate_trade_listing_chain(&tags).is_ok());
    }

    #[test]
    fn trade_listing_dvm_tags_builds_expected_tags() {
        let listing_addr = format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg");
        let tags = trade_listing_dvm_tags("pubkey", &listing_addr, Some("order-1"));
        let expected: Vec<Vec<String>> = vec![
            vec![String::from("p"), String::from("pubkey")],
            vec![String::from("a"), listing_addr.clone()],
            vec![String::from(TAG_D), String::from("order-1")],
        ];
        assert_eq!(tags, expected);
    }

    #[test]
    fn trade_listing_dvm_tags_omit_order_id_when_missing() {
        let listing_addr = format!("{KIND_LISTING}:pubkey:AAAAAAAAAAAAAAAAAAAAAg");
        let tags = trade_listing_dvm_tags("pubkey", &listing_addr, None::<String>);
        let expected: Vec<Vec<String>> = vec![
            vec![String::from("p"), String::from("pubkey")],
            vec![String::from("a"), listing_addr.clone()],
        ];
        assert_eq!(tags, expected);
    }

    #[test]
    fn push_trade_listing_chain_tags_appends_optional_fields() {
        let mut tags = vec![vec![String::from("x"), String::from("seed")]];
        push_trade_listing_chain_tags(
            &mut tags,
            "root-id",
            Some("prev-id".to_string()),
            Some("trade-id".to_string()),
        );

        assert_eq!(
            tags,
            vec![
                vec![String::from("x"), String::from("seed")],
                vec![String::from(TAG_E_ROOT), String::from("root-id")],
                vec![String::from(TAG_E_PREV), String::from("prev-id")],
                vec![String::from(TAG_D), String::from("trade-id")],
            ]
        );
    }

    #[test]
    fn push_trade_listing_chain_tags_skips_missing_optional_fields() {
        let mut tags = Vec::new();
        push_trade_listing_chain_tags(
            &mut tags,
            "root-id",
            Option::<String>::None,
            Option::<String>::None,
        );

        assert_eq!(
            tags,
            vec![vec![String::from(TAG_E_ROOT), String::from("root-id")]]
        );
    }
}
