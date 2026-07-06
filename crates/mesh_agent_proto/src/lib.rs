#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::string::String;
use core::fmt;
use sha2::{Digest, Sha256};

pub const RADROOTS_MESH_AGENT_SCHEMA_ID: &str = "0xb83e0c4f71838d9a";
pub const RADROOTS_MESH_AGENT_SCHEMA_NAMESPACE: &str = "radroots::mesh_agent::v1";
pub const RADROOTS_MESH_AGENT_SCHEMA: &str = include_str!("../schema/radroots_mesh_agent_v1.capnp");

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RadrootsMeshAgentProtoError {
    MissingSchemaId,
    MissingNamespace,
    MissingRequest,
    MissingAction,
    MissingResponse,
    MissingReceipt,
    MissingError,
}

impl fmt::Display for RadrootsMeshAgentProtoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSchemaId => f.write_str("mesh agent schema id is missing"),
            Self::MissingNamespace => f.write_str("mesh agent schema namespace is missing"),
            Self::MissingRequest => f.write_str("mesh agent request schema is missing"),
            Self::MissingAction => f.write_str("mesh agent action schema is missing"),
            Self::MissingResponse => f.write_str("mesh agent response schema is missing"),
            Self::MissingReceipt => f.write_str("mesh agent receipt schema is missing"),
            Self::MissingError => f.write_str("mesh agent error schema is missing"),
        }
    }
}

pub fn schema_sha256_hex() -> String {
    let mut hasher = Sha256::new();
    hasher.update(RADROOTS_MESH_AGENT_SCHEMA.as_bytes());
    hex_encode(&hasher.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

pub fn validate_schema() -> Result<(), RadrootsMeshAgentProtoError> {
    validate_schema_text(RADROOTS_MESH_AGENT_SCHEMA)
}

pub fn validate_schema_text(schema: &str) -> Result<(), RadrootsMeshAgentProtoError> {
    if !schema.contains(RADROOTS_MESH_AGENT_SCHEMA_ID) {
        return Err(RadrootsMeshAgentProtoError::MissingSchemaId);
    }
    if !schema.contains(RADROOTS_MESH_AGENT_SCHEMA_NAMESPACE) {
        return Err(RadrootsMeshAgentProtoError::MissingNamespace);
    }
    if !schema.contains("struct MeshAgentRequest") || !schema.contains("frameCbor @2 :Data;") {
        return Err(RadrootsMeshAgentProtoError::MissingRequest);
    }
    if !schema.contains("enum MeshAgentAction")
        || !schema.contains("validateFrame @0;")
        || !schema.contains("stageDelivery @1;")
        || !schema.contains("observeEventHead @2;")
    {
        return Err(RadrootsMeshAgentProtoError::MissingAction);
    }
    if !schema.contains("struct MeshAgentResponse")
        || !schema.contains("enum MeshAgentResponseStatus")
    {
        return Err(RadrootsMeshAgentProtoError::MissingResponse);
    }
    if !schema.contains("struct MeshAgentReceipt")
        || !schema.contains("acceptedEventHeads @1 :List(Text);")
    {
        return Err(RadrootsMeshAgentProtoError::MissingReceipt);
    }
    if !schema.contains("struct MeshAgentError") {
        return Err(RadrootsMeshAgentProtoError::MissingError);
    }
    Ok(())
}
