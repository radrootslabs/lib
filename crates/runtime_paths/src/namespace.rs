use std::path::PathBuf;

use crate::RadrootsRuntimePathsError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadrootsRuntimeNamespaceKind {
    App,
    Service,
    Worker,
    Shared,
}

impl RadrootsRuntimeNamespaceKind {
    #[must_use]
    pub fn path_segment(self) -> &'static str {
        match self {
            Self::App => "apps",
            Self::Service => "services",
            Self::Worker => "workers",
            Self::Shared => "shared",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsRuntimeNamespace {
    kind: RadrootsRuntimeNamespaceKind,
    value: String,
}

impl RadrootsRuntimeNamespace {
    pub fn app(value: impl Into<String>) -> Result<Self, RadrootsRuntimePathsError> {
        Self::new(RadrootsRuntimeNamespaceKind::App, value)
    }

    pub fn service(value: impl Into<String>) -> Result<Self, RadrootsRuntimePathsError> {
        Self::new(RadrootsRuntimeNamespaceKind::Service, value)
    }

    pub fn worker(value: impl Into<String>) -> Result<Self, RadrootsRuntimePathsError> {
        Self::new(RadrootsRuntimeNamespaceKind::Worker, value)
    }

    pub fn shared(value: impl Into<String>) -> Result<Self, RadrootsRuntimePathsError> {
        Self::new(RadrootsRuntimeNamespaceKind::Shared, value)
    }

    pub fn new(
        kind: RadrootsRuntimeNamespaceKind,
        value: impl Into<String>,
    ) -> Result<Self, RadrootsRuntimePathsError> {
        let value = value.into();
        validate_component(&value)?;
        Ok(Self { kind, value })
    }

    #[must_use]
    pub fn kind(&self) -> RadrootsRuntimeNamespaceKind {
        self.kind
    }

    #[must_use]
    pub fn value(&self) -> &str {
        self.value.as_str()
    }

    #[must_use]
    pub fn relative_path(&self) -> PathBuf {
        PathBuf::from(self.kind.path_segment()).join(self.value.as_str())
    }
}

fn validate_component(value: &str) -> Result<(), RadrootsRuntimePathsError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed == "."
        || trimmed == ".."
        || trimmed.contains('/')
        || trimmed.contains('\\')
    {
        return Err(RadrootsRuntimePathsError::InvalidNamespaceComponent {
            value: value.to_owned(),
        });
    }
    Ok(())
}
