#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod cbor;
mod error;
mod model;

pub use cbor::{decode_mesh_frame_cbor, encode_mesh_frame_cbor};
pub use error::RadrootsMeshError;
pub use model::{
    RADROOTS_MESH_FRAME_VERSION, RadrootsMeshFrame, RadrootsMeshFrameType, RadrootsMeshPayload,
    RadrootsMeshScope,
};
