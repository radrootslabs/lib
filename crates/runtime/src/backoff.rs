use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

fn default_base_ms() -> u64 {
    500
}

fn default_max_ms() -> u64 {
    30_000
}

fn default_factor() -> u32 {
    2
}

fn default_jitter_ms() -> u64 {
    0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackoffConfig {
    #[serde(default = "default_base_ms", alias = "reconnect_base_ms")]
    pub base_ms: u64,
    #[serde(default = "default_max_ms", alias = "reconnect_max_ms")]
    pub max_ms: u64,
    #[serde(default = "default_factor", alias = "reconnect_factor")]
    pub factor: u32,
    #[serde(default = "default_jitter_ms", alias = "reconnect_jitter_ms")]
    pub jitter_ms: u64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            base_ms: default_base_ms(),
            max_ms: default_max_ms(),
            factor: default_factor(),
            jitter_ms: default_jitter_ms(),
        }
    }
}

impl BackoffConfig {
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base = self.base_ms.max(1);
        let max = self.max_ms.max(base);
        let factor = self.factor.max(1) as u64;

        let mut delay = base;
        let steps = attempt.saturating_sub(1).min(10);
        for _ in 0..steps {
            delay = delay.saturating_mul(factor).min(max);
        }

        if self.jitter_ms > 0 {
            let jitter = jitter_ms(self.jitter_ms);
            delay = delay.saturating_add(jitter).min(max);
        }

        Duration::from_millis(delay)
    }
}

#[derive(Debug, Clone)]
pub struct Backoff {
    cfg: BackoffConfig,
    attempt: u32,
}

impl Backoff {
    pub fn new(cfg: BackoffConfig) -> Self {
        Self { cfg, attempt: 0 }
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    pub fn next_delay(&mut self) -> Duration {
        let attempt = self.attempt.saturating_add(1);
        self.attempt = attempt;
        self.cfg.delay_for_attempt(attempt)
    }

    pub fn attempt(&self) -> u32 {
        self.attempt
    }
}

fn jitter_ms(max: u64) -> u64 {
    if max == 0 {
        return 0;
    }
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    nanos % (max + 1)
}
