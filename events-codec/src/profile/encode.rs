use crate::profile::error::ProfileEncodeError;
use radroots_events::profile::models::RadrootsProfile;

use nostr::Metadata;
use nostr::prelude::Url;

#[cfg(feature = "serde_json")]
use crate::job::encode::WireEventParts;

pub fn to_metadata(p: &RadrootsProfile) -> Result<Metadata, ProfileEncodeError> {
    let mut md = Metadata::new().name(p.name.clone());

    if let Some(s) = &p.display_name {
        md = md.display_name(s.clone());
    }
    if let Some(s) = &p.about {
        md = md.about(s.clone());
    }
    if let Some(s) = &p.website {
        let u = Url::parse(s).map_err(|_| ProfileEncodeError::InvalidUrl("website", s.clone()))?;
        md = md.website(u);
    }
    if let Some(s) = &p.picture {
        let u = Url::parse(s).map_err(|_| ProfileEncodeError::InvalidUrl("picture", s.clone()))?;
        md = md.picture(u);
    }
    if let Some(s) = &p.banner {
        let u = Url::parse(s).map_err(|_| ProfileEncodeError::InvalidUrl("banner", s.clone()))?;
        md = md.banner(u);
    }
    if let Some(s) = &p.nip05 {
        md = md.nip05(s.clone());
    }
    if let Some(s) = &p.lud06 {
        md = md.lud06(s.clone());
    }
    if let Some(s) = &p.lud16 {
        md = md.lud16(s.clone());
    }

    Ok(md)
}

#[cfg(feature = "serde_json")]
pub fn to_wire_parts(p: &RadrootsProfile) -> Result<WireEventParts, ProfileEncodeError> {
    let md = to_metadata(p)?;
    let content = serde_json::to_string(&md).map_err(|_| ProfileEncodeError::Json)?;
    Ok(WireEventParts {
        kind: 0,
        content,
        tags: Vec::new(),
    })
}
