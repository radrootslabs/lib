#![forbid(unsafe_code)]

pub mod context;
pub mod conventions;
pub mod error;
pub mod identifier;
pub mod platform;
pub mod roots;
pub mod service;

pub use context::{
    RuntimeContext, RuntimeContextBootstrap, RuntimeContextError, RuntimeContextSource,
    RuntimeContextSources,
};
pub use conventions::{
    DEFAULT_CONFIG_FILE_NAME, DEFAULT_SHARED_GEONAMES_NAMESPACE,
    DEFAULT_SHARED_GEONAMES_NAMESPACE_KIND, DEFAULT_SHARED_GEONAMES_NAMESPACE_VALUE,
    DEFAULT_SHARED_RUNTIME_STORE_DB_FILE_NAME, DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE,
    DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE_KIND, DEFAULT_SHARED_RUNTIME_STORE_NAMESPACE_VALUE,
    RadrootsServiceInstanceArtifacts, SERVICE_ADMIN_SOCKET_FILE_NAME,
    SERVICE_CREDENTIAL_ARTIFACT_NAME_MAX_BYTES, SERVICE_STATE_DATABASE_FILE_NAME,
    SERVICE_STATE_LOCK_FILE_NAME, ServiceCredentialArtifactName,
    ServiceCredentialArtifactNameError, default_service_instance_artifacts,
    default_shared_geonames_database_file_name,
    default_shared_geonames_database_path_from_cache_root,
    default_shared_geonames_root_from_cache_root,
    default_shared_runtime_store_database_path_from_data_root,
    default_shared_runtime_store_database_path_from_shared_accounts_data_root,
    default_shared_runtime_store_root_from_data_root,
    default_shared_runtime_store_root_from_shared_accounts_data_root,
    service_credential_artifact_path,
};
pub use error::RadrootsRuntimePathsError;
pub use identifier::{
    INSTANCE_ID_MAX_BYTES, InstanceId, SERVICE_ID_MAX_BYTES, ServiceId, ServiceIdentityError,
    ServiceIdentityKind,
};
pub use platform::{RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPlatform};
pub use roots::RadrootsPathResolver;
pub use service::RadrootsServiceInstancePaths;
