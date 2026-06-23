use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommunityProvisioningError {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("invalid community provisioning fixture: {0}")]
    Invalid(String),
}
