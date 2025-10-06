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
