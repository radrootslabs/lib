//! Owner-private configuration on the existing stock transaction owner.
use super::*;
use radroots_domain::stock::{OptionMappingKey, OptionPoolMapping};

#[derive(Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MappingEffect {
    pub before: Option<OptionPoolMapping>,
    pub after: OptionPoolMapping,
}
impl<'de> Deserialize<'de> for MappingEffect {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            #[serde(deserialize_with = "super::super::required_nullable")]
            before: Option<OptionPoolMapping>,
            after: OptionPoolMapping,
        }
        let value: Wire = super::super::object(deserializer)?;
        Ok(Self {
            before: value.before,
            after: value.after,
        })
    }
}
pub(super) struct RetainedMapping {
    pub value: OptionPoolMapping,
    pub protected: Vec<u8>,
}
pub(super) struct MappingRead {
    pub key: OptionMappingKey,
    pub retained: Option<RetainedMapping>,
    pub proof: MinimumProof,
}
impl MappingRead {
    pub async fn confirm(&self, connection: &mut SqliteConnection) -> Result<(), OperationError> {
        self.proof.confirm(connection).await?;
        let key = self.key;
        let protected:Option<Vec<u8>>=sqlx::query_scalar("SELECT CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_mappings WHERE listing_author=? AND listing_d=? AND option_id=?")
            .bind(key.listing().author().as_bytes().as_slice()).bind(key.listing().d().as_bytes().as_slice()).bind(key.option_id().as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.flatten();
        if protected.as_ref() != self.retained.as_ref().map(|value| &value.protected) {
            return Err(OperationError::RecoveryRequired);
        }
        if let Some(retained) = &self.retained {
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_mappings WHERE listing_author=? AND listing_d=? AND option_id=? AND pool_id=? AND revision=?")
                .bind(key.listing().author().as_bytes().as_slice()).bind(key.listing().d().as_bytes().as_slice()).bind(key.option_id().as_bytes().as_slice()).bind(retained.value.pool_id().as_bytes().as_slice()).bind(retained.value.version().get() as i64).fetch_one(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
            if count != 1 {
                return Err(OperationError::RecoveryRequired);
            }
        } else {
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_mappings WHERE listing_author=? AND listing_d=? AND option_id=?").bind(key.listing().author().as_bytes().as_slice()).bind(key.listing().d().as_bytes().as_slice()).bind(key.option_id().as_bytes().as_slice()).fetch_one(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
            if count != 0 {
                return Err(OperationError::RecoveryRequired);
            }
        }
        Ok(())
    }
}
pub(super) fn require(access: &Access, key: OptionMappingKey) -> Result<(), OperationError> {
    if !(5..=9).contains(&access.authority.catalog) {
        return Err(OperationError::UnsupportedOperation);
    }
    if !access.record.administrator()
        || key.listing().author() != access.current.actor().participant()
    {
        return Err(OperationError::Unauthorized);
    }
    Ok(())
}
pub(super) struct MappingPlan {
    pub effect: MappingEffect,
    pub protected: Vec<u8>,
    pub original: Option<Vec<u8>>,
    pub proof: MinimumProof,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MappingWire {
    version: u8,
    mapping: OptionPoolMapping,
}
pub(super) fn key_id(key: OptionMappingKey) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.mapping_key.v1\0");
    hash.update(key.listing().author().as_bytes());
    hash.update(key.listing().d().as_bytes());
    hash.update(key.option_id().as_bytes());
    hash.finalize().into()
}
pub(super) async fn read(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    key: OptionMappingKey,
) -> Result<Option<RetainedMapping>, OperationError> {
    let row=sqlx::query("SELECT pool_id,revision,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_mappings WHERE listing_author=? AND listing_d=? AND option_id=?")
        .bind(key.listing().author().as_bytes().as_slice()).bind(key.listing().d().as_bytes().as_slice()).bind(key.option_id().as_bytes().as_slice()).fetch_optional(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let Some(row) = row else { return Ok(None) };
    let protected: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(2)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let clear = crypto
        .open_stock_record(input, "mapping", &key_id(key), &protected)
        .await?;
    let wire: MappingWire = clear
        .expose_secret(|bytes| super::super::closed_record(bytes))
        .map_err(|_| OperationError::RecoveryRequired)?;
    let pool: Vec<u8> = row
        .try_get(0)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let revision: i64 = row
        .try_get(1)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if wire.version != 1
        || wire.mapping.key() != key
        || pool != wire.mapping.pool_id().as_bytes()
        || revision <= 0
        || revision as u64 != wire.mapping.version().get()
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Some(RetainedMapping {
        value: wire.mapping,
        protected,
    }))
}
pub(super) fn effect(
    command: &radroots_storage::inventory::CapturedInventoryCommand,
    pool: PoolSnapshot,
    before: Option<OptionPoolMapping>,
) -> Result<Option<MappingEffect>, OperationError> {
    let Some(input) = command.mapping_input() else {
        return Ok(None);
    };
    if pool.id() != input.pool_id() {
        return Err(OperationError::RecoveryRequired);
    }
    if before.is_some_and(|value| value.key() != input.key()) {
        return Err(OperationError::RecoveryRequired);
    }
    if input.expected_mapping_version().get()
        != before.map(|value| value.version().get()).unwrap_or(0)
    {
        return Err(OperationError::StaleState);
    }
    let version = Sequence::new(
        input
            .expected_mapping_version()
            .get()
            .checked_add(1)
            .ok_or(WorkspaceError::Capacity)?,
    )
    .map_err(|_| WorkspaceError::Capacity)?;
    let after = OptionPoolMapping::new(
        input.key(),
        input.pool_id(),
        pool.unit(),
        input.per_sale_unit(),
        version,
    )
    .map_err(|_| OperationError::InvalidCommand)?;
    Ok(Some(MappingEffect { before, after }))
}
pub(super) async fn prepare(
    turn: &mut Turn<'_>,
    access: &Access,
    request: &InventoryRequest,
    pool: PoolSnapshot,
) -> Result<Option<MappingPlan>, OperationError> {
    let Some(input) = request.command().mapping_input() else {
        return Ok(None);
    };
    require(access, input.key())?;
    let original = read(turn.connection, turn.selected, turn.crypto, input.key()).await?;
    let before = original.as_ref().map(|value| value.value);
    let proof = checkpoint::authenticate_mapping_projection(
        turn.connection,
        turn.selected,
        turn.crypto,
        &access.current,
        &access.authority,
        input.key(),
        before,
    )
    .await?;
    let effect =
        effect(request.command(), pool, before)?.ok_or(OperationError::RecoveryRequired)?;
    let protected = turn
        .crypto
        .seal_stock_record(
            turn.selected,
            "mapping",
            &key_id(input.key()),
            &encode(&MappingWire {
                version: 1,
                mapping: effect.after,
            })?,
        )
        .await?;
    Ok(Some(MappingPlan {
        effect,
        protected,
        original: original.map(|value| value.protected),
        proof,
    }))
}
impl MappingPlan {
    pub async fn apply(&self, connection: &mut SqliteConnection) -> Result<(), OperationError> {
        self.proof.confirm(connection).await?;
        let key = self.effect.after.key();
        if let (Some(before), Some(protected)) = (self.effect.before, self.original.as_ref()) {
            let updated=sqlx::query("UPDATE stock_mappings SET pool_id=?,revision=?,protected=? WHERE listing_author=? AND listing_d=? AND option_id=? AND revision=? AND protected=? AND pool_id=?")
                .bind(self.effect.after.pool_id().as_bytes().as_slice()).bind(self.effect.after.version().get() as i64).bind(&self.protected).bind(key.listing().author().as_bytes().as_slice()).bind(key.listing().d().as_bytes().as_slice()).bind(key.option_id().as_bytes().as_slice()).bind(before.version().get() as i64).bind(protected).bind(before.pool_id().as_bytes().as_slice()).execute(connection).await.map_err(|_|OperationError::StorageFailure)?;
            if updated.rows_affected() != 1 {
                return Err(OperationError::RecoveryRequired);
            }
        } else {
            let count:i64=sqlx::query_scalar("SELECT count(*) FROM stock_mappings WHERE listing_author=? AND listing_d=? AND option_id=?").bind(key.listing().author().as_bytes().as_slice()).bind(key.listing().d().as_bytes().as_slice()).bind(key.option_id().as_bytes().as_slice()).fetch_one(&mut *connection).await.map_err(|_|OperationError::StorageFailure)?;
            if count != 0 {
                return Err(OperationError::RecoveryRequired);
            }
            sqlx::query("INSERT INTO stock_mappings(listing_author,listing_d,option_id,pool_id,revision,protected) VALUES(?,?,?,?,?,?)")
                .bind(key.listing().author().as_bytes().as_slice()).bind(key.listing().d().as_bytes().as_slice()).bind(key.option_id().as_bytes().as_slice()).bind(self.effect.after.pool_id().as_bytes().as_slice()).bind(self.effect.after.version().get() as i64).bind(&self.protected).execute(connection).await.map_err(|_|OperationError::StorageFailure)?;
        }
        Ok(())
    }
}
