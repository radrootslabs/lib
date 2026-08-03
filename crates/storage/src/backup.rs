//! Versioned backup, staged restore, and member-integrity contracts.

use radroots_transport::BoxFuture;
use std::collections::BTreeSet;

use crate::{
    Error,
    status::{IntegrityStatus, StorageStatus},
};

pub const BACKUP_MEMBER_PATH_MAX_BYTES: usize = 512;
pub const BACKUP_MEMBER_MAX: usize = 1_024;

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BackupId([u8; 16]);

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for BackupId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes = <[u8; 16] as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(bytes).map_err(serde::de::Error::custom)
    }
}

impl BackupId {
    pub const fn new(bytes: [u8; 16]) -> Result<Self, Error> {
        if bytes_are_zero(&bytes) {
            return Err(Error::InvalidBackupId);
        }
        Ok(Self(bytes))
    }
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BackupFormatVersion(u16);

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for BackupFormatVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::new(<u16 as serde::Deserialize>::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }
}

impl BackupFormatVersion {
    pub const V1: Self = Self(1);
    pub const fn new(value: u16) -> Result<Self, Error> {
        if value == 0 {
            Err(Error::InvalidBackupVersion)
        } else {
            Ok(Self(value))
        }
    }
    pub const fn get(self) -> u16 {
        self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackupSecretPolicy {
    ExcludeProtectedStorage,
    IncludeProtectedStorage,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackupMemberKind {
    Runtime,
    Protected,
    Metadata,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemberDigest([u8; 32]);

impl MemberDigest {
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupMember {
    relative_path: String,
    kind: BackupMemberKind,
    byte_length: u64,
    sha256: MemberDigest,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for BackupMember {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Wire {
            relative_path: String,
            kind: BackupMemberKind,
            byte_length: u64,
            sha256: MemberDigest,
        }

        let wire = <Wire as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(wire.relative_path, wire.kind, wire.byte_length, wire.sha256)
            .map_err(serde::de::Error::custom)
    }
}

impl BackupMember {
    pub fn new(
        relative_path: impl Into<String>,
        kind: BackupMemberKind,
        byte_length: u64,
        sha256: MemberDigest,
    ) -> Result<Self, Error> {
        let relative_path = relative_path.into();
        if !valid_member_path(relative_path.as_str()) {
            return Err(Error::InvalidBackupMemberPath);
        }
        if byte_length == 0 {
            return Err(Error::InvalidBackupMemberLength);
        }
        Ok(Self {
            relative_path,
            kind,
            byte_length,
            sha256,
        })
    }
    pub fn relative_path(&self) -> &str {
        self.relative_path.as_str()
    }
    pub const fn kind(&self) -> BackupMemberKind {
        self.kind
    }
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
    pub const fn sha256(&self) -> MemberDigest {
        self.sha256
    }
}

/// Self-contained immutable inventory of one backup bundle.
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupManifest {
    format_version: BackupFormatVersion,
    backup_id: BackupId,
    created_at_unix_ms: u64,
    secret_policy: BackupSecretPolicy,
    total_bytes: u64,
    members: Vec<BackupMember>,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for BackupManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Wire {
            format_version: BackupFormatVersion,
            backup_id: BackupId,
            created_at_unix_ms: u64,
            secret_policy: BackupSecretPolicy,
            total_bytes: u64,
            members: Vec<BackupMember>,
        }

        let wire = <Wire as serde::Deserialize>::deserialize(deserializer)?;
        let expected_total_bytes = wire.total_bytes;
        let manifest = Self::new(
            wire.format_version,
            wire.backup_id,
            wire.created_at_unix_ms,
            wire.secret_policy,
            wire.members,
        )
        .map_err(serde::de::Error::custom)?;
        if manifest.total_bytes() != expected_total_bytes {
            return Err(serde::de::Error::custom(Error::InvalidBackupManifest));
        }
        Ok(manifest)
    }
}

impl BackupManifest {
    pub fn new(
        format_version: BackupFormatVersion,
        backup_id: BackupId,
        created_at_unix_ms: u64,
        secret_policy: BackupSecretPolicy,
        members: Vec<BackupMember>,
    ) -> Result<Self, Error> {
        if created_at_unix_ms == 0 || members.is_empty() || members.len() > BACKUP_MEMBER_MAX {
            return Err(Error::InvalidBackupManifest);
        }
        let mut paths = BTreeSet::new();
        let mut total_bytes = 0_u64;
        for member in &members {
            if !paths.insert(member.relative_path()) {
                return Err(Error::DuplicateBackupMember);
            }
            if member.kind() == BackupMemberKind::Protected
                && secret_policy == BackupSecretPolicy::ExcludeProtectedStorage
            {
                return Err(Error::BackupSecretPolicyViolation);
            }
            total_bytes = total_bytes
                .checked_add(member.byte_length())
                .ok_or(Error::InvalidBackupManifest)?;
        }
        Ok(Self {
            format_version,
            backup_id,
            created_at_unix_ms,
            secret_policy,
            total_bytes,
            members,
        })
    }
    pub const fn format_version(&self) -> BackupFormatVersion {
        self.format_version
    }
    pub const fn backup_id(&self) -> BackupId {
        self.backup_id
    }
    pub const fn created_at_unix_ms(&self) -> u64 {
        self.created_at_unix_ms
    }
    pub const fn secret_policy(&self) -> BackupSecretPolicy {
        self.secret_policy
    }
    pub const fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
    pub fn members(&self) -> &[BackupMember] {
        self.members.as_slice()
    }
    pub fn member(&self, path: &str) -> Option<&BackupMember> {
        self.members
            .iter()
            .find(|member| member.relative_path() == path)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupPlan {
    backup_id: BackupId,
    format_version: BackupFormatVersion,
    secret_policy: BackupSecretPolicy,
    requested_at_unix_ms: u64,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for BackupPlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Wire {
            backup_id: BackupId,
            format_version: BackupFormatVersion,
            secret_policy: BackupSecretPolicy,
            requested_at_unix_ms: u64,
        }

        let wire = <Wire as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(
            wire.backup_id,
            wire.format_version,
            wire.secret_policy,
            wire.requested_at_unix_ms,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl BackupPlan {
    pub const fn new(
        backup_id: BackupId,
        format_version: BackupFormatVersion,
        secret_policy: BackupSecretPolicy,
        requested_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if requested_at_unix_ms == 0 {
            return Err(Error::InvalidBackupTimestamp);
        }
        Ok(Self {
            backup_id,
            format_version,
            secret_policy,
            requested_at_unix_ms,
        })
    }
    pub const fn backup_id(&self) -> BackupId {
        self.backup_id
    }
    pub const fn format_version(&self) -> BackupFormatVersion {
        self.format_version
    }
    pub const fn secret_policy(&self) -> BackupSecretPolicy {
        self.secret_policy
    }
    pub const fn requested_at_unix_ms(&self) -> u64 {
        self.requested_at_unix_ms
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ReliabilityRevision(u64);

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ReliabilityRevision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::new(<u64 as serde::Deserialize>::deserialize(deserializer)?)
            .map_err(serde::de::Error::custom)
    }
}

impl ReliabilityRevision {
    pub const INITIAL: Self = Self(1);
    pub const fn new(value: u64) -> Result<Self, Error> {
        if value == 0 {
            Err(Error::InvalidReliabilityRevision)
        } else {
            Ok(Self(value))
        }
    }
    pub const fn get(self) -> u64 {
        self.0
    }
    fn next(self) -> Result<Self, Error> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(Error::CorruptReliabilityOperation)
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackupStage {
    Planned,
    Captured,
    Verified,
    Finalized,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackupOperation {
    plan: BackupPlan,
    revision: ReliabilityRevision,
    stage: BackupStage,
    manifest: Option<BackupManifest>,
    updated_at_unix_ms: u64,
}

impl BackupOperation {
    pub const fn planned(plan: BackupPlan) -> Self {
        let at = plan.requested_at_unix_ms;
        Self {
            plan,
            revision: ReliabilityRevision::INITIAL,
            stage: BackupStage::Planned,
            manifest: None,
            updated_at_unix_ms: at,
        }
    }
    pub const fn plan(&self) -> &BackupPlan {
        &self.plan
    }
    pub const fn revision(&self) -> ReliabilityRevision {
        self.revision
    }
    pub const fn stage(&self) -> BackupStage {
        self.stage
    }
    pub const fn manifest(&self) -> Option<&BackupManifest> {
        self.manifest.as_ref()
    }
    pub const fn updated_at_unix_ms(&self) -> u64 {
        self.updated_at_unix_ms
    }

    pub fn transition(
        &self,
        expected_revision: ReliabilityRevision,
        transition: BackupTransition,
        at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if expected_revision != self.revision {
            return Err(Error::ReliabilityRevisionConflict);
        }
        if at_unix_ms < self.updated_at_unix_ms {
            return Err(Error::InvalidBackupTimestamp);
        }
        let (stage, manifest) = match (self.stage, transition) {
            (BackupStage::Planned, BackupTransition::Captured(manifest)) => {
                if manifest.backup_id() != self.plan.backup_id()
                    || manifest.format_version() != self.plan.format_version()
                    || manifest.secret_policy() != self.plan.secret_policy()
                {
                    return Err(Error::BackupManifestPlanMismatch);
                }
                (BackupStage::Captured, Some(manifest))
            }
            (BackupStage::Captured, BackupTransition::Verified) => {
                (BackupStage::Verified, self.manifest.clone())
            }
            (BackupStage::Verified, BackupTransition::Finalize) => {
                (BackupStage::Finalized, self.manifest.clone())
            }
            (
                BackupStage::Planned | BackupStage::Captured | BackupStage::Verified,
                BackupTransition::Fail,
            ) => (BackupStage::Failed, self.manifest.clone()),
            (BackupStage::Finalized | BackupStage::Failed, _) => {
                return Err(Error::ReliabilityOperationTerminal);
            }
            _ => return Err(Error::InvalidBackupTransition),
        };
        Ok(Self {
            plan: self.plan.clone(),
            revision: self.revision.next()?,
            stage,
            manifest,
            updated_at_unix_ms: at_unix_ms,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BackupTransition {
    Captured(BackupManifest),
    Verified,
    Finalize,
    Fail,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestorePlan {
    manifest: BackupManifest,
    accepted_secret_policy: BackupSecretPolicy,
    requested_at_unix_ms: u64,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RestorePlan {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Wire {
            manifest: BackupManifest,
            accepted_secret_policy: BackupSecretPolicy,
            requested_at_unix_ms: u64,
        }

        let wire = <Wire as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(
            wire.manifest,
            wire.accepted_secret_policy,
            wire.requested_at_unix_ms,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl RestorePlan {
    pub fn new(
        manifest: BackupManifest,
        accepted_secret_policy: BackupSecretPolicy,
        requested_at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if requested_at_unix_ms == 0 {
            return Err(Error::InvalidRestoreTimestamp);
        }
        if manifest.secret_policy() == BackupSecretPolicy::IncludeProtectedStorage
            && accepted_secret_policy != BackupSecretPolicy::IncludeProtectedStorage
        {
            return Err(Error::BackupSecretPolicyViolation);
        }
        Ok(Self {
            manifest,
            accepted_secret_policy,
            requested_at_unix_ms,
        })
    }
    pub const fn manifest(&self) -> &BackupManifest {
        &self.manifest
    }
    pub const fn accepted_secret_policy(&self) -> BackupSecretPolicy {
        self.accepted_secret_policy
    }
    pub const fn requested_at_unix_ms(&self) -> u64 {
        self.requested_at_unix_ms
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberVerification {
    Verified,
    Missing,
    HashMismatch,
    LengthMismatch,
    UnsafePath,
    Unexpected,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoreMemberStatus {
    relative_path: String,
    verification: MemberVerification,
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for RestoreMemberStatus {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        struct Wire {
            relative_path: String,
            verification: MemberVerification,
        }

        let wire = <Wire as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(wire.relative_path, wire.verification).map_err(serde::de::Error::custom)
    }
}

impl RestoreMemberStatus {
    pub fn new(
        relative_path: impl Into<String>,
        verification: MemberVerification,
    ) -> Result<Self, Error> {
        let relative_path = relative_path.into();
        if !valid_member_path(relative_path.as_str()) {
            return Err(Error::InvalidBackupMemberPath);
        }
        Ok(Self {
            relative_path,
            verification,
        })
    }
    pub fn relative_path(&self) -> &str {
        self.relative_path.as_str()
    }
    pub const fn verification(&self) -> MemberVerification {
        self.verification
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RestoreStage {
    Staging,
    Verifying,
    Finalizing,
    Finalized,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RestoreOperation {
    plan: RestorePlan,
    revision: ReliabilityRevision,
    stage: RestoreStage,
    member_status: Vec<RestoreMemberStatus>,
    updated_at_unix_ms: u64,
}

impl RestoreOperation {
    pub const fn staging(plan: RestorePlan) -> Self {
        let at = plan.requested_at_unix_ms;
        Self {
            plan,
            revision: ReliabilityRevision::INITIAL,
            stage: RestoreStage::Staging,
            member_status: Vec::new(),
            updated_at_unix_ms: at,
        }
    }
    pub const fn plan(&self) -> &RestorePlan {
        &self.plan
    }
    pub const fn revision(&self) -> ReliabilityRevision {
        self.revision
    }
    pub const fn stage(&self) -> RestoreStage {
        self.stage
    }
    pub fn member_status(&self) -> &[RestoreMemberStatus] {
        self.member_status.as_slice()
    }

    pub fn transition(
        &self,
        expected_revision: ReliabilityRevision,
        transition: RestoreTransition,
        at_unix_ms: u64,
    ) -> Result<Self, Error> {
        if expected_revision != self.revision {
            return Err(Error::ReliabilityRevisionConflict);
        }
        if at_unix_ms < self.updated_at_unix_ms {
            return Err(Error::InvalidRestoreTimestamp);
        }
        let (stage, member_status) = match (self.stage, transition) {
            (RestoreStage::Staging, RestoreTransition::Staged) => {
                (RestoreStage::Verifying, Vec::new())
            }
            (RestoreStage::Verifying, RestoreTransition::Verified(statuses)) => {
                validate_restore_members(self.plan.manifest(), &statuses)?;
                (RestoreStage::Finalizing, statuses)
            }
            (RestoreStage::Finalizing, RestoreTransition::Finalize) => {
                (RestoreStage::Finalized, self.member_status.clone())
            }
            (
                RestoreStage::Staging | RestoreStage::Verifying | RestoreStage::Finalizing,
                RestoreTransition::Fail,
            ) => (RestoreStage::Failed, self.member_status.clone()),
            (RestoreStage::Finalized | RestoreStage::Failed, _) => {
                return Err(Error::ReliabilityOperationTerminal);
            }
            _ => return Err(Error::InvalidRestoreTransition),
        };
        Ok(Self {
            plan: self.plan.clone(),
            revision: self.revision.next()?,
            stage,
            member_status,
            updated_at_unix_ms: at_unix_ms,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RestoreTransition {
    Staged,
    Verified(Vec<RestoreMemberStatus>),
    Finalize,
    Fail,
}

/// Backend-neutral reliability operations. Implementations own staging and
/// atomic filesystem replacement; callers receive only typed state.
pub trait StorageReliability: Send + Sync {
    fn begin_backup(&self, plan: BackupPlan) -> BoxFuture<'_, Result<BackupOperation, Error>>;
    fn transition_backup(
        &self,
        backup_id: BackupId,
        expected_revision: ReliabilityRevision,
        transition: BackupTransition,
        at_unix_ms: u64,
    ) -> BoxFuture<'_, Result<BackupOperation, Error>>;
    fn begin_restore(&self, plan: RestorePlan) -> BoxFuture<'_, Result<RestoreOperation, Error>>;
    fn transition_restore(
        &self,
        backup_id: BackupId,
        expected_revision: ReliabilityRevision,
        transition: RestoreTransition,
        at_unix_ms: u64,
    ) -> BoxFuture<'_, Result<RestoreOperation, Error>>;
    fn integrity(&self) -> BoxFuture<'_, Result<IntegrityStatus, Error>>;
    fn status(&self) -> BoxFuture<'_, Result<StorageStatus, Error>>;
    fn close(&self) -> BoxFuture<'_, Result<StorageStatus, Error>>;
}

fn validate_restore_members(
    manifest: &BackupManifest,
    statuses: &[RestoreMemberStatus],
) -> Result<(), Error> {
    if statuses.len() != manifest.members().len() {
        return Err(Error::RestoreMemberVerificationFailed);
    }
    let mut paths = BTreeSet::new();
    for status in statuses {
        if status.verification() != MemberVerification::Verified
            || !paths.insert(status.relative_path())
            || manifest.member(status.relative_path()).is_none()
        {
            return Err(Error::RestoreMemberVerificationFailed);
        }
    }
    Ok(())
}

fn valid_member_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= BACKUP_MEMBER_PATH_MAX_BYTES
        && value == value.trim()
        && !value.starts_with('/')
        && !value.contains('\\')
        && value.split('/').all(|part| {
            !part.is_empty() && part != "." && part != ".." && !part.chars().any(char::is_control)
        })
}

const fn bytes_are_zero(bytes: &[u8; 16]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != 0 {
            return false;
        }
        index += 1;
    }
    true
}
