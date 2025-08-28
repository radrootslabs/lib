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

    #[allow(unreachable_code)]
    pub fn build(self) -> Result<NetHandle> {
        let net = Net::new(self.config.clone());

        #[cfg(feature = "rt")]
        {
            let mut net = net;
            if self.manage_runtime {
                net.init_managed_runtime(None)?;
            }
            return Ok(NetHandle::from_inner(net));
        }

        Ok(NetHandle::from_inner(net))
    }
}
