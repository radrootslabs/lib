//! Explicit provisioning for one canonical service-instance state directory.

use core::fmt;
use std::{
    error::Error,
    ffi::{OsStr, OsString},
    path::{Component, Path, PathBuf},
};

use crate::{InstanceId, RadrootsPathProfile, RuntimeContext, ServiceId};

#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::fs::File;

#[cfg(any(target_os = "linux", target_os = "macos"))]
use rustix::{
    fs::{AtFlags, FileType, Mode, OFlags, fstat, mkdirat, open, openat, statat, unlinkat},
    process::geteuid,
};

const SERVICES_COMPONENT: &str = "services";

/// A sealed plan for validating or creating one canonical state directory.
///
/// Construction is available only from [`RuntimeContext::state_directory_plan`].
/// The plan owns no ambient-environment lookup and performs no filesystem I/O
/// until [`Self::provision`] is called.
///
/// ```compile_fail
/// use radroots_runtime_paths::{RadrootsPathProfile, RuntimeStateDirectoryPlan};
///
/// let _ = RuntimeStateDirectoryPlan {
///     profile: RadrootsPathProfile::RepoLocal,
///     state_root: "/tmp/alternate".into(),
///     service: todo!(),
///     instance: todo!(),
/// };
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct RuntimeStateDirectoryPlan {
    profile: RadrootsPathProfile,
    state_root: PathBuf,
    service: ServiceId,
    instance: InstanceId,
}

impl RuntimeStateDirectoryPlan {
    pub(crate) fn from_context(
        context: &RuntimeContext,
    ) -> Result<Self, StateDirectoryProvisionError> {
        let state_root = context.paths().state_root();
        validate_absolute_root(state_root)?;
        let expected = state_root
            .join(SERVICES_COMPONENT)
            .join(context.service().as_str())
            .join(context.instance().as_str());
        if expected != context.paths().state() {
            return Err(StateDirectoryProvisionError::InvalidPlan);
        }
        Ok(Self {
            profile: context.profile(),
            state_root: state_root.to_path_buf(),
            service: context.service().clone(),
            instance: context.instance().clone(),
        })
    }

    /// Returns the path profile whose creation policy is frozen by this plan.
    #[must_use]
    pub fn profile(&self) -> RadrootsPathProfile {
        self.profile
    }

    /// Validates or creates the exact `services/<service>/<instance>` suffix.
    ///
    /// `InteractiveUser` and `RepoLocal` plans may create missing suffix
    /// directories. `ServiceHost` plans validate an already-provisioned suffix
    /// and never create it. Existing directories are never permission-repaired.
    /// Every traversal and creation is descriptor-relative and rejects symlinks.
    pub fn provision(&self) -> Result<(), StateDirectoryProvisionError> {
        provision_supported(self)
    }

    fn components(&self) -> [&OsStr; 3] {
        [
            OsStr::new(SERVICES_COMPONENT),
            OsStr::new(self.service.as_str()),
            OsStr::new(self.instance.as_str()),
        ]
    }

    fn permits_creation(&self) -> Result<bool, StateDirectoryProvisionError> {
        match self.profile {
            RadrootsPathProfile::InteractiveUser | RadrootsPathProfile::RepoLocal => Ok(true),
            RadrootsPathProfile::ServiceHost => Ok(false),
            RadrootsPathProfile::MobileNative => {
                Err(StateDirectoryProvisionError::UnsupportedProfile)
            }
        }
    }
}

impl fmt::Debug for RuntimeStateDirectoryPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeStateDirectoryPlan")
            .field("profile", &self.profile)
            .field("state_root", &"[redacted]")
            .field("service", &"[redacted]")
            .field("instance", &"[redacted]")
            .finish()
    }
}

/// Stable path-free failures from state-directory planning or provisioning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StateDirectoryProvisionError {
    InvalidPlan,
    UnsupportedPlatform,
    UnsupportedProfile,
    StateRootUnavailable,
    MissingDirectory,
    DirectoryConflict,
    UnsafeDirectory,
    Filesystem,
    Cleanup,
}

impl fmt::Display for StateDirectoryProvisionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidPlan => "runtime state-directory plan is invalid",
            Self::UnsupportedPlatform => {
                "runtime state-directory provisioning is unsupported on this platform"
            }
            Self::UnsupportedProfile => {
                "runtime state-directory provisioning is unsupported for this profile"
            }
            Self::StateRootUnavailable => "runtime state-directory root is unavailable or unsafe",
            Self::MissingDirectory => "runtime state directory must already exist for this profile",
            Self::DirectoryConflict => {
                "runtime state-directory entry conflicts with the canonical plan"
            }
            Self::UnsafeDirectory => "runtime state directory failed security validation",
            Self::Filesystem => "runtime state-directory filesystem operation failed",
            Self::Cleanup => "runtime state-directory cleanup could not be proven complete",
        })
    }
}

impl Error for StateDirectoryProvisionError {}

fn validate_absolute_root(root: &Path) -> Result<(), StateDirectoryProvisionError> {
    if !root.is_absolute() || root.parent().is_none() {
        return Err(StateDirectoryProvisionError::InvalidPlan);
    }
    let mut saw_root = false;
    let mut saw_normal = false;
    for component in root.components() {
        match component {
            Component::RootDir if !saw_root && !saw_normal => saw_root = true,
            Component::Normal(_) if saw_root => saw_normal = true,
            Component::Prefix(_)
            | Component::CurDir
            | Component::ParentDir
            | Component::RootDir => {
                return Err(StateDirectoryProvisionError::InvalidPlan);
            }
            Component::Normal(_) => return Err(StateDirectoryProvisionError::InvalidPlan),
        }
    }
    if saw_normal {
        Ok(())
    } else {
        Err(StateDirectoryProvisionError::InvalidPlan)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn provision_supported(
    _plan: &RuntimeStateDirectoryPlan,
) -> Result<(), StateDirectoryProvisionError> {
    Err(StateDirectoryProvisionError::UnsupportedPlatform)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn provision_supported(
    plan: &RuntimeStateDirectoryPlan,
) -> Result<(), StateDirectoryProvisionError> {
    provision_with_operations(plan, &SystemProvisionOperations)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
trait ProvisionOperations {
    fn after_create(&self, _component_index: usize) -> Result<(), StateDirectoryProvisionError> {
        Ok(())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct SystemProvisionOperations;

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl ProvisionOperations for SystemProvisionOperations {}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DirectoryIdentity {
    device: u64,
    inode: u64,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct CreatedDirectory {
    parent: File,
    name: OsString,
    held: File,
    identity: DirectoryIdentity,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct CreationJournal {
    state_root_path: PathBuf,
    state_root: File,
    state_root_identity: DirectoryIdentity,
    entries: Vec<CreatedDirectory>,
    committed: bool,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl CreationJournal {
    fn new(
        state_root_path: PathBuf,
        state_root: File,
        state_root_identity: DirectoryIdentity,
    ) -> Self {
        Self {
            state_root_path,
            state_root,
            state_root_identity,
            entries: Vec::new(),
            committed: false,
        }
    }

    fn fail(
        &mut self,
        failure: StateDirectoryProvisionError,
    ) -> Result<(), StateDirectoryProvisionError> {
        match self.cleanup() {
            Ok(()) => Err(failure),
            Err(()) => Err(StateDirectoryProvisionError::Cleanup),
        }
    }

    fn cleanup(&mut self) -> Result<(), ()> {
        if validate_absolute_directory_binding(
            &self.state_root_path,
            &self.state_root,
            self.state_root_identity,
        )
        .is_err()
        {
            return Err(());
        }
        let mut clean = true;
        for entry in self.entries.iter().rev() {
            if cleanup_created_directory(entry).is_err() {
                clean = false;
                break;
            }
        }
        if clean {
            self.entries.clear();
            self.committed = true;
            Ok(())
        } else {
            Err(())
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for CreationJournal {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.cleanup();
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn provision_with_operations(
    plan: &RuntimeStateDirectoryPlan,
    operations: &dyn ProvisionOperations,
) -> Result<(), StateDirectoryProvisionError> {
    let permits_creation = plan.permits_creation()?;
    let state_root = open_absolute_directory(&plan.state_root)?;
    let state_root_identity = validate_secure_directory(&state_root, false)
        .map_err(|_| StateDirectoryProvisionError::StateRootUnavailable)?;
    let mut current = state_root
        .try_clone()
        .map_err(|_| StateDirectoryProvisionError::Filesystem)?;
    let mut journal =
        CreationJournal::new(plan.state_root.clone(), state_root, state_root_identity);
    let mut bindings = Vec::with_capacity(3);

    for (component_index, component) in plan.components().into_iter().enumerate() {
        if validate_absolute_directory_binding(
            &journal.state_root_path,
            &journal.state_root,
            journal.state_root_identity,
        )
        .is_err()
        {
            return journal.fail(StateDirectoryProvisionError::StateRootUnavailable);
        }
        let parent = match current.try_clone() {
            Ok(parent) => parent,
            Err(_) => return journal.fail(StateDirectoryProvisionError::Filesystem),
        };
        match open_directory_at(&current, component) {
            Ok(next) => {
                let identity = match validate_secure_directory(&next, false) {
                    Ok(identity) => identity,
                    Err(failure) => return journal.fail(failure),
                };
                let held = match next.try_clone() {
                    Ok(held) => held,
                    Err(_) => return journal.fail(StateDirectoryProvisionError::Filesystem),
                };
                bindings.push(DirectoryBinding {
                    parent,
                    name: component.to_os_string(),
                    held,
                    identity,
                    exact_owner_mode: false,
                });
                current = next;
            }
            Err(rustix::io::Errno::NOENT) if !permits_creation => {
                return journal.fail(StateDirectoryProvisionError::MissingDirectory);
            }
            Err(rustix::io::Errno::NOENT) => {
                match mkdirat(&parent, component, Mode::RUSR | Mode::WUSR | Mode::XUSR) {
                    Ok(()) => {}
                    Err(rustix::io::Errno::EXIST) => match open_directory_at(&parent, component) {
                        Ok(next) => {
                            let identity = match validate_secure_directory(&next, false) {
                                Ok(identity) => identity,
                                Err(failure) => return journal.fail(failure),
                            };
                            let held = match next.try_clone() {
                                Ok(held) => held,
                                Err(_) => {
                                    return journal.fail(StateDirectoryProvisionError::Filesystem);
                                }
                            };
                            bindings.push(DirectoryBinding {
                                parent,
                                name: component.to_os_string(),
                                held,
                                identity,
                                exact_owner_mode: false,
                            });
                            current = next;
                            continue;
                        }
                        Err(_) => {
                            return journal.fail(StateDirectoryProvisionError::DirectoryConflict);
                        }
                    },
                    Err(_) => return journal.fail(StateDirectoryProvisionError::Filesystem),
                }

                let created_identity = match created_directory_identity(&parent, component) {
                    Ok(identity) => identity,
                    Err(failure) => return journal.fail(failure),
                };
                let held = match open_directory_at(&parent, component) {
                    Ok(held) => held,
                    Err(_) => return journal.fail(StateDirectoryProvisionError::DirectoryConflict),
                };
                let opened_identity = match validate_secure_directory(&held, true) {
                    Ok(identity) => identity,
                    Err(failure) => return journal.fail(failure),
                };
                if opened_identity != created_identity {
                    return journal.fail(StateDirectoryProvisionError::DirectoryConflict);
                }
                journal.entries.push(CreatedDirectory {
                    parent,
                    name: component.to_os_string(),
                    held,
                    identity: created_identity,
                });
                let Some(created) = journal.entries.last() else {
                    return journal.fail(StateDirectoryProvisionError::Filesystem);
                };
                let next = match created.held.try_clone() {
                    Ok(next) => next,
                    Err(_) => return journal.fail(StateDirectoryProvisionError::Filesystem),
                };
                let binding_parent = match created.parent.try_clone() {
                    Ok(binding_parent) => binding_parent,
                    Err(_) => return journal.fail(StateDirectoryProvisionError::Filesystem),
                };
                let binding_held = match created.held.try_clone() {
                    Ok(binding_held) => binding_held,
                    Err(_) => return journal.fail(StateDirectoryProvisionError::Filesystem),
                };
                if created.parent.sync_all().is_err() {
                    return journal.fail(StateDirectoryProvisionError::Filesystem);
                }
                if let Err(failure) = operations.after_create(component_index) {
                    return journal.fail(failure);
                }
                bindings.push(DirectoryBinding {
                    parent: binding_parent,
                    name: component.to_os_string(),
                    held: binding_held,
                    identity: created_identity,
                    exact_owner_mode: true,
                });
                current = next;
            }
            Err(_) => return journal.fail(StateDirectoryProvisionError::DirectoryConflict),
        }
    }

    if validate_absolute_directory_binding(
        &journal.state_root_path,
        &journal.state_root,
        journal.state_root_identity,
    )
    .is_err()
    {
        return journal.fail(StateDirectoryProvisionError::StateRootUnavailable);
    }
    for binding in &bindings {
        if validate_directory_binding(binding).is_err() {
            return journal.fail(StateDirectoryProvisionError::DirectoryConflict);
        }
    }

    journal.committed = true;
    Ok(())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct DirectoryBinding {
    parent: File,
    name: OsString,
    held: File,
    identity: DirectoryIdentity,
    exact_owner_mode: bool,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_directory_binding(binding: &DirectoryBinding) -> Result<(), ()> {
    let current = open_directory_at(&binding.parent, &binding.name).map_err(|_| ())?;
    let held_identity =
        validate_secure_directory(&binding.held, binding.exact_owner_mode).map_err(|_| ())?;
    let current_identity =
        validate_secure_directory(&current, binding.exact_owner_mode).map_err(|_| ())?;
    if held_identity == binding.identity && current_identity == binding.identity {
        Ok(())
    } else {
        Err(())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_absolute_directory_binding(
    path: &Path,
    held: &File,
    expected: DirectoryIdentity,
) -> Result<(), ()> {
    let current = open_absolute_directory(path).map_err(|_| ())?;
    let held_identity = validate_secure_directory(held, false).map_err(|_| ())?;
    let current_identity = validate_secure_directory(&current, false).map_err(|_| ())?;
    if held_identity == expected && current_identity == expected {
        Ok(())
    } else {
        Err(())
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_absolute_directory(root: &Path) -> Result<File, StateDirectoryProvisionError> {
    let mut current = File::from(
        open(
            Path::new("/"),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| StateDirectoryProvisionError::StateRootUnavailable)?,
    );
    for component in root.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                current = open_directory_at(&current, name)
                    .map_err(|_| StateDirectoryProvisionError::StateRootUnavailable)?;
            }
            Component::Prefix(_) | Component::CurDir | Component::ParentDir => {
                return Err(StateDirectoryProvisionError::InvalidPlan);
            }
        }
    }
    Ok(current)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_directory_at(parent: &File, name: &OsStr) -> Result<File, rustix::io::Errno> {
    openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map(File::from)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn created_directory_identity(
    parent: &File,
    name: &OsStr,
) -> Result<DirectoryIdentity, StateDirectoryProvisionError> {
    let status = statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| StateDirectoryProvisionError::DirectoryConflict)?;
    validate_directory_status(
        FileType::from_raw_mode(status.st_mode).is_dir(),
        status.st_uid,
        normalize_mode(status.st_mode),
        true,
    )?;
    Ok(DirectoryIdentity {
        device: normalize_device(status.st_dev)
            .map_err(|_| StateDirectoryProvisionError::UnsafeDirectory)?,
        inode: status.st_ino,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_secure_directory(
    directory: &File,
    exact_owner_mode: bool,
) -> Result<DirectoryIdentity, StateDirectoryProvisionError> {
    let status = fstat(directory).map_err(|_| StateDirectoryProvisionError::Filesystem)?;
    validate_directory_status(
        FileType::from_raw_mode(status.st_mode).is_dir(),
        status.st_uid,
        normalize_mode(status.st_mode),
        exact_owner_mode,
    )?;
    Ok(DirectoryIdentity {
        device: normalize_device(status.st_dev)
            .map_err(|_| StateDirectoryProvisionError::UnsafeDirectory)?,
        inode: status.st_ino,
    })
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn validate_directory_status(
    is_directory: bool,
    owner: u32,
    mode: u32,
    exact_owner_mode: bool,
) -> Result<(), StateDirectoryProvisionError> {
    let permissions = mode & 0o777;
    let permissions_valid = if exact_owner_mode {
        permissions == 0o700
    } else {
        permissions & 0o022 == 0 && permissions & 0o500 == 0o500
    };
    if is_directory && owner == geteuid().as_raw() && permissions_valid {
        Ok(())
    } else {
        Err(StateDirectoryProvisionError::UnsafeDirectory)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn cleanup_created_directory(entry: &CreatedDirectory) -> Result<(), ()> {
    let current = open_directory_at(&entry.parent, &entry.name).map_err(|_| ())?;
    let held_identity = validate_secure_directory(&entry.held, true).map_err(|_| ())?;
    let current_identity = validate_secure_directory(&current, true).map_err(|_| ())?;
    if held_identity != entry.identity || current_identity != entry.identity {
        return Err(());
    }
    unlinkat(&entry.parent, &entry.name, AtFlags::REMOVEDIR).map_err(|_| ())?;
    entry.parent.sync_all().map_err(|_| ())
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn normalize_mode<T: Into<u32>>(raw: T) -> u32 {
    raw.into()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn normalize_device<T: TryInto<u64>>(raw: T) -> Result<u64, T::Error> {
    raw.try_into()
}

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
mod tests {
    use std::{
        error::Error as _,
        ffi::OsString,
        fs,
        os::unix::fs::{MetadataExt, PermissionsExt, symlink},
        path::{Path, PathBuf},
        sync::Mutex,
    };

    use tempfile::TempDir;

    use super::{
        ProvisionOperations, RuntimeStateDirectoryPlan, StateDirectoryProvisionError,
        provision_with_operations,
    };
    use crate::{
        InstanceId, RadrootsHostEnvironment, RadrootsPathProfile, RadrootsPathResolver,
        RadrootsPlatform, RuntimeContext, RuntimeContextBootstrap, RuntimeContextSource, ServiceId,
    };

    fn context(
        platform: RadrootsPlatform,
        profile: RadrootsPathProfile,
        root: &Path,
    ) -> RuntimeContext {
        let environment = match platform {
            RadrootsPlatform::Linux => RadrootsHostEnvironment {
                xdg_data_home: Some(root.to_path_buf()),
                xdg_config_home: Some(root.join("config-root")),
                xdg_state_home: Some(root.join("state-root")),
                xdg_cache_home: Some(root.join("cache-root")),
                xdg_runtime_dir: Some(root.join("runtime-root")),
                ..RadrootsHostEnvironment::default()
            },
            RadrootsPlatform::Macos => RadrootsHostEnvironment {
                home_dir: Some(root.to_path_buf()),
                ..RadrootsHostEnvironment::default()
            },
            _ => RadrootsHostEnvironment::default(),
        };
        let repo_local_root =
            matches!(profile, RadrootsPathProfile::RepoLocal).then(|| root.to_path_buf());
        let bootstrap = RuntimeContextBootstrap::new(
            profile,
            repo_local_root,
            if matches!(profile, RadrootsPathProfile::RepoLocal) {
                RuntimeContextSource::BootstrapCli
            } else {
                RuntimeContextSource::SafeDefault
            },
            RuntimeContextSource::BootstrapCli,
        )
        .expect("bootstrap");
        RuntimeContext::resolve(
            &RadrootsPathResolver::new(platform, environment),
            bootstrap,
            ServiceId::new("myc").expect("service"),
            InstanceId::new("primary").expect("instance"),
        )
        .expect("context")
    }

    fn prepare_state_root(context: &RuntimeContext) -> PathBuf {
        let root = context.paths().state_root().to_path_buf();
        fs::create_dir_all(&root).expect("state root");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).expect("root mode");
        root
    }

    #[test]
    fn repo_local_creates_only_the_exact_canonical_suffix() {
        let temporary = TempDir::new().expect("temporary root");
        let context = context(
            RadrootsPlatform::Linux,
            RadrootsPathProfile::RepoLocal,
            temporary.path(),
        );
        let state_root = prepare_state_root(&context);

        context
            .state_directory_plan()
            .expect("plan")
            .provision()
            .expect("provision");

        let expected = state_root.join("services/myc/primary");
        assert_eq!(context.paths().state(), expected);
        for directory in [
            state_root.join("services"),
            state_root.join("services/myc"),
            expected,
        ] {
            let metadata = fs::metadata(directory).expect("created directory");
            assert!(metadata.is_dir());
            assert_eq!(metadata.permissions().mode() & 0o777, 0o700);
        }
        assert_eq!(
            fs::read_dir(temporary.path())
                .expect("base inventory")
                .map(|entry| entry.expect("entry").file_name())
                .collect::<Vec<_>>(),
            vec![OsString::from("data")],
        );
    }

    #[test]
    fn linux_and_macos_interactive_profiles_create_the_exact_suffix() {
        let linux = TempDir::new().expect("linux root");
        let linux_context = context(
            RadrootsPlatform::Linux,
            RadrootsPathProfile::InteractiveUser,
            linux.path(),
        );
        let linux_root = prepare_state_root(&linux_context);
        linux_context
            .state_directory_plan()
            .expect("linux plan")
            .provision()
            .expect("linux provision");
        assert_eq!(
            linux_context.paths().state(),
            linux_root.join("services/myc/primary")
        );

        let macos = TempDir::new().expect("macOS home");
        let macos_context = context(
            RadrootsPlatform::Macos,
            RadrootsPathProfile::InteractiveUser,
            macos.path(),
        );
        let macos_root = prepare_state_root(&macos_context);
        macos_context
            .state_directory_plan()
            .expect("macOS plan")
            .provision()
            .expect("macOS provision");
        assert_eq!(
            macos_context.paths().state(),
            macos_root.join("services/myc/primary")
        );
    }

    #[test]
    fn existing_directories_are_validated_without_permission_repair() {
        let temporary = TempDir::new().expect("temporary root");
        let context = context(
            RadrootsPlatform::Linux,
            RadrootsPathProfile::RepoLocal,
            temporary.path(),
        );
        let root = prepare_state_root(&context);
        fs::create_dir_all(context.paths().state()).expect("existing suffix");
        fs::set_permissions(root.join("services/myc"), fs::Permissions::from_mode(0o755))
            .expect("safe existing mode");
        fs::set_permissions(context.paths().state(), fs::Permissions::from_mode(0o770))
            .expect("unsafe mode");

        assert_eq!(
            context.state_directory_plan().expect("plan").provision(),
            Err(StateDirectoryProvisionError::UnsafeDirectory)
        );
        assert_eq!(
            fs::metadata(context.paths().state())
                .expect("state metadata")
                .permissions()
                .mode()
                & 0o777,
            0o770
        );
        assert_eq!(
            fs::metadata(root.join("services/myc"))
                .expect("service metadata")
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }

    #[test]
    fn symlink_entries_are_rejected_without_following_or_repairing() {
        let temporary = TempDir::new().expect("temporary root");
        let context = context(
            RadrootsPlatform::Linux,
            RadrootsPathProfile::RepoLocal,
            temporary.path(),
        );
        let root = prepare_state_root(&context);
        let outside = temporary.path().join("outside");
        fs::create_dir(&outside).expect("outside");
        symlink(&outside, root.join("services")).expect("symlink");

        assert_eq!(
            context.state_directory_plan().expect("plan").provision(),
            Err(StateDirectoryProvisionError::DirectoryConflict)
        );
        assert!(
            outside
                .read_dir()
                .expect("outside inventory")
                .next()
                .is_none()
        );
    }

    #[test]
    fn service_host_is_existing_only() {
        let temporary = TempDir::new().expect("temporary root");
        let plan = RuntimeStateDirectoryPlan {
            profile: RadrootsPathProfile::ServiceHost,
            state_root: temporary.path().join("state-root"),
            service: ServiceId::new("myc").expect("service"),
            instance: InstanceId::new("primary").expect("instance"),
        };
        fs::create_dir(&plan.state_root).expect("state root");
        fs::set_permissions(&plan.state_root, fs::Permissions::from_mode(0o700))
            .expect("root mode");

        assert_eq!(
            plan.provision(),
            Err(StateDirectoryProvisionError::MissingDirectory)
        );
        assert!(!plan.state_root.join("services").exists());

        fs::create_dir_all(plan.state_root.join("services/myc/primary"))
            .expect("preprovisioned suffix");
        for directory in [
            plan.state_root.join("services"),
            plan.state_root.join("services/myc"),
            plan.state_root.join("services/myc/primary"),
        ] {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).expect("suffix mode");
        }
        plan.provision().expect("existing-only validation");
    }

    struct FailingOperations {
        fail_after: usize,
    }

    impl ProvisionOperations for FailingOperations {
        fn after_create(&self, component_index: usize) -> Result<(), StateDirectoryProvisionError> {
            if component_index == self.fail_after {
                Err(StateDirectoryProvisionError::Filesystem)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn partial_creation_failure_removes_only_exact_created_identities() {
        let temporary = TempDir::new().expect("temporary root");
        let context = context(
            RadrootsPlatform::Linux,
            RadrootsPathProfile::RepoLocal,
            temporary.path(),
        );
        let root = prepare_state_root(&context);
        let plan = context.state_directory_plan().expect("plan");

        assert_eq!(
            provision_with_operations(&plan, &FailingOperations { fail_after: 1 }),
            Err(StateDirectoryProvisionError::Filesystem)
        );
        assert!(!root.join("services").exists());
    }

    struct ReplacingOperations {
        created: PathBuf,
        displaced: PathBuf,
        ran: Mutex<bool>,
    }

    impl ProvisionOperations for ReplacingOperations {
        fn after_create(&self, component_index: usize) -> Result<(), StateDirectoryProvisionError> {
            if component_index == 0 {
                fs::rename(&self.created, &self.displaced).expect("displace created directory");
                fs::create_dir(&self.created).expect("replacement directory");
                fs::set_permissions(&self.created, fs::Permissions::from_mode(0o700))
                    .expect("replacement mode");
                *self.ran.lock().expect("replacement flag") = true;
                return Err(StateDirectoryProvisionError::Filesystem);
            }
            Ok(())
        }
    }

    #[test]
    fn cleanup_preserves_a_replacement_with_an_unmatched_identity() {
        let temporary = TempDir::new().expect("temporary root");
        let context = context(
            RadrootsPlatform::Linux,
            RadrootsPathProfile::RepoLocal,
            temporary.path(),
        );
        let root = prepare_state_root(&context);
        let operations = ReplacingOperations {
            created: root.join("services"),
            displaced: root.join("displaced-services"),
            ran: Mutex::new(false),
        };

        assert_eq!(
            provision_with_operations(&context.state_directory_plan().expect("plan"), &operations,),
            Err(StateDirectoryProvisionError::Cleanup)
        );
        assert!(*operations.ran.lock().expect("replacement flag"));
        assert!(operations.created.is_dir());
        assert_ne!(
            fs::metadata(&operations.created)
                .expect("replacement")
                .ino(),
            fs::metadata(&operations.displaced)
                .expect("displaced")
                .ino(),
        );
    }

    struct ReplacingRootOperations {
        root: PathBuf,
        displaced: PathBuf,
    }

    impl ProvisionOperations for ReplacingRootOperations {
        fn after_create(&self, component_index: usize) -> Result<(), StateDirectoryProvisionError> {
            if component_index == 0 {
                fs::rename(&self.root, &self.displaced).expect("displace state root");
                fs::create_dir(&self.root).expect("replacement state root");
                fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))
                    .expect("replacement root mode");
                return Err(StateDirectoryProvisionError::Filesystem);
            }
            Ok(())
        }
    }

    #[test]
    fn state_root_replacement_blocks_cleanup_and_preserves_both_identities() {
        let temporary = TempDir::new().expect("temporary root");
        let context = context(
            RadrootsPlatform::Linux,
            RadrootsPathProfile::RepoLocal,
            temporary.path(),
        );
        let root = prepare_state_root(&context);
        let displaced = temporary.path().join("displaced-data");
        let operations = ReplacingRootOperations {
            root: root.clone(),
            displaced: displaced.clone(),
        };

        assert_eq!(
            provision_with_operations(&context.state_directory_plan().expect("plan"), &operations,),
            Err(StateDirectoryProvisionError::Cleanup)
        );
        assert!(root.is_dir());
        assert!(displaced.join("services").is_dir());
        assert_ne!(
            fs::metadata(root).expect("replacement root").ino(),
            fs::metadata(displaced).expect("displaced root").ino(),
        );
    }

    #[test]
    fn plan_and_errors_do_not_render_paths_or_identities() {
        let temporary = TempDir::new().expect("temporary root");
        let context = context(
            RadrootsPlatform::Linux,
            RadrootsPathProfile::RepoLocal,
            temporary.path(),
        );
        let plan = context.state_directory_plan().expect("plan");
        let rendered = format!("{plan:?}");
        assert!(!rendered.contains(temporary.path().to_string_lossy().as_ref()));
        assert!(!rendered.contains("myc"));
        assert!(!rendered.contains("primary"));
        for failure in [
            StateDirectoryProvisionError::InvalidPlan,
            StateDirectoryProvisionError::UnsupportedPlatform,
            StateDirectoryProvisionError::UnsupportedProfile,
            StateDirectoryProvisionError::StateRootUnavailable,
            StateDirectoryProvisionError::MissingDirectory,
            StateDirectoryProvisionError::DirectoryConflict,
            StateDirectoryProvisionError::UnsafeDirectory,
            StateDirectoryProvisionError::Filesystem,
            StateDirectoryProvisionError::Cleanup,
        ] {
            assert!(failure.source().is_none());
            assert!(!format!("{failure:?} {failure}").contains("/"));
        }
    }
}
