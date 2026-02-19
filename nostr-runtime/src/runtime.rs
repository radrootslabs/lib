use crate::error::RadrootsNostrRuntimeError;
use crate::types::{
    RadrootsNostrConnectionSnapshot, RadrootsNostrRuntimeEvent, RadrootsNostrSubscriptionHandle,
    RadrootsNostrSubscriptionSpec, RadrootsNostrTrafficLight,
};

#[derive(Debug, Default, Clone)]
pub struct RadrootsNostrRuntimeBuilder {
    queue_capacity: Option<usize>,
}

impl RadrootsNostrRuntimeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn queue_capacity(mut self, capacity: usize) -> Self {
        self.queue_capacity = Some(capacity);
        self
    }

    pub fn build(self) -> Result<RadrootsNostrRuntime, RadrootsNostrRuntimeError> {
        let _ = self.queue_capacity;
        Ok(RadrootsNostrRuntime {})
    }
}

#[derive(Debug, Clone)]
pub struct RadrootsNostrRuntime {}

impl RadrootsNostrRuntime {
    pub async fn start(&self) -> Result<(), RadrootsNostrRuntimeError> {
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), RadrootsNostrRuntimeError> {
        Ok(())
    }

    pub async fn subscribe(
        &self,
        _spec: RadrootsNostrSubscriptionSpec,
    ) -> Result<RadrootsNostrSubscriptionHandle, RadrootsNostrRuntimeError> {
        Err(RadrootsNostrRuntimeError::Runtime(
            "subscribe not implemented".to_string(),
        ))
    }

    pub async fn unsubscribe(
        &self,
        _handle: &RadrootsNostrSubscriptionHandle,
    ) -> Result<(), RadrootsNostrRuntimeError> {
        Err(RadrootsNostrRuntimeError::Runtime(
            "unsubscribe not implemented".to_string(),
        ))
    }

    pub fn drain_events(&self, _max: usize) -> alloc::vec::Vec<RadrootsNostrRuntimeEvent> {
        alloc::vec::Vec::new()
    }

    pub fn snapshot(&self) -> RadrootsNostrConnectionSnapshot {
        RadrootsNostrConnectionSnapshot {
            light: RadrootsNostrTrafficLight::Red,
            connected: 0,
            connecting: 0,
            last_error: None,
        }
    }
}
