use serde::Serialize;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize)]
pub struct BuildInfo {
    pub crate_name: &'static str,
    pub crate_version: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rustc: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_sha: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_time_unix: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetInfo {
    pub build: BuildInfo,
}

pub struct Net {
    pub info: NetInfo,
    pub config: crate::config::NetConfig,

    #[cfg(feature = "rt")]
    pub rt: Option<tokio::runtime::Runtime>,
}

impl Net {
    pub fn new(cfg: crate::config::NetConfig) -> Self {
        Self {
            info: NetInfo {
                build: BuildInfo {
                    crate_name: env!("CARGO_PKG_NAME"),
                    crate_version: env!("CARGO_PKG_VERSION"),
                    rustc: option_env!("RUSTC_VERSION"),
                    profile: option_env!("PROFILE"),
                    git_sha: option_env!("GIT_HASH"),
                    build_time_unix: option_env!("BUILD_TIME_UNIX").and_then(|s| s.parse().ok()),
                },
            },
            config: cfg,
            #[cfg(feature = "rt")]
            rt: None,
        }
    }

    #[cfg(feature = "rt")]
    pub fn init_managed_runtime(&mut self, worker_threads: Option<usize>) -> Result<()> {
        if self.rt.is_some() {
            return Ok(());
        }

        let threads = worker_threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
                .max(1)
        });

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(threads)
            .enable_all()
            .build()
            .map_err(|e| Error::msg(format!("failed to build tokio runtime: {e}")))?;

        self.rt = Some(rt);
        Ok(())
    }
}

#[derive(Clone)]
pub struct NetHandle(Arc<Mutex<Net>>);

impl NetHandle {
    pub fn from_inner(inner: Net) -> Self {
        Self(Arc::new(Mutex::new(inner)))
    }

    pub fn lock(&self) -> Result<MutexGuard<'_, Net>> {
        self.0.lock().map_err(|_| Error::Poisoned)
    }
}

#[cfg(test)]
mod tests {
    use crate::builder::NetBuilder;

    #[test]
    fn builds_minimal() {
        let cfg = crate::config::NetConfig::default();
        let handle = NetBuilder::new().config(cfg).build();
        assert!(handle.is_ok());
    }

    #[test]
    fn lock_is_ok() {
        let cfg = crate::config::NetConfig::default();
        let handle = NetBuilder::new().config(cfg).build().unwrap();
        let guard = handle.lock();
        assert!(guard.is_ok());
    }

    #[cfg(feature = "rt")]
    #[test]
    fn builds_with_managed_rt() {
        let cfg = crate::config::NetConfig::default();
        let handle = crate::builder::NetBuilder::new()
            .config(cfg)
            .manage_runtime(true)
            .build()
            .expect("build with runtime");

        let rt_present = handle.lock().unwrap().rt.is_some();
        assert!(rt_present);
    }
}
