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

#[cfg(test)]
mod tests {
    use super::{Backoff, BackoffConfig, jitter_ms};
    use core::time::Duration;

    #[test]
    fn default_values_round_trip() {
        let cfg: BackoffConfig =
            toml::from_str("").expect("backoff config defaults should deserialize");
        assert_eq!(cfg.base_ms, 500);
        assert_eq!(cfg.max_ms, 30_000);
        assert_eq!(cfg.factor, 2);
        assert_eq!(cfg.jitter_ms, 0);

        let cfg_default = BackoffConfig::default();
        assert_eq!(cfg_default.base_ms, 500);
        assert_eq!(cfg_default.max_ms, 30_000);
        assert_eq!(cfg_default.factor, 2);
        assert_eq!(cfg_default.jitter_ms, 0);
    }

    #[test]
    fn alias_fields_deserialize() {
        let cfg: BackoffConfig = toml::from_str(
            r#"
reconnect_base_ms = 10
reconnect_max_ms = 100
reconnect_factor = 3
reconnect_jitter_ms = 5
"#,
        )
        .expect("backoff aliases should deserialize");

        assert_eq!(cfg.base_ms, 10);
        assert_eq!(cfg.max_ms, 100);
        assert_eq!(cfg.factor, 3);
        assert_eq!(cfg.jitter_ms, 5);
    }

    #[test]
    fn delay_for_attempt_applies_bounds_and_factor_defaults() {
        let cfg = BackoffConfig {
            base_ms: 0,
            max_ms: 0,
            factor: 0,
            jitter_ms: 0,
        };
        assert_eq!(cfg.delay_for_attempt(1), Duration::from_millis(1));
        assert_eq!(cfg.delay_for_attempt(8), Duration::from_millis(1));
    }

    #[test]
    fn delay_for_attempt_caps_growth_to_max() {
        let cfg = BackoffConfig {
            base_ms: 100,
            max_ms: 1_000,
            factor: 2,
            jitter_ms: 0,
        };

        assert_eq!(cfg.delay_for_attempt(1), Duration::from_millis(100));
        assert_eq!(cfg.delay_for_attempt(2), Duration::from_millis(200));
        assert_eq!(cfg.delay_for_attempt(3), Duration::from_millis(400));
        assert_eq!(cfg.delay_for_attempt(4), Duration::from_millis(800));
        assert_eq!(cfg.delay_for_attempt(5), Duration::from_millis(1_000));
        assert_eq!(cfg.delay_for_attempt(16), Duration::from_millis(1_000));
    }

    #[test]
    fn delay_for_attempt_applies_jitter_without_exceeding_max() {
        let cfg = BackoffConfig {
            base_ms: 100,
            max_ms: 500,
            factor: 2,
            jitter_ms: 50,
        };

        let delay = cfg.delay_for_attempt(2).as_millis() as u64;
        assert!(delay >= 200);
        assert!(delay <= 250);
    }

    #[test]
    fn stateful_backoff_tracks_attempts_and_reset() {
        let cfg = BackoffConfig {
            base_ms: 5,
            max_ms: 50,
            factor: 2,
            jitter_ms: 0,
        };
        let mut backoff = Backoff::new(cfg);

        assert_eq!(backoff.attempt(), 0);
        assert_eq!(backoff.next_delay(), Duration::from_millis(5));
        assert_eq!(backoff.attempt(), 1);
        assert_eq!(backoff.next_delay(), Duration::from_millis(10));
        assert_eq!(backoff.attempt(), 2);

        backoff.reset();
        assert_eq!(backoff.attempt(), 0);
        assert_eq!(backoff.next_delay(), Duration::from_millis(5));
    }

    #[test]
    fn jitter_ms_bounds_output() {
        assert_eq!(jitter_ms(0), 0);
        let jitter = jitter_ms(7);
        assert!(jitter <= 7);
    }
}
