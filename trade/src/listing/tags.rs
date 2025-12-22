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

        if has_root && has_d {
            return Ok(());
        }
    }

    if !has_root {
        return Err(JobParseError::MissingChainTag(TAG_E_ROOT));
    }
    if !has_d {
        return Err(JobParseError::MissingChainTag(TAG_D));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_trade_listing_chain;
    use radroots_events::tags::{TAG_D, TAG_E_ROOT};
    use radroots_events_codec::job::error::JobParseError;

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
        match validate_trade_listing_chain(&tags) {
            Err(JobParseError::MissingChainTag(tag)) => {
                assert_eq!(tag, TAG_E_ROOT);
            }
            other => panic!("expected missing root tag, got {other:?}"),
        }
    }

    #[test]
    fn validate_trade_listing_chain_rejects_empty_root_value() {
        let tags = vec![
            vec![TAG_E_ROOT.into(), " ".into()],
            vec![TAG_D.into(), "trade".into()],
        ];
        match validate_trade_listing_chain(&tags) {
            Err(JobParseError::InvalidTag(tag)) => {
                assert_eq!(tag, TAG_E_ROOT);
            }
            other => panic!("expected invalid root tag, got {other:?}"),
        }
    }
}
