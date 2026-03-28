use alloc::vec::Vec;
use radroots_simplex_smp_proto::prelude::{
    RadrootsSimplexSmpBrokerTransmission, RadrootsSimplexSmpCommandTransmission,
    RadrootsSimplexSmpServerAddress,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpTransportRequest {
    pub server: RadrootsSimplexSmpServerAddress,
    pub transport_version: u16,
    pub transmission: RadrootsSimplexSmpCommandTransmission,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsSimplexSmpTransportResponse {
    pub server: RadrootsSimplexSmpServerAddress,
    pub transport_version: u16,
    pub transmission: RadrootsSimplexSmpBrokerTransmission,
    pub transport_hash: Vec<u8>,
}

pub trait RadrootsSimplexSmpCommandTransport {
    type Error: core::fmt::Display;

    fn execute(
        &mut self,
        request: RadrootsSimplexSmpTransportRequest,
    ) -> Result<RadrootsSimplexSmpTransportResponse, Self::Error>;
}
