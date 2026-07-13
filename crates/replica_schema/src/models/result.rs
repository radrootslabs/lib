use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReplicaSchemaError<T> {
    pub error: T,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReplicaSchemaResult<T> {
    pub result: T,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReplicaSchemaResultList<T> {
    pub results: Vec<T>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReplicaSchemaResultPass {
    pub pass: bool,
}

impl<T> From<T> for ReplicaSchemaError<T> {
    fn from(error: T) -> Self {
        Self { error }
    }
}

impl<T> ReplicaSchemaError<T> {
    pub fn new(error: T) -> Self {
        Self { error }
    }
}

impl<T> ReplicaSchemaResult<T> {
    pub fn new(result: T) -> Self {
        Self { result }
    }
}

impl<T> ReplicaSchemaResultList<T> {
    pub fn new(results: Vec<T>) -> Self {
        Self { results }
    }

    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }
}

impl ReplicaSchemaResultPass {
    pub fn new(pass: bool) -> Self {
        Self { pass }
    }

    pub fn status_label(&self) -> &'static str {
        if self.pass { "pass" } else { "fail" }
    }
}
