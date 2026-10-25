use super::*;
use radroots_domain::{CheckpointDigest, Sequence, UnixSeconds};
use radroots_storage::command::OperationError;
use sha2::{Digest, Sha256};

pub(super) const PART_BYTES: usize = 16 * 1024;
pub(super) const RAW_MAX_BYTES: usize = 64 * 1024;

fn listing_revision<'de, D: serde::Deserializer<'de>>(d: D) -> Result<ListingRevision, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Wire {
        coordinate: ListingCoordinate,
        event_id: EnvelopeEventId,
    }
    let value: Wire = super::super::object(d)?;
    Ok(ListingRevision::new(value.coordinate, value.event_id))
}

pub(super) fn time(value: &str) -> Result<u64, OperationError> {
    if value.is_empty()
        || value.len() > 20
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(OperationError::RecoveryRequired);
    }
    value.parse().map_err(|_| OperationError::RecoveryRequired)
}

#[derive(Clone, Eq, PartialEq, Serialize)]
pub(super) struct Head {
    pub version: u8,
    pub listing: ListingRevision,
    pub created_at: String,
    pub revision: Sequence,
}
impl<'de> Deserialize<'de> for Head {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            version: u8,
            #[serde(deserialize_with = "listing_revision")]
            listing: ListingRevision,
            created_at: String,
            revision: Sequence,
        }
        let value: Wire = super::super::object(d)?;
        let value = Self {
            version: value.version,
            listing: value.listing,
            created_at: value.created_at,
            revision: value.revision,
        };
        value.validate().map_err(serde::de::Error::custom)?;
        Ok(value)
    }
}
impl Head {
    fn validate(&self) -> Result<(), OperationError> {
        if self.version != 1 || self.revision.get() == 0 {
            return Err(OperationError::RecoveryRequired);
        }
        time(&self.created_at)?;
        Ok(())
    }
}
pub(super) fn select(before: Option<&Head>, candidate: Head) -> Result<Head, OperationError> {
    candidate.validate()?;
    let Some(before) = before else {
        return Ok(candidate);
    };
    before.validate()?;
    if before.listing.coordinate() != candidate.listing.coordinate() {
        return Err(OperationError::RecoveryRequired);
    }
    let old = time(&before.created_at)?;
    let new = time(&candidate.created_at)?;
    if old > new
        || (old == new
            && before.listing.event_id().as_bytes() <= candidate.listing.event_id().as_bytes())
    {
        Ok(before.clone())
    } else {
        Ok(candidate)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    #[serde(deserialize_with = "listing_revision")]
    pub listing: ListingRevision,
    pub created_at: String,
    pub bytes: u32,
    pub digest: [u8; 32],
    pub parts: u8,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Header {
    pub version: u8,
    #[serde(deserialize_with = "super::super::object")]
    pub namespace: SellerAuthoritySelection,
    pub transaction: TransactionId,
    pub revision: Sequence,
    #[serde(deserialize_with = "super::super::required_nullable")]
    pub previous: Option<CheckpointDigest>,
    #[serde(deserialize_with = "super::super::required_nullable")]
    pub head: Option<CheckpointDigest>,
    pub grant: GrantId,
    pub principal: String,
    pub authorization: StockAuthorizationRef,
    pub observed_at: UnixSeconds,
    pub minimum_revision: Sequence,
    #[serde(deserialize_with = "super::super::required_nullable")]
    pub minimum_head: Option<CheckpointDigest>,
    #[serde(deserialize_with = "super::super::object")]
    pub manifest: Manifest,
    #[serde(deserialize_with = "super::super::required_nullable")]
    pub before: Option<Head>,
    pub after: Head,
}
impl Header {
    pub fn receipt(&self) -> Result<StockListingReceipt, OperationError> {
        Ok(StockListingReceipt {
            transaction:self.transaction,listing:self.manifest.listing,
            selected_at_retention:self.after.listing.event_id()==self.manifest.listing.event_id(),
            checkpoint:radroots_storage::workspace::seller_stock::StockListingCheckpointRef::from_host_metadata(
                self.namespace.authority(),self.revision,self.head,
            )?.with_authorization(self.authorization),
        })
    }
    pub fn calculate_head(&self) -> Result<CheckpointDigest, OperationError> {
        // Version1 canonical preimage includes every provenance, scope,
        // manifest and projection field, with only its own pointer cleared.
        let mut preimage = self.clone();
        preimage.head = None;
        let mut hash = Sha256::new();
        hash.update(b"radroots.seller_stock.public_source_chain.v1\0");
        hash.update(encode(&preimage)?.as_slice());
        Ok(CheckpointDigest::from_bytes(hash.finalize().into()))
    }
    pub fn validate(
        &self,
        input: &AttachmentInput,
        administrator: GrantId,
    ) -> Result<(), OperationError> {
        let manifest = &self.manifest;
        if self.version != 1
            || !self.namespace.matches_attachment(input)
            || manifest.listing.coordinate().author() != input.participant()
            || self.grant != administrator
            || self.principal.is_empty()
            || self.principal.len() > 128
            || self.principal.chars().any(char::is_control)
            || self.observed_at.get() == 0
            || self.revision.get() == 0
            || (self.revision.get() == 1) != self.previous.is_none()
            || self.minimum_revision.get() >= self.revision.get()
            || (self.minimum_revision.get() == 0) != self.minimum_head.is_none()
            || manifest.bytes == 0
            || manifest.bytes as usize > RAW_MAX_BYTES
            || usize::from(manifest.parts) != (manifest.bytes as usize).div_ceil(PART_BYTES)
            || self.head != Some(self.calculate_head()?)
        {
            return Err(OperationError::RecoveryRequired);
        }
        time(&manifest.created_at)?;
        if let Some(before) = &self.before {
            before.validate()?;
            if before.revision >= self.revision
                || before.listing.event_id() == manifest.listing.event_id()
            {
                return Err(OperationError::RecoveryRequired);
            }
        }
        let candidate = Head {
            version: 1,
            listing: manifest.listing,
            created_at: manifest.created_at.clone(),
            revision: self.revision,
        };
        if self.after != select(self.before.as_ref(), candidate)? {
            return Err(OperationError::RecoveryRequired);
        }
        Ok(())
    }
}
pub(super) fn encode(value: &impl Serialize) -> Result<Zeroizing<Vec<u8>>, OperationError> {
    let bytes =
        Zeroizing::new(serde_json::to_vec(value).map_err(|_| OperationError::StorageFailure)?);
    if bytes.len() > 16 * 1024 {
        return Err(WorkspaceError::Capacity.into());
    }
    Ok(bytes)
}

pub(super) fn coordinate_id(coordinate: ListingCoordinate) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.public_source_coordinate.v1\0");
    hash.update(coordinate.author().as_bytes());
    hash.update(coordinate.d().as_bytes());
    hash.finalize().into()
}
pub(super) fn part_id(event: EnvelopeEventId, part: u8) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.public_source_part.v1\0");
    hash.update(event.as_bytes());
    hash.update([part]);
    hash.finalize().into()
}

pub(super) struct RetainedHeader {
    pub header: Header,
    pub protected: Vec<u8>,
}
pub(super) struct RetainedHead {
    pub head: Head,
    pub protected: Vec<u8>,
}
pub(super) struct Parts {
    pub raw: Zeroizing<Vec<u8>>,
    pub witness: [u8; 32],
}

pub(super) async fn read(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    administrator: GrantId,
    event: EnvelopeEventId,
) -> Result<Option<RetainedHeader>, OperationError> {
    use sqlx::Row;
    let row=sqlx::query("SELECT listing_author,listing_d,created_at,transaction_id,revision,chain_head,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_public_events WHERE event_id=?")
        .bind(event.as_bytes().as_slice()).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let Some(row) = row else { return Ok(None) };
    let protected: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(6)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let clear = crypto
        .open_stock_record(input, "public_event", event.as_bytes(), &protected)
        .await?;
    let header: Header = clear.expose_secret(|bytes| super::super::closed_record(bytes))?;
    header.validate(input, administrator)?;
    let coordinate = header.manifest.listing.coordinate();
    if header.manifest.listing.event_id() != event {
        return Err(OperationError::RecoveryRequired);
    }
    let author: Vec<u8> = row
        .try_get(0)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let discriminator: Vec<u8> = row
        .try_get(1)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let created: Vec<u8> = row
        .try_get(2)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let transaction: Vec<u8> = row
        .try_get(3)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let revision: i64 = row
        .try_get(4)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let head: Vec<u8> = row
        .try_get(5)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if author != coordinate.author().as_bytes()
        || discriminator != coordinate.d().as_bytes()
        || created != time(&header.manifest.created_at)?.to_be_bytes()
        || transaction != header.transaction.as_bytes()
        || revision <= 0
        || revision as u64 != header.revision.get()
        || head
            != header
                .head
                .ok_or(OperationError::RecoveryRequired)?
                .as_bytes()
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Some(RetainedHeader { header, protected }))
}
pub(super) async fn read_head(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    coordinate: ListingCoordinate,
) -> Result<Option<RetainedHead>, OperationError> {
    use sqlx::Row;
    let row=sqlx::query("SELECT event_id,revision,CASE WHEN length(protected) BETWEEN 1 AND 262144 THEN protected END FROM stock_public_heads WHERE listing_author=? AND listing_d=?")
        .bind(coordinate.author().as_bytes().as_slice()).bind(coordinate.d().as_bytes().as_slice())
        .fetch_optional(connection).await.map_err(|_|OperationError::RecoveryRequired)?;
    let Some(row) = row else { return Ok(None) };
    let protected: Vec<u8> = row
        .try_get::<Option<Vec<u8>>, _>(2)
        .map_err(|_| OperationError::RecoveryRequired)?
        .ok_or(OperationError::RecoveryRequired)?;
    let clear = crypto
        .open_stock_record(input, "public_head", &coordinate_id(coordinate), &protected)
        .await?;
    let head: Head = clear.expose_secret(|bytes| super::super::closed_record(bytes))?;
    head.validate()?;
    let event: Vec<u8> = row
        .try_get(0)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let revision: i64 = row
        .try_get(1)
        .map_err(|_| OperationError::RecoveryRequired)?;
    if head.listing.coordinate() != coordinate
        || coordinate.author() != input.participant()
        || event != head.listing.event_id().as_bytes()
        || revision <= 0
        || revision as u64 != head.revision.get()
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Some(RetainedHead { head, protected }))
}
fn part_hash() -> Sha256 {
    let mut hash = Sha256::new();
    hash.update(b"radroots.seller_stock.public_source_parts_snapshot.v1\0");
    hash
}
fn hash_part(hash: &mut Sha256, part: u8, protected: &[u8]) {
    hash.update([part]);
    hash.update((protected.len() as u64).to_be_bytes());
    hash.update(protected);
}
pub(super) async fn parts_witness(
    connection: &mut SqliteConnection,
    event: EnvelopeEventId,
) -> Result<(u8, [u8; 32]), OperationError> {
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM stock_public_parts WHERE event_id=?")
        .bind(event.as_bytes().as_slice())
        .fetch_one(&mut *connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?;
    if !(1..=4).contains(&count) {
        return Err(OperationError::RecoveryRequired);
    }
    let mut hash = part_hash();
    for part in 0..count {
        let protected:Vec<u8>=sqlx::query_scalar("SELECT protected FROM stock_public_parts WHERE event_id=? AND part=? AND length(protected) BETWEEN 1 AND 262144")
            .bind(event.as_bytes().as_slice()).bind(part).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        hash_part(&mut hash, part as u8, &protected);
    }
    Ok((count as u8, hash.finalize().into()))
}
pub(super) async fn read_parts(
    connection: &mut SqliteConnection,
    input: &AttachmentInput,
    crypto: &WorkspaceCrypto,
    header: &Header,
) -> Result<Parts, OperationError> {
    let event = header.manifest.listing.event_id();
    let mut raw = Zeroizing::new(Vec::with_capacity(header.manifest.bytes as usize));
    let mut hash = part_hash();
    for part in 0..header.manifest.parts {
        let protected:Vec<u8>=sqlx::query_scalar("SELECT protected FROM stock_public_parts WHERE event_id=? AND part=? AND length(protected) BETWEEN 1 AND 262144")
            .bind(event.as_bytes().as_slice()).bind(i64::from(part)).fetch_optional(&mut *connection).await.map_err(|_|OperationError::RecoveryRequired)?.ok_or(OperationError::RecoveryRequired)?;
        hash_part(&mut hash, part, &protected);
        let clear = crypto
            .open_stock_record(input, "public_part", &part_id(event, part), &protected)
            .await?;
        clear.expose_secret(|bytes| {
            let expected = if part + 1 == header.manifest.parts {
                header.manifest.bytes as usize - usize::from(part) * PART_BYTES
            } else {
                PART_BYTES
            };
            if bytes.len() != expected {
                return Err(OperationError::RecoveryRequired);
            }
            raw.extend_from_slice(bytes);
            Ok(())
        })?;
    }
    let witness: [u8; 32] = hash.finalize().into();
    if raw.len() != header.manifest.bytes as usize
        || <[u8; 32]>::from(Sha256::digest(raw.as_slice())) != header.manifest.digest
        || parts_witness(connection, event).await? != (header.manifest.parts, witness)
    {
        return Err(OperationError::RecoveryRequired);
    }
    // Original signature verification can reconstruct public observation
    // evidence; private authenticated-opening tokens are never reconstructed.
    let mut observed = ObservedHead::listing(
        header.manifest.listing.coordinate().author(),
        &header.manifest.listing.coordinate().d().to_string(),
    )
    .map_err(|_| OperationError::RecoveryRequired)?;
    observed
        .observe(std::str::from_utf8(&raw).map_err(|_| OperationError::RecoveryRequired)?)
        .map_err(|_| OperationError::RecoveryRequired)?;
    let selected = observed
        .selected()
        .ok_or(OperationError::RecoveryRequired)?;
    if selected.event_id.as_bytes() != event.as_bytes()
        || selected.created_at != time(&header.manifest.created_at)?
    {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(Parts { raw, witness })
}

pub(super) async fn confirm_head(
    connection: &mut SqliteConnection,
    coordinate: ListingCoordinate,
    original: Option<&RetainedHead>,
) -> Result<(), OperationError> {
    let count: i64 = if let Some(original) = original {
        sqlx::query_scalar("SELECT count(*) FROM stock_public_heads WHERE listing_author=? AND listing_d=? AND event_id=? AND revision=? AND protected=?")
            .bind(coordinate.author().as_bytes().as_slice()).bind(coordinate.d().as_bytes().as_slice())
            .bind(original.head.listing.event_id().as_bytes().as_slice()).bind(original.head.revision.get() as i64).bind(&original.protected)
            .fetch_one(connection).await.map_err(|_|OperationError::RecoveryRequired)?
    } else {
        sqlx::query_scalar(
            "SELECT count(*) FROM stock_public_heads WHERE listing_author=? AND listing_d=?",
        )
        .bind(coordinate.author().as_bytes().as_slice())
        .bind(coordinate.d().as_bytes().as_slice())
        .fetch_one(connection)
        .await
        .map_err(|_| OperationError::RecoveryRequired)?
    };
    if count != i64::from(original.is_some()) {
        return Err(OperationError::RecoveryRequired);
    }
    Ok(())
}
pub(super) struct PreparedParts {
    protected: Vec<Vec<u8>>,
}
impl PreparedParts {
    pub async fn prepare(
        crypto: &WorkspaceCrypto,
        input: &AttachmentInput,
        capture: &StockListingCapture,
    ) -> Result<Self, OperationError> {
        if capture.raw.is_empty() || capture.raw.len() > RAW_MAX_BYTES {
            return Err(WorkspaceError::Capacity.into());
        }
        let mut protected = Vec::with_capacity(capture.raw.len().div_ceil(PART_BYTES));
        for (part, bytes) in capture.raw.chunks(PART_BYTES).enumerate() {
            protected.push(
                crypto
                    .seal_stock_record(
                        input,
                        "public_part",
                        &part_id(capture.listing.event_id(), part as u8),
                        bytes,
                    )
                    .await?,
            );
        }
        Ok(Self { protected })
    }
    pub async fn insert(
        &self,
        connection: &mut SqliteConnection,
        event: EnvelopeEventId,
    ) -> Result<(), OperationError> {
        for (part, protected) in self.protected.iter().enumerate() {
            sqlx::query("INSERT INTO stock_public_parts(event_id,part,protected) VALUES(?,?,?)")
                .bind(event.as_bytes().as_slice())
                .bind(part as i64)
                .bind(protected)
                .execute(&mut *connection)
                .await
                .map_err(|_| OperationError::StorageFailure)?;
        }
        Ok(())
    }
}
