//! Sealed CLI-v1 argument plans for hardened service management.

use core::fmt;
use std::ffi::{OsStr, OsString};

use radroots_runtime_paths::{RadrootsPathProfile, RuntimeContext};

use crate::RadrootsRuntimeManagerError;

const CLI_PATH_MAX_UTF8_BYTES: usize = 4_096;

/// Common hardened-service CLI-v1 operations governed by this package.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ManagedCliCommand {
    ConfigInit,
    ConfigValidate,
    StateInit,
    Run,
    Status,
    Doctor,
}

impl ManagedCliCommand {
    fn tokens(self) -> &'static [&'static str] {
        match self {
            Self::ConfigInit => &["config", "init"],
            Self::ConfigValidate => &["config", "validate"],
            Self::StateInit => &["state", "init"],
            Self::Run => &["run"],
            Self::Status => &["status"],
            Self::Doctor => &["doctor"],
        }
    }
}

/// Validated arguments for a caller-owned Myc or RHI executable.
///
/// The plan intentionally contains no program, executable, archive, channel,
/// or install path. Those distribution concerns remain outside Step219.
/// External construction is sealed so every argument remains bound to the
/// selected [`RuntimeContext`].
///
/// ```compile_fail
/// use radroots_runtime_manager::ManagedCliInvocation;
///
/// let _ = ManagedCliInvocation {
///     command: todo!(),
///     profile: todo!(),
///     arguments: todo!(),
/// };
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct ManagedCliInvocation {
    command: ManagedCliCommand,
    profile: RadrootsPathProfile,
    arguments: Box<[OsString]>,
}

impl ManagedCliInvocation {
    pub(crate) fn for_context(
        context: &RuntimeContext,
        command: ManagedCliCommand,
    ) -> Result<Self, RadrootsRuntimeManagerError> {
        let profile = cli_profile(context.profile())?;
        let mut arguments = Vec::with_capacity(9);
        arguments.extend([
            OsString::from("--profile"),
            OsString::from(profile),
            OsString::from("--instance"),
            OsString::from(context.instance().as_str()),
        ]);
        if context.profile() == RadrootsPathProfile::RepoLocal {
            let root = context
                .repo_local_root()
                .ok_or(RadrootsRuntimeManagerError::ContextMismatch)?;
            if root
                .to_str()
                .is_none_or(|value| value.len() > CLI_PATH_MAX_UTF8_BYTES)
            {
                return Err(RadrootsRuntimeManagerError::ContextMismatch);
            }
            arguments.push(OsString::from("--repo-local-root"));
            arguments.push(root.as_os_str().to_owned());
        } else if context.repo_local_root().is_some() {
            return Err(RadrootsRuntimeManagerError::ContextMismatch);
        }
        arguments.extend(command.tokens().iter().map(OsString::from));
        Ok(Self {
            command,
            profile: context.profile(),
            arguments: arguments.into_boxed_slice(),
        })
    }

    #[must_use]
    pub const fn command(&self) -> ManagedCliCommand {
        self.command
    }

    #[must_use]
    pub const fn profile(&self) -> RadrootsPathProfile {
        self.profile
    }

    #[must_use]
    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }
}

impl fmt::Debug for ManagedCliInvocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ManagedCliInvocation")
            .field("command", &self.command)
            .field("profile", &self.profile)
            .field("argument_count", &self.arguments.len())
            .field("arguments", &"[redacted]")
            .finish()
    }
}

fn cli_profile(
    profile: RadrootsPathProfile,
) -> Result<&'static OsStr, RadrootsRuntimeManagerError> {
    match profile {
        RadrootsPathProfile::InteractiveUser => Ok(OsStr::new("interactive")),
        RadrootsPathProfile::ServiceHost => Ok(OsStr::new("service-host")),
        RadrootsPathProfile::RepoLocal => Ok(OsStr::new("repo-local")),
        RadrootsPathProfile::MobileNative => Err(RadrootsRuntimeManagerError::UnsupportedProfile),
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, path::PathBuf};

    use radroots_runtime_paths::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };

    use super::{ManagedCliCommand, ManagedCliInvocation, cli_profile};

    fn context(profile: RadrootsPathProfile) -> RuntimeContext {
        let root = (profile == RadrootsPathProfile::RepoLocal)
            .then(|| PathBuf::from("/sensitive/project-root"));
        RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                profile,
                root,
                if profile == RadrootsPathProfile::RepoLocal {
                    RuntimeContextSource::BootstrapCli
                } else {
                    RuntimeContextSource::SafeDefault
                },
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new("myc").expect("service"),
            InstanceId::new("primary").expect("instance"),
        )
        .expect("context")
    }

    #[test]
    fn every_common_command_uses_the_exact_cli_v1_shape() {
        for (command, suffix) in [
            (ManagedCliCommand::ConfigInit, &["config", "init"][..]),
            (
                ManagedCliCommand::ConfigValidate,
                &["config", "validate"][..],
            ),
            (ManagedCliCommand::StateInit, &["state", "init"][..]),
            (ManagedCliCommand::Run, &["run"][..]),
            (ManagedCliCommand::Status, &["status"][..]),
            (ManagedCliCommand::Doctor, &["doctor"][..]),
        ] {
            let invocation = ManagedCliInvocation::for_context(
                &context(RadrootsPathProfile::ServiceHost),
                command,
            )
            .expect("invocation");
            let mut expected = vec![
                OsString::from("--profile"),
                OsString::from("service-host"),
                OsString::from("--instance"),
                OsString::from("primary"),
            ];
            expected.extend(suffix.iter().map(OsString::from));
            assert_eq!(invocation.arguments(), expected);
            assert_eq!(invocation.command(), command);
        }
    }

    #[test]
    fn profile_names_match_both_service_cli_v1_parsers() {
        for (profile, expected) in [
            (RadrootsPathProfile::InteractiveUser, "interactive"),
            (RadrootsPathProfile::ServiceHost, "service-host"),
            (RadrootsPathProfile::RepoLocal, "repo-local"),
        ] {
            assert_eq!(cli_profile(profile).expect("profile"), expected);
        }
        assert!(cli_profile(RadrootsPathProfile::MobileNative).is_err());
    }

    #[test]
    fn repo_local_plan_preserves_the_validated_explicit_root_and_redacts_debug() {
        let invocation = ManagedCliInvocation::for_context(
            &context(RadrootsPathProfile::RepoLocal),
            ManagedCliCommand::Run,
        )
        .expect("invocation");
        assert_eq!(
            invocation.arguments(),
            [
                "--profile",
                "repo-local",
                "--instance",
                "primary",
                "--repo-local-root",
                "/sensitive/project-root",
                "run",
            ]
            .map(OsString::from)
        );
        let debug = format!("{invocation:?}");
        assert!(!debug.contains("sensitive"));
        assert!(!debug.contains("project-root"));
    }

    #[test]
    fn cli_plan_rejects_a_context_root_outside_the_cli_v1_text_bound() {
        let root = format!("/{}", "x".repeat(super::CLI_PATH_MAX_UTF8_BYTES));
        let context = RuntimeContext::resolve(
            &RadrootsPathResolver::new(RadrootsPlatform::Linux, RadrootsHostEnvironment::default()),
            RuntimeContextBootstrap::new(
                RadrootsPathProfile::RepoLocal,
                Some(PathBuf::from(root)),
                RuntimeContextSource::BootstrapCli,
                RuntimeContextSource::BootstrapCli,
            )
            .expect("bootstrap"),
            ServiceId::new("myc").expect("service"),
            InstanceId::new("primary").expect("instance"),
        )
        .expect("context");
        assert_eq!(
            ManagedCliInvocation::for_context(&context, ManagedCliCommand::Run),
            Err(crate::RadrootsRuntimeManagerError::ContextMismatch)
        );
    }
}
