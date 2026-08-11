//! Platform-bounded local-admin peer authorization.

use core::fmt;
use std::error::Error;

/// Host support level for local-admin peer authorization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminPeerAuthorizationSupport {
    /// Linux requires kernel-reported `SO_PEERCRED` credentials.
    LinuxSoPeerCredRequired,
    /// macOS v1 relies only on owner-restricted filesystem permissions.
    MacOsFilesystemOwnerPermissionsOnly,
    /// The local-admin transport is not supported on this platform in v1.
    Unsupported,
}

impl AdminPeerAuthorizationSupport {
    /// Returns the exact v1 authorization support for the compilation target.
    #[must_use]
    pub const fn current() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::LinuxSoPeerCredRequired
        }
        #[cfg(target_os = "macos")]
        {
            Self::MacOsFilesystemOwnerPermissionsOnly
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            Self::Unsupported
        }
    }
}

/// A safe configuration failure for local-admin peer authorization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminPeerAuthorizationPolicyError {
    InvalidAdminGroupId,
    AdminGroupUnsupported,
}

impl fmt::Display for AdminPeerAuthorizationPolicyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("admin peer authorization policy is invalid")
    }
}

impl Error for AdminPeerAuthorizationPolicyError {}

/// One immutable policy shared by Unix-socket permissions and peer admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AdminPeerAuthorizationPolicy {
    admin_gid: Option<u32>,
}

impl AdminPeerAuthorizationPolicy {
    /// Restricts access to the daemon's effective user identity.
    #[must_use]
    pub const fn owner_only() -> Self {
        Self { admin_gid: None }
    }

    /// Allows the configured Linux admin group in addition to the daemon user.
    pub fn with_admin_gid(admin_gid: u32) -> Result<Self, AdminPeerAuthorizationPolicyError> {
        if AdminPeerAuthorizationSupport::current()
            != AdminPeerAuthorizationSupport::LinuxSoPeerCredRequired
        {
            return Err(AdminPeerAuthorizationPolicyError::AdminGroupUnsupported);
        }
        if admin_gid == u32::MAX {
            return Err(AdminPeerAuthorizationPolicyError::InvalidAdminGroupId);
        }
        Ok(Self {
            admin_gid: Some(admin_gid),
        })
    }

    /// Returns the host support contract represented by this policy.
    #[must_use]
    pub const fn support(self) -> AdminPeerAuthorizationSupport {
        AdminPeerAuthorizationSupport::current()
    }

    /// Returns the configured Linux admin group, when present.
    #[must_use]
    pub const fn admin_gid(self) -> Option<u32> {
        self.admin_gid
    }
}

impl Default for AdminPeerAuthorizationPolicy {
    fn default() -> Self {
        Self::owner_only()
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PeerAuthorizationFailure {
    CredentialsUnavailable { kind: std::io::ErrorKind },
    Denied,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PeerAuthorizationFailure {}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PeerAuthorizer {
    policy: AdminPeerAuthorizationPolicy,
    daemon_euid: u32,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl PeerAuthorizer {
    pub(crate) const fn new(policy: AdminPeerAuthorizationPolicy, daemon_euid: u32) -> Self {
        Self {
            policy,
            daemon_euid,
        }
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn authorize(
        self,
        stream: &tokio::net::UnixStream,
    ) -> Result<(), PeerAuthorizationFailure> {
        self.authorize_linux_result(
            stream
                .peer_cred()
                .map(|credentials| (credentials.uid(), credentials.gid()))
                .map_err(|error| error.kind()),
        )
    }

    #[cfg(target_os = "linux")]
    fn authorize_linux_result(
        self,
        credentials: Result<(u32, u32), std::io::ErrorKind>,
    ) -> Result<(), PeerAuthorizationFailure> {
        let (peer_uid, peer_gid) = credentials
            .map_err(|kind| PeerAuthorizationFailure::CredentialsUnavailable { kind })?;
        self.authorize_linux_credentials(peer_uid, peer_gid)
    }

    #[cfg(target_os = "linux")]
    fn authorize_linux_credentials(
        self,
        peer_uid: u32,
        peer_gid: u32,
    ) -> Result<(), PeerAuthorizationFailure> {
        if peer_uid == self.daemon_euid || self.policy.admin_gid == Some(peer_gid) {
            Ok(())
        } else {
            Err(PeerAuthorizationFailure::Denied)
        }
    }

    #[cfg(target_os = "macos")]
    pub(crate) const fn authorize(
        self,
        _stream: &tokio::net::UnixStream,
    ) -> Result<(), PeerAuthorizationFailure> {
        let _ = self.policy;
        let _ = self.daemon_euid;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_inventory_is_exact_for_the_compilation_target() {
        #[cfg(target_os = "linux")]
        assert_eq!(
            AdminPeerAuthorizationSupport::current(),
            AdminPeerAuthorizationSupport::LinuxSoPeerCredRequired
        );
        #[cfg(target_os = "macos")]
        assert_eq!(
            AdminPeerAuthorizationSupport::current(),
            AdminPeerAuthorizationSupport::MacOsFilesystemOwnerPermissionsOnly
        );
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        assert_eq!(
            AdminPeerAuthorizationSupport::current(),
            AdminPeerAuthorizationSupport::Unsupported
        );
    }

    #[test]
    fn owner_policy_is_stable_and_group_policy_is_platform_bounded() {
        let owner = AdminPeerAuthorizationPolicy::owner_only();
        assert_eq!(owner.admin_gid(), None);
        assert_eq!(owner.support(), AdminPeerAuthorizationSupport::current());
        assert_eq!(
            AdminPeerAuthorizationPolicy::with_admin_gid(u32::MAX),
            Err(if cfg!(target_os = "linux") {
                AdminPeerAuthorizationPolicyError::InvalidAdminGroupId
            } else {
                AdminPeerAuthorizationPolicyError::AdminGroupUnsupported
            })
        );
        #[cfg(target_os = "linux")]
        assert_eq!(
            AdminPeerAuthorizationPolicy::with_admin_gid(42)
                .expect("valid Linux admin group")
                .admin_gid(),
            Some(42)
        );
        #[cfg(not(target_os = "linux"))]
        assert_eq!(
            AdminPeerAuthorizationPolicy::with_admin_gid(42),
            Err(AdminPeerAuthorizationPolicyError::AdminGroupUnsupported)
        );
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn linux_process_credentials_allow_uid_or_gid_and_deny_otherwise() {
        let (peer, observed) = tokio::net::UnixStream::pair().expect("Unix stream pair");
        let credentials = peer.peer_cred().expect("Linux SO_PEERCRED");
        let different_uid = different_id(credentials.uid());
        let different_gid = different_id(credentials.gid());

        PeerAuthorizer::new(
            AdminPeerAuthorizationPolicy::owner_only(),
            credentials.uid(),
        )
        .authorize(&observed)
        .expect("matching daemon euid");
        PeerAuthorizer::new(
            AdminPeerAuthorizationPolicy::with_admin_gid(credentials.gid())
                .expect("peer group policy"),
            different_uid,
        )
        .authorize(&observed)
        .expect("matching configured admin gid");
        assert_eq!(
            PeerAuthorizer::new(
                AdminPeerAuthorizationPolicy::with_admin_gid(different_gid)
                    .expect("different group policy"),
                different_uid,
            )
            .authorize(&observed),
            Err(PeerAuthorizationFailure::Denied)
        );
        assert_eq!(
            PeerAuthorizer::new(
                AdminPeerAuthorizationPolicy::owner_only(),
                credentials.uid(),
            )
            .authorize_linux_result(Err(std::io::ErrorKind::PermissionDenied)),
            Err(PeerAuthorizationFailure::CredentialsUnavailable {
                kind: std::io::ErrorKind::PermissionDenied,
            })
        );
    }

    #[cfg(target_os = "linux")]
    fn different_id(id: u32) -> u32 {
        if id == 0 { 1 } else { 0 }
    }
}
