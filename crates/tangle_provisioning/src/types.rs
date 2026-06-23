use std::{fs, path::Path};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    CommunityProvisioningError,
    schema::{load_schema_checked_value, validate_contract_schema},
};

pub const ADAPTER_INPUT_SCHEMA: &str = "radroots.tangle.community_provisioning.adapter_input.v1";
pub const ADAPTER_OUTPUT_SCHEMA: &str = "radroots.tangle.community_provisioning.adapter_output.v1";
pub const PUBLIC_REPORT_SCHEMA: &str = "radroots.tangle.community_provisioning.public_report.v1";
pub const GROUP_CREATE_KIND: u64 = 9_007;
pub const GROUP_SEED_APPLY_MODE: &str = "publish_tenant_local_relay_event";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityProvisioningAdapterInput {
    pub schema: String,
    pub community_space_id: String,
    pub display_name: String,
    pub description: String,
    pub discoverability: DiscoverabilityMode,
    pub tenant: ProvisioningTenantInput,
    pub groups: ProvisioningGroupInput,
    pub backup_export: ProvisioningBackupExportInput,
    pub relay_self_key: ProvisioningRelaySelfKeyInput,
    pub custom_domain: Option<ProvisioningCustomDomainInput>,
    pub unsupported_fields_policy: UnsupportedFieldsPolicy,
}

impl CommunityProvisioningAdapterInput {
    pub fn load(path: &Path) -> Result<Self, CommunityProvisioningError> {
        let input: Self = serde_json::from_str(&fs::read_to_string(path)?)?;
        input.validate()?;
        Ok(input)
    }

    pub fn load_with_schema(
        path: &Path,
        schema: &Value,
    ) -> Result<Self, CommunityProvisioningError> {
        let value = load_schema_checked_value(path, schema)?;
        let input: Self = serde_json::from_value(value)?;
        input.validate()?;
        Ok(input)
    }

    pub fn validate(&self) -> Result<(), CommunityProvisioningError> {
        if self.schema != ADAPTER_INPUT_SCHEMA {
            return Err(CommunityProvisioningError::Invalid(format!(
                "adapter input schema must be {ADAPTER_INPUT_SCHEMA}"
            )));
        }
        if self.community_space_id != self.tenant.tenant_id {
            return Err(CommunityProvisioningError::Invalid(
                "community_space_id must match tenant_id".to_owned(),
            ));
        }
        if !self.groups.enabled {
            return Err(CommunityProvisioningError::Invalid(
                "community groups must be enabled".to_owned(),
            ));
        }
        if self.groups.initial_groups.is_empty() {
            return Err(CommunityProvisioningError::Invalid(
                "at least one initial group is required".to_owned(),
            ));
        }
        if !is_lower_hex_64(&self.relay_self_key.fixture_secret_hex) {
            return Err(CommunityProvisioningError::Invalid(
                "relay self key fixture secret must be lowercase hex".to_owned(),
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn materialize_output(&self) -> CommunityProvisioningAdapterOutput {
        let data_directory = format!("runtime/tenants/{}/pocket", self.tenant.tenant_schema);
        let tenant_config_path = format!("tenants/{}.json", self.tenant.tenant_schema);
        CommunityProvisioningAdapterOutput {
            schema: ADAPTER_OUTPUT_SCHEMA.to_owned(),
            community_space_id: self.community_space_id.clone(),
            host_config_update: ProvisioningHostConfigUpdate {
                tenant_config_dir: "tenants".to_owned(),
                tenant_config_file: tenant_config_path,
                apply_mode: "write_config_then_restart".to_owned(),
            },
            tenant: ProvisioningTenantOutput {
                tenant_id: self.tenant.tenant_id.clone(),
                tenant_schema: self.tenant.tenant_schema.clone(),
                canonical_host: self.tenant.canonical_host.clone(),
                relay_url: self.tenant.relay_url.clone(),
                display_name: self.display_name.clone(),
                description: self.description.clone(),
                discoverability: self.discoverability,
            },
            pocket_store: ProvisioningPocketStoreOutput {
                data_directory,
                create_if_missing: true,
            },
            relay_self_key: ProvisioningRelaySelfKeyOutput {
                mode: self.relay_self_key.mode.clone(),
                secret_ref: self.relay_self_key.secret_ref.clone(),
                materialized_config_field: "groups.relay_secret".to_owned(),
            },
            backup_export: self.backup_export.clone(),
            groups: ProvisioningGroupsOutput {
                enabled: self.groups.enabled,
                canonical_relay_url: self.tenant.relay_url.clone(),
                owner_pubkeys: vec![self.tenant.owner_pubkey.clone()],
                admin_pubkeys: self.tenant.admin_pubkeys.clone(),
                policy: ProvisioningGroupPolicyOutput {
                    public_join: self.groups.public_join,
                    invites_enabled: self.groups.invites_enabled,
                },
                limits: ProvisioningGroupLimitsOutput::default(),
            },
            group_seed: group_seed_output(self),
            redaction: ProvisioningRedactionOutput {
                redact_fields: vec!["groups.relay_secret".to_owned()],
                public_report_allowed: true,
            },
            validation_report: ProvisioningValidationReport {
                adapter_input_schema: ADAPTER_INPUT_SCHEMA.to_owned(),
                adapter_output_schema: ADAPTER_OUTPUT_SCHEMA.to_owned(),
                unsupported_fields_policy: self.unsupported_fields_policy,
                tangle_config_validate_required: true,
            },
            operator_apply: ProvisioningOperatorApply {
                instruction:
                    "write tenant config file, update host config reference, restart Tangle"
                        .to_owned(),
                hot_reload: false,
                remote_management_route: false,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscoverabilityMode {
    Public,
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnsupportedFieldsPolicy {
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningTenantInput {
    pub tenant_id: String,
    pub tenant_schema: String,
    pub canonical_host: String,
    pub relay_url: String,
    pub owner_pubkey: String,
    pub admin_pubkeys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningGroupInput {
    pub enabled: bool,
    pub public_join: bool,
    pub invites_enabled: bool,
    pub initial_groups: Vec<ProvisioningInitialGroupInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningInitialGroupInput {
    pub group_id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningBackupExportInput {
    pub backup_enabled: bool,
    pub export_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningRelaySelfKeyInput {
    pub mode: String,
    pub secret_ref: String,
    pub fixture_secret_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningCustomDomainInput {
    pub requested_host: String,
    pub certificate_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommunityProvisioningAdapterOutput {
    pub schema: String,
    pub community_space_id: String,
    pub host_config_update: ProvisioningHostConfigUpdate,
    pub tenant: ProvisioningTenantOutput,
    pub pocket_store: ProvisioningPocketStoreOutput,
    pub relay_self_key: ProvisioningRelaySelfKeyOutput,
    pub backup_export: ProvisioningBackupExportInput,
    pub groups: ProvisioningGroupsOutput,
    pub group_seed: ProvisioningGroupSeedOutput,
    pub redaction: ProvisioningRedactionOutput,
    pub validation_report: ProvisioningValidationReport,
    pub operator_apply: ProvisioningOperatorApply,
}

impl CommunityProvisioningAdapterOutput {
    pub fn load(path: &Path) -> Result<Self, CommunityProvisioningError> {
        let output: Self = serde_json::from_str(&fs::read_to_string(path)?)?;
        output.validate()?;
        Ok(output)
    }

    pub fn load_with_schema(
        path: &Path,
        schema: &Value,
    ) -> Result<Self, CommunityProvisioningError> {
        let value = load_schema_checked_value(path, schema)?;
        let output: Self = serde_json::from_value(value)?;
        output.validate()?;
        Ok(output)
    }

    pub fn validate(&self) -> Result<(), CommunityProvisioningError> {
        if self.schema != ADAPTER_OUTPUT_SCHEMA {
            return Err(CommunityProvisioningError::Invalid(format!(
                "adapter output schema must be {ADAPTER_OUTPUT_SCHEMA}"
            )));
        }
        if self.community_space_id != self.tenant.tenant_id {
            return Err(CommunityProvisioningError::Invalid(
                "community_space_id must match tenant_id".to_owned(),
            ));
        }
        if self.host_config_update.tenant_config_dir != "tenants"
            || self.host_config_update.tenant_config_file
                != format!("tenants/{}.json", self.tenant.tenant_schema)
            || self.host_config_update.apply_mode != "write_config_then_restart"
        {
            return Err(CommunityProvisioningError::Invalid(
                "host config update must target the tenant config file".to_owned(),
            ));
        }
        if self.operator_apply.hot_reload || self.operator_apply.remote_management_route {
            return Err(CommunityProvisioningError::Invalid(
                "adapter output must remain outside Tangle remote management".to_owned(),
            ));
        }
        if self.operator_apply.instruction.is_empty() {
            return Err(CommunityProvisioningError::Invalid(
                "operator apply instruction is required".to_owned(),
            ));
        }
        if self.pocket_store.data_directory
            != format!("runtime/tenants/{}/pocket", self.tenant.tenant_schema)
            || !self.pocket_store.create_if_missing
        {
            return Err(CommunityProvisioningError::Invalid(
                "pocket store output must match tenant schema".to_owned(),
            ));
        }
        if !self.groups.enabled || self.groups.canonical_relay_url != self.tenant.relay_url {
            return Err(CommunityProvisioningError::Invalid(
                "group output must use the tenant relay url".to_owned(),
            ));
        }
        let Some(owner_pubkey) = self.groups.owner_pubkeys.first() else {
            return Err(CommunityProvisioningError::Invalid(
                "group output must include an owner pubkey".to_owned(),
            ));
        };
        if self.group_seed.apply_mode != GROUP_SEED_APPLY_MODE
            || self.group_seed.operations.is_empty()
        {
            return Err(CommunityProvisioningError::Invalid(
                "adapter output must include tenant-local group seed operations".to_owned(),
            ));
        }
        if self.relay_self_key.materialized_config_field != "groups.relay_secret" {
            return Err(CommunityProvisioningError::Invalid(
                "relay self key must materialize into groups.relay_secret".to_owned(),
            ));
        }
        if !self.redaction.public_report_allowed
            || !self
                .redaction
                .redact_fields
                .iter()
                .any(|field| field == "groups.relay_secret")
        {
            return Err(CommunityProvisioningError::Invalid(
                "redaction output must protect groups.relay_secret".to_owned(),
            ));
        }
        if self.validation_report.adapter_input_schema != ADAPTER_INPUT_SCHEMA
            || self.validation_report.adapter_output_schema != ADAPTER_OUTPUT_SCHEMA
            || self.validation_report.unsupported_fields_policy != UnsupportedFieldsPolicy::Reject
            || !self.validation_report.tangle_config_validate_required
        {
            return Err(CommunityProvisioningError::Invalid(
                "validation report must require strict adapter validation".to_owned(),
            ));
        }
        for operation in &self.group_seed.operations {
            if operation.community_space_id != self.community_space_id
                || operation.tenant_id != self.tenant.tenant_id
                || operation.tenant_relay_url != self.tenant.relay_url
                || operation.event_kind != GROUP_CREATE_KIND
                || operation.apply_mode != GROUP_SEED_APPLY_MODE
                || operation.signer_role != "tenant_owner"
                || operation.signer_pubkey != *owner_pubkey
            {
                return Err(CommunityProvisioningError::Invalid(format!(
                    "invalid group seed operation for {}",
                    operation.group_id
                )));
            }
            if !operation.unsigned_event_tags.iter().any(|tag| {
                tag.first().map(String::as_str) == Some("h")
                    && tag.get(1) == Some(&operation.group_id)
            }) || !operation.unsigned_event_tags.iter().any(|tag| {
                tag.first().map(String::as_str) == Some("name")
                    && tag.get(1) == Some(&operation.group_display_name)
            }) {
                return Err(CommunityProvisioningError::Invalid(format!(
                    "group seed operation tags do not match {}",
                    operation.group_id
                )));
            }
        }
        Ok(())
    }

    pub fn render_tangle_tenant_config(
        &self,
        relay_secret_hex: &str,
    ) -> Result<Value, CommunityProvisioningError> {
        self.validate()?;
        if !is_lower_hex_64(relay_secret_hex) {
            return Err(CommunityProvisioningError::Invalid(
                "relay self key secret must be lowercase hex".to_owned(),
            ));
        }
        Ok(json!({
            "tenant_id": self.tenant.tenant_id,
            "tenant_schema": self.tenant.tenant_schema,
            "host": self.tenant.canonical_host,
            "relay_url": self.tenant.relay_url,
            "inactive": false,
            "info": {
                "name": self.tenant.display_name,
                "description": self.tenant.description
            },
            "pocket": {
                "data_directory": self.pocket_store.data_directory,
                "sync_policy": "flush_on_shutdown"
            },
            "pocket_query": {
                "allow_scraping": false,
                "allow_scrape_if_limited_to": 100,
                "allow_scrape_if_max_seconds": 3600
            },
            "groups": {
                "enabled": self.groups.enabled,
                "canonical_relay_url": self.groups.canonical_relay_url,
                "relay_secret": relay_secret_hex,
                "owner_pubkeys": self.groups.owner_pubkeys,
                "admin_pubkeys": self.groups.admin_pubkeys,
                "policy": {
                    "public_join": self.groups.policy.public_join,
                    "invites_enabled": self.groups.policy.invites_enabled
                },
                "limits": {
                    "max_group_id_bytes": self.groups.limits.max_group_id_bytes,
                    "max_group_tags_per_event": self.groups.limits.max_group_tags_per_event,
                    "max_supported_kinds": self.groups.limits.max_supported_kinds,
                    "max_member_list_pubkeys": self.groups.limits.max_member_list_pubkeys,
                    "max_outbox_replay_batch": self.groups.limits.max_outbox_replay_batch
                }
            },
            "backup_export": {
                "backup_enabled": self.backup_export.backup_enabled,
                "export_enabled": self.backup_export.export_enabled
            },
            "auth": {
                "challenge_ttl_seconds": 300,
                "created_at_skew_seconds": 600
            },
            "limits": {
                "max_message_length": 1048576,
                "max_subid_length": 64,
                "max_subscriptions_per_connection": 64,
                "max_filters_per_request": 10,
                "max_tag_values_per_filter": 100,
                "max_query_complexity": 2048,
                "max_limit": 500,
                "default_limit": 100,
                "max_event_tags": 200,
                "max_content_length": 65536,
                "broadcast_channel_capacity": 8,
                "per_connection_outbound_queue": 8
            },
            "rate_limits": {
                "auth": {
                    "per_ip": {"window_seconds": 60, "max_hits": 120},
                    "per_pubkey": {"window_seconds": 60, "max_hits": 30},
                    "failures": {"window_seconds": 300, "max_hits": 5},
                    "failures_per_ip": {"window_seconds": 300, "max_hits": 20}
                },
                "event": {
                    "per_ip": {"window_seconds": 60, "max_hits": 600},
                    "per_pubkey": {"window_seconds": 60, "max_hits": 120},
                    "per_kind": {"window_seconds": 60, "max_hits": 1000}
                },
                "group": {
                    "write_per_ip": {"window_seconds": 60, "max_hits": 300},
                    "write_per_pubkey": {"window_seconds": 60, "max_hits": 60},
                    "write_per_group": {"window_seconds": 60, "max_hits": 90},
                    "write_per_kind": {"window_seconds": 60, "max_hits": 300},
                    "join_flow": {"window_seconds": 300, "max_hits": 10},
                    "join_flow_per_ip": {"window_seconds": 300, "max_hits": 30}
                },
                "req": {
                    "per_ip": {"window_seconds": 60, "max_hits": 600},
                    "per_connection": {"window_seconds": 60, "max_hits": 120},
                    "per_pubkey": {"window_seconds": 60, "max_hits": 240},
                    "per_group": {"window_seconds": 60, "max_hits": 240},
                    "per_kind": {"window_seconds": 60, "max_hits": 500},
                    "broad": {"window_seconds": 60, "max_hits": 30}
                },
                "count": {
                    "per_ip": {"window_seconds": 60, "max_hits": 300},
                    "per_connection": {"window_seconds": 60, "max_hits": 60},
                    "per_pubkey": {"window_seconds": 60, "max_hits": 120},
                    "per_group": {"window_seconds": 60, "max_hits": 120},
                    "per_kind": {"window_seconds": 60, "max_hits": 240},
                    "broad": {"window_seconds": 60, "max_hits": 20}
                }
            }
        }))
    }

    pub fn validate_against_schema(
        &self,
        schema: &Value,
    ) -> Result<(), CommunityProvisioningError> {
        validate_contract_schema(schema, &serde_json::to_value(self)?)?;
        self.validate()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningHostConfigUpdate {
    pub tenant_config_dir: String,
    pub tenant_config_file: String,
    pub apply_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningTenantOutput {
    pub tenant_id: String,
    pub tenant_schema: String,
    pub canonical_host: String,
    pub relay_url: String,
    pub display_name: String,
    pub description: String,
    pub discoverability: DiscoverabilityMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningPocketStoreOutput {
    pub data_directory: String,
    pub create_if_missing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningRelaySelfKeyOutput {
    pub mode: String,
    pub secret_ref: String,
    pub materialized_config_field: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningGroupsOutput {
    pub enabled: bool,
    pub canonical_relay_url: String,
    pub owner_pubkeys: Vec<String>,
    pub admin_pubkeys: Vec<String>,
    pub policy: ProvisioningGroupPolicyOutput,
    pub limits: ProvisioningGroupLimitsOutput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningGroupPolicyOutput {
    pub public_join: bool,
    pub invites_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningGroupLimitsOutput {
    pub max_group_id_bytes: u64,
    pub max_group_tags_per_event: u64,
    pub max_supported_kinds: u64,
    pub max_member_list_pubkeys: u64,
    pub max_outbox_replay_batch: u64,
}

impl Default for ProvisioningGroupLimitsOutput {
    fn default() -> Self {
        Self {
            max_group_id_bytes: 128,
            max_group_tags_per_event: 8,
            max_supported_kinds: 512,
            max_member_list_pubkeys: 100_000,
            max_outbox_replay_batch: 1_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningGroupSeedOutput {
    pub apply_mode: String,
    pub operations: Vec<ProvisioningGroupSeedOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningGroupSeedOperation {
    pub community_space_id: String,
    pub tenant_id: String,
    pub tenant_relay_url: String,
    pub group_id: String,
    pub group_display_name: String,
    pub group_description: String,
    pub event_kind: u64,
    pub unsigned_event_tags: Vec<Vec<String>>,
    pub signer_role: String,
    pub signer_pubkey: String,
    pub apply_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningRedactionOutput {
    pub redact_fields: Vec<String>,
    pub public_report_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningValidationReport {
    pub adapter_input_schema: String,
    pub adapter_output_schema: String,
    pub unsupported_fields_policy: UnsupportedFieldsPolicy,
    pub tangle_config_validate_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningOperatorApply {
    pub instruction: String,
    pub hot_reload: bool,
    pub remote_management_route: bool,
}

fn group_seed_output(input: &CommunityProvisioningAdapterInput) -> ProvisioningGroupSeedOutput {
    ProvisioningGroupSeedOutput {
        apply_mode: GROUP_SEED_APPLY_MODE.to_owned(),
        operations: input
            .groups
            .initial_groups
            .iter()
            .map(|group| ProvisioningGroupSeedOperation {
                community_space_id: input.community_space_id.clone(),
                tenant_id: input.tenant.tenant_id.clone(),
                tenant_relay_url: input.tenant.relay_url.clone(),
                group_id: group.group_id.clone(),
                group_display_name: group.name.clone(),
                group_description: group.description.clone(),
                event_kind: GROUP_CREATE_KIND,
                unsigned_event_tags: vec![
                    vec!["h".to_owned(), group.group_id.clone()],
                    vec!["name".to_owned(), group.name.clone()],
                ],
                signer_role: "tenant_owner".to_owned(),
                signer_pubkey: input.tenant.owner_pubkey.clone(),
                apply_mode: GROUP_SEED_APPLY_MODE.to_owned(),
            })
            .collect(),
    }
}

fn is_lower_hex_64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn materialized_output_is_narrow_and_renderable() {
        let input = sample_input();
        let output = input.materialize_output();
        let value = serde_json::to_value(&output).expect("value");

        output.validate().expect("valid output");
        assert!(value.get("tenant_config_json").is_none());
        assert_eq!(output.tenant.tenant_id, input.tenant.tenant_id);
        assert_eq!(
            output.groups.owner_pubkeys,
            vec![input.tenant.owner_pubkey.clone()]
        );

        let rendered = output
            .render_tangle_tenant_config(&input.relay_self_key.fixture_secret_hex)
            .expect("tenant config");

        assert_eq!(rendered["tenant_id"], output.tenant.tenant_id);
        assert_eq!(
            rendered["groups"]["relay_secret"],
            input.relay_self_key.fixture_secret_hex
        );
    }

    #[test]
    fn rendering_rejects_invalid_secret_material() {
        let output = sample_input().materialize_output();

        let error = output
            .render_tangle_tenant_config("not-a-secret")
            .expect_err("secret error");

        assert!(error.to_string().contains("lowercase hex"));
    }

    #[test]
    fn output_validation_rejects_remote_management() {
        let mut output = sample_input().materialize_output();
        output.operator_apply.remote_management_route = true;

        let error = output.validate().expect_err("validation error");

        assert!(error.to_string().contains("remote management"));
    }

    #[test]
    fn output_validation_rejects_group_seed_mismatch() {
        let mut output = sample_input().materialize_output();
        output.group_seed.operations[0].tenant_relay_url = "ws://wrong.tangle.local".to_owned();

        let error = output.validate().expect_err("validation error");

        assert!(error.to_string().contains("invalid group seed operation"));
    }

    #[test]
    fn schema_validation_can_wrap_typed_output() {
        let output = sample_input().materialize_output();
        let schema = json!({
            "type": "object",
            "required": ["schema", "community_space_id", "tenant"],
            "properties": {
                "schema": {"const": ADAPTER_OUTPUT_SCHEMA},
                "community_space_id": {"type": "string"},
                "tenant": {
                    "type": "object",
                    "required": ["tenant_id"],
                    "properties": {
                        "tenant_id": {"type": "string"}
                    },
                    "additionalProperties": true
                }
            },
            "additionalProperties": true
        });

        output.validate_against_schema(&schema).expect("schema");
    }

    fn sample_input() -> CommunityProvisioningAdapterInput {
        CommunityProvisioningAdapterInput {
            schema: ADAPTER_INPUT_SCHEMA.to_owned(),
            community_space_id: "vancouver_local_food_association".to_owned(),
            display_name: "Vancouver Local Food Association".to_owned(),
            description: "Tangle virtual relay tenant for a Vancouver local food association."
                .to_owned(),
            discoverability: DiscoverabilityMode::Public,
            tenant: ProvisioningTenantInput {
                tenant_id: "vancouver_local_food_association".to_owned(),
                tenant_schema: "vancouver_local_food_association".to_owned(),
                canonical_host: "vancouver-local-food-association.tangle.local".to_owned(),
                relay_url: "ws://vancouver-local-food-association.tangle.local".to_owned(),
                owner_pubkey: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798"
                    .to_owned(),
                admin_pubkeys: vec![
                    "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5".to_owned(),
                ],
            },
            groups: ProvisioningGroupInput {
                enabled: true,
                public_join: false,
                invites_enabled: false,
                initial_groups: vec![ProvisioningInitialGroupInput {
                    group_id: "market".to_owned(),
                    name: "Market Coordination".to_owned(),
                    description: "Room for weekly market coordination and local food updates."
                        .to_owned(),
                }],
            },
            backup_export: ProvisioningBackupExportInput {
                backup_enabled: true,
                export_enabled: true,
            },
            relay_self_key: ProvisioningRelaySelfKeyInput {
                mode: "operator_supplied_secret".to_owned(),
                secret_ref: "tangle/community/vancouver_local_food_association/relay_self"
                    .to_owned(),
                fixture_secret_hex:
                    "9999999999999999999999999999999999999999999999999999999999999999".to_owned(),
            },
            custom_domain: None,
            unsupported_fields_policy: UnsupportedFieldsPolicy::Reject,
        }
    }
}
