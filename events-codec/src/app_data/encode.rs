#[cfg(not(feature = "std"))]
use alloc::{string::String, vec::Vec};

use radroots_events::app_data::{RadrootsAppData, KIND_APP_DATA};
use radroots_events::tags::TAG_D;

use crate::error::EventEncodeError;
use crate::wire::WireEventParts;

pub fn app_data_build_tags(
    app_data: &RadrootsAppData,
) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if app_data.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("d_tag"));
    }
    let mut tags = Vec::with_capacity(1);
    tags.push(vec![TAG_D.to_string(), app_data.d_tag.clone()]);
    Ok(tags)
}

pub fn to_wire_parts(app_data: &RadrootsAppData) -> Result<WireEventParts, EventEncodeError> {
    to_wire_parts_with_kind(app_data, KIND_APP_DATA)
}

pub fn to_wire_parts_with_kind(
    app_data: &RadrootsAppData,
    kind: u32,
) -> Result<WireEventParts, EventEncodeError> {
    if kind != KIND_APP_DATA {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = app_data_build_tags(app_data)?;
    Ok(WireEventParts {
        kind,
        content: app_data.content.clone(),
        tags,
    })
}
