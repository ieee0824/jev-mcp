use std::{fmt, time::Duration};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionProfile {
    Reliable,
    Interactive,
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileSettings {
    pub per_attempt_timeout: Duration,
    pub total_budget: Duration,
    pub max_retries: usize,
    pub retry_delay: Duration,
    pub retry_timeouts: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct ProfileSet {
    reliable: ProfileSettings,
    interactive: ProfileSettings,
}

impl ExecutionProfile {
    pub fn from_env() -> Result<Self, String> {
        match std::env::var("JEV_PROFILE") {
            Err(std::env::VarError::NotPresent) => Ok(Self::Reliable),
            Err(std::env::VarError::NotUnicode(_)) => Err("JEV_PROFILE must be valid UTF-8".into()),
            Ok(value) => Self::parse(&value),
        }
    }

    fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "reliable" => Ok(Self::Reliable),
            "interactive" => Ok(Self::Interactive),
            _ => Err("JEV_PROFILE must be reliable or interactive".into()),
        }
    }
}

impl fmt::Display for ExecutionProfile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Reliable => "reliable",
            Self::Interactive => "interactive",
        })
    }
}

impl Default for ProfileSet {
    fn default() -> Self {
        Self {
            reliable: ProfileSettings {
                per_attempt_timeout: Duration::from_secs(30),
                total_budget: Duration::from_secs(60),
                max_retries: 2,
                retry_delay: Duration::from_millis(500),
                retry_timeouts: false,
            },
            interactive: ProfileSettings {
                per_attempt_timeout: Duration::from_millis(1_500),
                total_budget: Duration::from_secs(3),
                max_retries: 1,
                retry_delay: Duration::from_millis(150),
                retry_timeouts: true,
            },
        }
    }
}

impl ProfileSet {
    pub fn get(self, profile: ExecutionProfile) -> ProfileSettings {
        match profile {
            ExecutionProfile::Reliable => self.reliable,
            ExecutionProfile::Interactive => self.interactive,
        }
    }

    #[cfg(test)]
    pub fn set(&mut self, profile: ExecutionProfile, settings: ProfileSettings) {
        match profile {
            ExecutionProfile::Reliable => self.reliable = settings,
            ExecutionProfile::Interactive => self.interactive = settings,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_preserve_reliable_behavior_and_bound_interactive_latency() {
        let profiles = ProfileSet::default();
        let reliable = profiles.get(ExecutionProfile::Reliable);
        assert_eq!(reliable.per_attempt_timeout, Duration::from_secs(30));
        assert_eq!(reliable.total_budget, Duration::from_secs(60));
        assert_eq!(reliable.max_retries, 2);
        assert!(!reliable.retry_timeouts);

        let interactive = profiles.get(ExecutionProfile::Interactive);
        assert_eq!(
            interactive.per_attempt_timeout,
            Duration::from_millis(1_500)
        );
        assert_eq!(interactive.total_budget, Duration::from_secs(3));
        assert_eq!(interactive.max_retries, 1);
        assert!(interactive.retry_timeouts);
    }

    #[test]
    fn profile_names_match_environment_values() {
        assert_eq!(
            ExecutionProfile::parse("reliable").unwrap(),
            ExecutionProfile::Reliable
        );
        assert_eq!(
            ExecutionProfile::parse(" INTERACTIVE ").unwrap(),
            ExecutionProfile::Interactive
        );
        assert!(ExecutionProfile::parse("fast").is_err());
    }
}
