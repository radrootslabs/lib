#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::tag::{TAG_D, TAG_E_PREV, TAG_E_ROOT};
use radroots_events_codec::job::error::JobParseError;

#[inline]
pub fn push_trade_listing_chain_tags(
    tags: &mut Vec<Vec<String>>,
    e_root_id: impl Into<String>,
    e_prev_id: Option<impl Into<String>>,
    trade_id: Option<impl Into<String>>,
) {
    tags.push(vec![TAG_E_ROOT.into(), e_root_id.into()]);
    if let Some(prev) = e_prev_id {
        tags.push(vec![TAG_E_PREV.into(), prev.into()]);
    }
    if let Some(d) = trade_id {
        tags.push(vec![TAG_D.into(), d.into()]);
    }
}

#[inline]
pub fn validate_trade_listing_chain(tags: &[Vec<String>]) -> Result<(), JobParseError> {
    let has_root = tags
        .iter()
        .any(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_E_ROOT));
    if !has_root {
        return Err(JobParseError::MissingChainTag(TAG_E_ROOT));
    }
    let has_d = tags
        .iter()
        .any(|t| t.get(0).map(|s| s.as_str()) == Some(TAG_D));
    if !has_d {
        return Err(JobParseError::MissingChainTag(TAG_D));
    }
    Ok(())
}
