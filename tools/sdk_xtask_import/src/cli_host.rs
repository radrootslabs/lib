use std::path::{Path, PathBuf};

use radroots_runtime_contract_v1::{RUNTIME_OPERATION_DESCRIPTORS_V1, RuntimeOperationIdV1};

use crate::fs::{workspace_root, write_if_changed};

struct CliOperationMetadata {
    operation_id: &'static str,
    variant: &'static str,
    cli_path: &'static str,
    description: &'static str,
    role: &'static str,
    supports_ndjson: bool,
}

const CLI_OPERATION_METADATA: &[CliOperationMetadata] = &[
    meta(
        "profile.inspect",
        "ProfileInspect",
        "radroots profile inspect",
        "Inspect the active runtime profile.",
        "Any",
        false,
    ),
    meta(
        "profile.reset",
        "ProfileReset",
        "radroots profile reset",
        "Reset the active runtime profile stores.",
        "Any",
        false,
    ),
    meta(
        "account.create",
        "AccountCreate",
        "radroots account create",
        "Create a local account identity.",
        "Any",
        false,
    ),
    meta(
        "account.import",
        "AccountImport",
        "radroots account import",
        "Import an existing account identity.",
        "Any",
        false,
    ),
    meta(
        "account.select",
        "AccountSelect",
        "radroots account select <selector>",
        "Select the active account context.",
        "Any",
        false,
    ),
    meta(
        "account.list",
        "AccountList",
        "radroots account list",
        "List known local accounts.",
        "Any",
        true,
    ),
    meta(
        "account.remove",
        "AccountRemove",
        "radroots account remove <selector>",
        "Remove an account from local configuration and storage.",
        "Any",
        false,
    ),
    meta(
        "signer.status",
        "SignerStatus",
        "radroots signer status",
        "Inspect signer readiness.",
        "Any",
        false,
    ),
    meta(
        "store.inspect",
        "StoreInspect",
        "radroots store inspect",
        "Inspect local runtime stores.",
        "Any",
        false,
    ),
    meta(
        "store.backup",
        "StoreBackup",
        "radroots store backup",
        "Create a local runtime-store backup.",
        "Any",
        false,
    ),
    meta(
        "store.restore",
        "StoreRestore",
        "radroots store restore <source>",
        "Restore local runtime stores from a backup.",
        "Any",
        false,
    ),
    meta(
        "farm.create",
        "FarmCreate",
        "radroots farm create",
        "Create farm profile data.",
        "Seller",
        false,
    ),
    meta(
        "farm.update",
        "FarmUpdate",
        "radroots farm update",
        "Update farm profile data.",
        "Seller",
        false,
    ),
    meta(
        "farm.publish",
        "FarmPublish",
        "radroots farm publish",
        "Publish farm profile data.",
        "Seller",
        false,
    ),
    meta(
        "farm.get",
        "FarmGet",
        "radroots farm get",
        "Get farm profile data.",
        "Any",
        false,
    ),
    meta(
        "farm.list",
        "FarmList",
        "radroots farm list",
        "List farm profile data.",
        "Any",
        true,
    ),
    meta(
        "listing.create",
        "ListingCreate",
        "radroots listing create",
        "Create a listing draft.",
        "Seller",
        false,
    ),
    meta(
        "listing.update",
        "ListingUpdate",
        "radroots listing update <file>",
        "Update a listing.",
        "Seller",
        false,
    ),
    meta(
        "listing.publish",
        "ListingPublish",
        "radroots listing publish <file>",
        "Publish a listing.",
        "Seller",
        false,
    ),
    meta(
        "listing.pause",
        "ListingPause",
        "radroots listing pause <file>",
        "Pause a listing.",
        "Seller",
        false,
    ),
    meta(
        "listing.withdraw",
        "ListingWithdraw",
        "radroots listing withdraw <file>",
        "Withdraw a listing.",
        "Seller",
        false,
    ),
    meta(
        "listing.get",
        "ListingGet",
        "radroots listing get <key>",
        "Get a listing.",
        "Any",
        false,
    ),
    meta(
        "listing.list",
        "ListingList",
        "radroots listing list",
        "List listings.",
        "Any",
        true,
    ),
    meta(
        "market.pull",
        "MarketPull",
        "radroots market pull",
        "Pull market data into the local projection.",
        "Any",
        true,
    ),
    meta(
        "market.search",
        "MarketSearch",
        "radroots market search <query>",
        "Search local market projections.",
        "Any",
        true,
    ),
    meta(
        "market.get",
        "MarketGet",
        "radroots market get <key>",
        "Get market listing details.",
        "Any",
        false,
    ),
    meta(
        "basket.create",
        "BasketCreate",
        "radroots basket create",
        "Create a basket.",
        "Buyer",
        false,
    ),
    meta(
        "basket.get",
        "BasketGet",
        "radroots basket get <basket-id>",
        "Get a basket.",
        "Buyer",
        false,
    ),
    meta(
        "basket.list",
        "BasketList",
        "radroots basket list",
        "List baskets.",
        "Buyer",
        true,
    ),
    meta(
        "basket.item.add",
        "BasketItemAdd",
        "radroots basket item add <basket-id>",
        "Add an item to a basket.",
        "Buyer",
        false,
    ),
    meta(
        "basket.item.update",
        "BasketItemUpdate",
        "radroots basket item update <basket-id>",
        "Update a basket item.",
        "Buyer",
        false,
    ),
    meta(
        "basket.item.remove",
        "BasketItemRemove",
        "radroots basket item remove <basket-id>",
        "Remove a basket item.",
        "Buyer",
        false,
    ),
    meta(
        "basket.quote",
        "BasketQuote",
        "radroots basket quote <basket-id>",
        "Quote a basket.",
        "Buyer",
        false,
    ),
    meta(
        "trade.request",
        "TradeRequest",
        "radroots trade request",
        "Request a trade from a quoted basket.",
        "Buyer",
        false,
    ),
    meta(
        "trade.get",
        "TradeGet",
        "radroots trade get <key>",
        "Get trade details.",
        "Any",
        false,
    ),
    meta(
        "trade.list",
        "TradeList",
        "radroots trade list",
        "List trades.",
        "Any",
        true,
    ),
    meta(
        "trade.accept",
        "TradeAccept",
        "radroots trade accept <key>",
        "Accept a trade request.",
        "Seller",
        false,
    ),
    meta(
        "trade.decline",
        "TradeDecline",
        "radroots trade decline <key>",
        "Decline a trade request.",
        "Seller",
        false,
    ),
    meta(
        "trade.cancel",
        "TradeCancel",
        "radroots trade cancel <key>",
        "Cancel a trade request.",
        "Buyer",
        false,
    ),
    meta(
        "validation.status",
        "ValidationStatus",
        "radroots validation status",
        "Inspect validation status.",
        "Any",
        false,
    ),
    meta(
        "validation.receipt.get",
        "ValidationReceiptGet",
        "radroots validation receipt get <receipt-event-id>",
        "Get a validation receipt.",
        "Any",
        false,
    ),
    meta(
        "validation.receipt.verify",
        "ValidationReceiptVerify",
        "radroots validation receipt verify <receipt-event-id>",
        "Verify a validation receipt.",
        "Any",
        false,
    ),
    meta(
        "sync.status",
        "SyncStatus",
        "radroots sync status",
        "Inspect synchronization status.",
        "Any",
        false,
    ),
    meta(
        "sync.pull",
        "SyncPull",
        "radroots sync pull",
        "Pull synchronization events.",
        "Any",
        true,
    ),
    meta(
        "sync.push",
        "SyncPush",
        "radroots sync push",
        "Push queued synchronization events.",
        "Any",
        true,
    ),
    meta(
        "health.inspect",
        "HealthInspect",
        "radroots health inspect",
        "Inspect runtime health.",
        "Any",
        false,
    ),
    meta(
        "transport.capability.list",
        "TransportCapabilityList",
        "radroots transport capability list",
        "List transport capabilities.",
        "Any",
        false,
    ),
    meta(
        "transport.config.inspect",
        "TransportConfigInspect",
        "radroots transport config inspect",
        "Inspect transport configuration.",
        "Any",
        false,
    ),
    meta(
        "transport.config.update",
        "TransportConfigUpdate",
        "radroots transport config update --kind local-only",
        "Update transport configuration.",
        "Any",
        false,
    ),
    meta(
        "transport.status.inspect",
        "TransportStatusInspect",
        "radroots transport status inspect",
        "Inspect transport status.",
        "Any",
        false,
    ),
    meta(
        "transport.delivery.inspect",
        "TransportDeliveryInspect",
        "radroots transport delivery inspect",
        "Inspect transport delivery.",
        "Any",
        false,
    ),
    meta(
        "transport.delivery.retry",
        "TransportDeliveryRetry",
        "radroots transport delivery retry",
        "Retry transport delivery.",
        "Any",
        true,
    ),
    meta(
        "diagnostics.inspect",
        "DiagnosticsInspect",
        "radroots diagnostics inspect",
        "Inspect runtime diagnostics.",
        "Any",
        false,
    ),
];

const fn meta(
    operation_id: &'static str,
    variant: &'static str,
    cli_path: &'static str,
    description: &'static str,
    role: &'static str,
    supports_ndjson: bool,
) -> CliOperationMetadata {
    CliOperationMetadata {
        operation_id,
        variant,
        cli_path,
        description,
        role,
        supports_ndjson,
    }
}

pub fn generate_cli_host() -> Result<(), String> {
    let root = workspace_root()?;
    let cli_root = cli_root(&root);
    let registry = render_registry()?;
    let target = render_target()?;
    write_if_changed(
        &cli_root.join("src/generated/runtime_contract_registry.rs"),
        &registry,
    )?;
    write_if_changed(
        &cli_root.join("src/generated/runtime_contract_target.rs"),
        &target,
    )?;
    println!("generated CLI Runtime Contract V1 host bindings");
    Ok(())
}

pub fn check_cli_host() -> Result<(), String> {
    let root = workspace_root()?;
    let cli_root = cli_root(&root);
    check_file(
        &cli_root.join("src/generated/runtime_contract_registry.rs"),
        &render_registry()?,
    )?;
    check_file(
        &cli_root.join("src/generated/runtime_contract_target.rs"),
        &render_target()?,
    )
}

fn cli_root(sdk_root: &Path) -> PathBuf {
    sdk_root.join("../cli")
}

fn check_file(path: &Path, expected: &str) -> Result<(), String> {
    let actual = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    if actual != expected {
        return Err(format!(
            "{} is stale; run `cargo xtask generate` from the SDK workspace",
            path.display()
        ));
    }
    Ok(())
}

fn render_registry() -> Result<String, String> {
    validate_metadata()?;
    let mut out = generated_header();
    out.push_str("pub const OPERATION_REGISTRY: &[OperationSpec] = &[\n");
    for (index, descriptor) in RUNTIME_OPERATION_DESCRIPTORS_V1.iter().enumerate() {
        let metadata = metadata_for(descriptor.operation_id)?;
        out.push_str("    OperationSpec {\n");
        out.push_str(&format!(
            "        runtime_operation_id: RuntimeOperationIdV1::{},\n",
            metadata.variant
        ));
        out.push_str(&format!(
            "        descriptor: radroots_runtime_contract_v1::RUNTIME_OPERATION_DESCRIPTORS_V1[{index}],\n"
        ));
        out.push_str(&format!(
            "        operation_id: {:?},\n",
            metadata.operation_id
        ));
        out.push_str(&format!("        cli_path: {:?},\n", metadata.cli_path));
        out.push_str(&format!(
            "        namespace: {:?},\n",
            namespace(metadata.operation_id)
        ));
        out.push_str(&format!(
            "        mcp_tool: {:?},\n",
            mcp_tool(metadata.operation_id)
        ));
        out.push_str(&format!(
            "        rust_request: {:?},\n",
            format!("{}Request", metadata.variant)
        ));
        out.push_str(&format!(
            "        rust_result: {:?},\n",
            format!("{}Result", metadata.variant)
        ));
        out.push_str(&format!(
            "        json_kind: {:?},\n",
            metadata.operation_id
        ));
        out.push_str(&format!(
            "        description: {:?},\n",
            metadata.description
        ));
        out.push_str(&format!(
            "        role: OperationRole::{},\n",
            metadata.role
        ));
        out.push_str("        supports_json: true,\n");
        out.push_str(&format!(
            "        supports_ndjson: {},\n",
            metadata.supports_ndjson
        ));
        out.push_str("    },\n");
    }
    out.push_str("];\n");
    Ok(out)
}

fn render_target() -> Result<String, String> {
    validate_metadata()?;
    let mut out = generated_header();
    out.push_str("target_operation_contracts! {\n");
    for descriptor in RUNTIME_OPERATION_DESCRIPTORS_V1 {
        let metadata = metadata_for(descriptor.operation_id)?;
        let type_name = metadata.variant;
        out.push_str(&format!(
            "    {} => ({}Request, {}Result, {:?}),\n",
            type_name, type_name, type_name, metadata.operation_id
        ));
    }
    out.push_str("}\n");
    Ok(out)
}

fn generated_header() -> String {
    "// @generated by radroots_sdk_xtask. Do not edit by hand.\n\n".to_owned()
}

fn validate_metadata() -> Result<(), String> {
    if CLI_OPERATION_METADATA.len() != RUNTIME_OPERATION_DESCRIPTORS_V1.len() {
        return Err(format!(
            "CLI metadata count {} does not match runtime descriptor count {}",
            CLI_OPERATION_METADATA.len(),
            RUNTIME_OPERATION_DESCRIPTORS_V1.len()
        ));
    }
    for descriptor in RUNTIME_OPERATION_DESCRIPTORS_V1 {
        metadata_for(descriptor.operation_id)?;
    }
    for metadata in CLI_OPERATION_METADATA {
        RuntimeOperationIdV1::parse(metadata.operation_id)
            .map_err(|error| format!("invalid CLI metadata operation id: {error}"))?;
    }
    Ok(())
}

fn metadata_for(
    operation_id: RuntimeOperationIdV1,
) -> Result<&'static CliOperationMetadata, String> {
    let operation_id = operation_id.as_str();
    CLI_OPERATION_METADATA
        .iter()
        .find(|metadata| metadata.operation_id == operation_id)
        .ok_or_else(|| format!("missing CLI metadata for runtime operation {operation_id}"))
}

fn namespace(operation_id: &str) -> &str {
    operation_id.split('.').next().unwrap_or(operation_id)
}

fn mcp_tool(operation_id: &str) -> String {
    operation_id.replace('.', "_")
}
