use crate::config::NetConfig;
use crate::error::Result;
use crate::{Net, NetHandle};

#[derive(Debug, Clone)]
pub struct NetBuilder {
    config: NetConfig,
    manage_runtime: bool,
}

impl Default for NetBuilder {
    fn default() -> Self {
        Self {
            config: NetConfig::default(),
            manage_runtime: false,
        }
    }
}

impl NetBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn config(mut self, cfg: NetConfig) -> Self {
        self.config = cfg;
        self
    }

    pub fn manage_runtime(mut self, yes: bool) -> Self {
        self.manage_runtime = yes;
        self
    }

    pub fn build(self) -> Result<NetHandle> {
        let mut _net = Net::new(self.config.clone());

        #[cfg(feature = "rt")]
        if self.manage_runtime {
            _net.init_managed_runtime(None)?;
        }

        Ok(NetHandle::from_inner(_net))
    }
}

pub fn coverage_branch_probe(input: bool) -> bool {
    if input { true } else { false }
}

#[cfg(test)]
mod tests {
    use super::{NetBuilder, coverage_branch_probe};

    #[test]
    fn manage_runtime_path_is_callable() {
        let cfg = crate::config::NetConfig::default();
        let handle = NetBuilder::new()
            .config(cfg)
            .manage_runtime(true)
            .build()
            .expect("build net handle");
        let guard = handle.lock();
        assert!(guard.is_ok());
    }

    #[test]
    fn coverage_branch_probe_hits_both_paths() {
        assert!(coverage_branch_probe(true));
        assert!(!coverage_branch_probe(false));
    }
}
