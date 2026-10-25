use super::*;

#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum MovementKind {
    Introduced,
    Removed,
    Wasted,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Movement {
    pub(super) version: u8,
    pub(super) operation: OperationId,
    pub(super) digest: CommandDigest,
    pub(super) pool: PoolId,
    pub(super) quantity: radroots_domain::stock::StockTicks,
    pub(super) kind: MovementKind,
}
#[derive(Serialize)]
pub(super) struct OperationWire<'a> {
    pub(super) version: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) before: Option<Option<PoolSnapshot>>,
    pub(super) selection: &'a AttachmentInput,
    pub(super) scope: WorkspaceScope,
    pub(super) principal: &'a str,
    pub(super) grant_revision: Sequence,
    pub(super) grant_expires_at: UnixSeconds,
    pub(super) observed_at: UnixSeconds,
    pub(super) minimum_revision: Sequence,
    pub(super) minimum_head: Option<CheckpointDigest>,
    pub(super) transaction: TransactionId,
    pub(super) revision: Sequence,
    pub(super) previous_head: Option<CheckpointDigest>,
    pub(super) head: CheckpointDigest,
    pub(super) command: &'a RawValue,
    pub(super) pool: PoolSnapshot,
    pub(super) movement: Option<[u8; 32]>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OperationDecode<'a> {
    pub(super) version: u8,
    pub(super) selection: AttachmentInput,
    pub(super) scope: WorkspaceScope,
    pub(super) principal: String,
    pub(super) grant_revision: Sequence,
    pub(super) grant_expires_at: UnixSeconds,
    pub(super) observed_at: UnixSeconds,
    pub(super) minimum_revision: Sequence,
    #[serde(deserialize_with = "super::required_nullable")]
    pub(super) minimum_head: Option<CheckpointDigest>,
    pub(super) transaction: TransactionId,
    pub(super) revision: Sequence,
    #[serde(deserialize_with = "super::required_nullable")]
    pub(super) previous_head: Option<CheckpointDigest>,
    pub(super) head: CheckpointDigest,
    #[serde(borrow)]
    pub(super) command: &'a RawValue,
    pub(super) pool: PoolSnapshot,
    #[serde(deserialize_with = "super::required_nullable")]
    pub(super) movement: Option<[u8; 32]>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OperationDecodeV2<'a> {
    pub(super) version: u8,
    #[serde(deserialize_with = "super::required_nullable")]
    pub(super) before: Option<PoolSnapshot>,
    pub(super) selection: AttachmentInput,
    pub(super) scope: WorkspaceScope,
    pub(super) principal: String,
    pub(super) grant_revision: Sequence,
    pub(super) grant_expires_at: UnixSeconds,
    pub(super) observed_at: UnixSeconds,
    pub(super) minimum_revision: Sequence,
    #[serde(deserialize_with = "super::required_nullable")]
    pub(super) minimum_head: Option<CheckpointDigest>,
    pub(super) transaction: TransactionId,
    pub(super) revision: Sequence,
    #[serde(deserialize_with = "super::required_nullable")]
    pub(super) previous_head: Option<CheckpointDigest>,
    pub(super) head: CheckpointDigest,
    #[serde(borrow)]
    pub(super) command: &'a RawValue,
    pub(super) pool: PoolSnapshot,
    #[serde(deserialize_with = "super::required_nullable")]
    pub(super) movement: Option<[u8; 32]>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationV3<'a> {
    version: u8,
    authorization: StockAuthorizationRef,
    #[serde(borrow)]
    operation: &'a RawValue,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationV4<'a> {
    version: u8,
    authorization: StockAuthorizationRef,
    #[serde(borrow)]
    operation: &'a RawValue,
    #[serde(deserialize_with = "super::required_nullable")]
    mapping: Option<super::mapping::MappingEffect>,
}
pub(super) fn encode_operation(
    value: &OperationWire<'_>,
    catalog: i64,
    authorization: Option<StockAuthorizationRef>,
    mapping: Option<super::mapping::MappingEffect>,
) -> Result<zeroize::Zeroizing<Vec<u8>>, OperationError> {
    let clear = super::encode(value)?;
    if catalog < 4 {
        return Ok(clear);
    }
    let raw = RawValue::from_string(
        String::from_utf8(clear.to_vec()).map_err(|_| OperationError::StorageFailure)?,
    )
    .map_err(|_| OperationError::StorageFailure)?;
    if matches!(catalog, 5..=7) {
        return super::encode(&OperationV4 {
            version: 4,
            authorization: authorization.ok_or(OperationError::RecoveryRequired)?,
            operation: &raw,
            mapping,
        });
    }
    super::encode(&OperationV3 {
        version: 3,
        authorization: authorization.ok_or(OperationError::RecoveryRequired)?,
        operation: &raw,
    })
}
pub(super) struct OperationData {
    pub(super) mapping: Option<super::mapping::MappingEffect>,
    pub(super) authorization: Option<StockAuthorizationRef>,
    pub(super) version: u8,
    pub(super) before: Option<PoolSnapshot>,
    pub(super) transaction: TransactionId,
    pub(super) revision: Sequence,
    pub(super) previous_head: Option<CheckpointDigest>,
    pub(super) head: CheckpointDigest,
    pub(super) pool: PoolSnapshot,
    pub(super) movement: Option<[u8; 32]>,
}

pub(super) fn decode_operation(
    bytes: &[u8],
    catalog: i64,
) -> Result<(InventoryRequest, OperationData), OperationError> {
    decode_inner(bytes, catalog, false)
}
fn decode_inner(
    bytes: &[u8],
    catalog: i64,
    allow_mapping: bool,
) -> Result<(InventoryRequest, OperationData), OperationError> {
    if matches!(catalog, 5..=7) {
        let decoded: OperationV4<'_> =
            super::closed_record(bytes).map_err(|_| OperationError::RecoveryRequired)?;
        if decoded.version != 4 {
            return Err(OperationError::RecoveryRequired);
        }
        let (request, mut data) = decode_inner(decoded.operation.get().as_bytes(), 3, true)?;
        data.authorization = Some(decoded.authorization);
        data.mapping = decoded.mapping;
        return Ok((request, data));
    }
    if catalog == 4 {
        let decoded: OperationV3<'_> =
            super::closed_record(bytes).map_err(|_| OperationError::RecoveryRequired)?;
        if decoded.version != 3 {
            return Err(OperationError::RecoveryRequired);
        }
        let (request, mut data) = decode_inner(decoded.operation.get().as_bytes(), 3, false)?;
        data.authorization = Some(decoded.authorization);
        return Ok((request, data));
    }
    macro_rules! captured {
        ($decoded:expr,$prior:expr) => {{
            let decoded = $decoded;
            let prior = $prior;
            let captured = WorkspaceBinding::from_authenticated_host(
                decoded.selection,
                decoded.scope,
                radroots_storage::inventory::CapturedInventoryCommand::parse(
                    decoded.command.get().as_bytes(),
                )
                .map_err(|_| OperationError::RecoveryRequired)?
                .context()
                .actor(),
                decoded.principal,
                decoded.grant_revision,
                decoded.grant_expires_at,
                decoded.observed_at,
            )?;
            let minimum = radroots_storage::inventory::InventoryCheckpointRef::from_host_metadata(
                captured.input().authority(),
                decoded.minimum_revision,
                decoded.minimum_head,
            )?;
            let request = InventoryRequest::from_protected_wire(
                captured,
                minimum,
                decoded.command.get().as_bytes(),
            )?;
            if !allow_mapping && request.command().mapping_input().is_some() {
                return Err(OperationError::RecoveryRequired);
            }
            Ok((
                request,
                OperationData {
                    mapping: None,
                    authorization: None,
                    version: decoded.version,
                    before: prior,
                    transaction: decoded.transaction,
                    revision: decoded.revision,
                    previous_head: decoded.previous_head,
                    head: decoded.head,
                    pool: decoded.pool,
                    movement: decoded.movement,
                },
            ))
        }};
    }
    match catalog {
        2 => {
            let decoded: OperationDecode<'_> =
                super::closed_record(bytes).map_err(|_| OperationError::RecoveryRequired)?;
            captured!(decoded, None)
        }
        3 => {
            let decoded: OperationDecodeV2<'_> =
                super::closed_record(bytes).map_err(|_| OperationError::RecoveryRequired)?;
            let prior = decoded.before;
            captured!(decoded, prior)
        }
        _ => Err(OperationError::RecoveryRequired),
    }
}
