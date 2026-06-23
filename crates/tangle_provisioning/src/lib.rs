#![forbid(unsafe_code)]

pub mod error;
pub mod schema;
pub mod types;

pub use error::CommunityProvisioningError;
pub use schema::{load_contract_schema, load_schema_checked_value, validate_contract_schema};
pub use types::{
    ADAPTER_INPUT_SCHEMA, ADAPTER_OUTPUT_SCHEMA, CommunityProvisioningAdapterInput,
    CommunityProvisioningAdapterOutput, DiscoverabilityMode, GROUP_CREATE_KIND,
    GROUP_SEED_APPLY_MODE, PUBLIC_REPORT_SCHEMA, ProvisioningBackupExportInput,
    ProvisioningCustomDomainInput, ProvisioningGroupInput, ProvisioningGroupLimitsOutput,
    ProvisioningGroupPolicyOutput, ProvisioningGroupSeedOperation, ProvisioningGroupSeedOutput,
    ProvisioningGroupsOutput, ProvisioningHostConfigUpdate, ProvisioningInitialGroupInput,
    ProvisioningOperatorApply, ProvisioningPocketStoreOutput, ProvisioningRedactionOutput,
    ProvisioningRelaySelfKeyInput, ProvisioningRelaySelfKeyOutput, ProvisioningTenantInput,
    ProvisioningTenantOutput, ProvisioningValidationReport, UnsupportedFieldsPolicy,
};
