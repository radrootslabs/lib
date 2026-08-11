use std::path::{Path, PathBuf};

use crate::{InstanceId, ServiceId, roots::RadrootsPaths};

/// Canonical directories owned by one validated service instance.
///
/// Callers cannot construct this value without the crate's validated identity
/// and root derivation boundary.
///
/// ```compile_fail
/// use std::path::PathBuf;
/// use radroots_runtime_paths::RadrootsServiceInstancePaths;
///
/// let _forged = RadrootsServiceInstancePaths {
///     config: PathBuf::from("/tmp/escape"),
///     state: PathBuf::from("/tmp/escape"),
///     cache: PathBuf::from("/tmp/escape"),
///     logs: PathBuf::from("/tmp/escape"),
///     run: PathBuf::from("/tmp/escape"),
///     secrets: PathBuf::from("/tmp/escape"),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadrootsServiceInstancePaths {
    config: PathBuf,
    state: PathBuf,
    cache: PathBuf,
    logs: PathBuf,
    run: PathBuf,
    secrets: PathBuf,
}

impl RadrootsServiceInstancePaths {
    pub(crate) fn from_resolved_roots(
        roots: &RadrootsPaths,
        service: &ServiceId,
        instance: &InstanceId,
    ) -> Self {
        let relative = Path::new("services")
            .join(service.as_str())
            .join(instance.as_str());
        Self {
            config: roots.config.join(&relative),
            state: roots.data.join(&relative),
            cache: roots.cache.join(&relative),
            logs: roots.logs.join(&relative),
            run: roots.run.join(&relative),
            secrets: roots.secrets.join(relative),
        }
    }

    #[must_use]
    pub fn config(&self) -> &Path {
        &self.config
    }

    #[must_use]
    pub fn state(&self) -> &Path {
        &self.state
    }

    #[must_use]
    pub fn cache(&self) -> &Path {
        &self.cache
    }

    #[must_use]
    pub fn logs(&self) -> &Path {
        &self.logs
    }

    #[must_use]
    pub fn run(&self) -> &Path {
        &self.run
    }

    #[must_use]
    pub fn secrets(&self) -> &Path {
        &self.secrets
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::RadrootsServiceInstancePaths;
    use crate::{InstanceId, ServiceId, roots::RadrootsPaths};

    #[test]
    fn canonical_service_instance_suffix_uses_validated_ids() {
        let roots = RadrootsPaths::from_base_root("/repo/.local/radroots");
        let paths = RadrootsServiceInstancePaths::from_resolved_roots(
            &roots,
            &ServiceId::new("myc").expect("service"),
            &InstanceId::new("primary").expect("instance"),
        );
        let suffix = PathBuf::from("services/myc/primary");

        assert_eq!(paths.config(), roots.config.join(&suffix));
        assert_eq!(paths.state(), roots.data.join(&suffix));
        assert_eq!(paths.cache(), roots.cache.join(&suffix));
        assert_eq!(paths.logs(), roots.logs.join(&suffix));
        assert_eq!(paths.run(), roots.run.join(&suffix));
        assert_eq!(paths.secrets(), roots.secrets.join(&suffix));
    }
}
