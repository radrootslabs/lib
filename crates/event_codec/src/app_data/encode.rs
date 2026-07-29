#[cfg(not(feature = "std"))]
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use radroots_event::social::app_data::{AppData, KIND_APP_DATA};
use radroots_event::tag::name::TAG_D;

use crate::error::EventEncodeError;
use radroots_event::wire::Nip01EventWireParts;

pub fn app_data_build_tags(app_data: &AppData) -> Result<Vec<Vec<String>>, EventEncodeError> {
    if app_data.d_tag.trim().is_empty() {
        return Err(EventEncodeError::EmptyRequiredField("d_tag"));
    }
    Ok(vec![vec![TAG_D.to_string(), app_data.d_tag.clone()]])
}

pub fn to_wire_parts(app_data: &AppData) -> Result<Nip01EventWireParts, EventEncodeError> {
    to_wire_parts_with_kind(app_data, KIND_APP_DATA)
}

pub fn to_wire_parts_with_kind(
    app_data: &AppData,
    kind: u32,
) -> Result<Nip01EventWireParts, EventEncodeError> {
    if kind != KIND_APP_DATA {
        return Err(EventEncodeError::InvalidKind(kind));
    }
    let tags = app_data_build_tags(app_data)?;
    Ok(Nip01EventWireParts {
        kind,
        content: app_data.content.clone(),
        tags,
    })
}
