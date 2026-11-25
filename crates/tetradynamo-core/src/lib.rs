//! Dynamo Core is a deterministic, frontend-independent falling-block engine.
//!
//! [`Game::new`] preserves the original Guideline Marathon defaults. New
//! integrations can compose a [`GameConfig`] from a mechanical ruleset and a
//! game-mode objective with [`Game::with_config`].

pub mod board;
mod engine;
pub mod mode;
pub mod replay;
pub mod rules;

pub use board::Board;
pub use engine::*;
pub use mode::{GameModeConfig, MarathonConfig, SprintConfig};
pub use replay::{
    LifecycleAction, Replay, ReplayError, ReplayPlayer, ReplayRecorder, ReplayVerification,
    DEFAULT_CHECKPOINT_INTERVAL_TICKS, EVENT_HASH_ALGORITHM_VERSION, REPLAY_FORMAT_VERSION,
    SIMULATION_COMPATIBILITY_VERSION, STATE_HASH_ALGORITHM_VERSION,
};
pub use rules::{
    GravityCurve, HoldConfig, InputConfig, RandomizerConfig, RotationSystem, RulesetConfig,
    ScoringConfig, SpinConfig, TimingConfig,
};

/// Immutable simulation choices. Board geometry and simulation frequency are
/// deliberately fixed in this early-alpha API.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GameConfig {
    pub rules: RulesetConfig,
    pub mode: GameModeConfig,
}

impl GameConfig {
    /// The behavior used by `Game::new(seed)` and both shipped frontends.
    pub fn guideline_marathon() -> Self {
        Self {
            rules: RulesetConfig::guideline(),
            mode: GameModeConfig::marathon(),
        }
    }

    /// Guideline mechanics with the fixed-speed 40-line Sprint objective.
    pub fn guideline_sprint() -> Self {
        Self {
            rules: RulesetConfig::guideline(),
            mode: GameModeConfig::sprint(),
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        let input = self.rules.input;
        if input.soft_drop_divisor == 0 {
            return Err(ConfigError::ZeroSoftDropDivisor);
        }
        if input.arr_ticks == 0 {
            return Err(ConfigError::ZeroArrTicks);
        }
        if self.rules.scoring.back_to_back_multiplier_denominator == 0 {
            return Err(ConfigError::ZeroBackToBackDenominator);
        }
        match self.mode {
            GameModeConfig::Marathon(config) => {
                if config.target_lines == 0 {
                    return Err(ConfigError::ZeroTargetLines);
                }
                if config.lines_per_level == 0 {
                    return Err(ConfigError::ZeroLinesPerLevel);
                }
                if config.maximum_level == 0 {
                    return Err(ConfigError::ZeroMaximumLevel);
                }
            }
            GameModeConfig::Sprint(config) => {
                if config.target_lines == 0 {
                    return Err(ConfigError::ZeroTargetLines);
                }
                if config.fixed_level == 0 {
                    return Err(ConfigError::ZeroFixedLevel);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    ZeroSoftDropDivisor,
    ZeroArrTicks,
    ZeroBackToBackDenominator,
    ZeroTargetLines,
    ZeroLinesPerLevel,
    ZeroMaximumLevel,
    ZeroFixedLevel,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid game configuration: {self:?}")
    }
}

impl std::error::Error for ConfigError {}

impl Default for GameConfig {
    fn default() -> Self {
        Self::guideline_marathon()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprint_preset_has_a_fixed_40_line_objective() {
        let config = GameConfig::guideline_sprint();
        assert_eq!(config.mode.target_lines(), 40);
        assert_eq!(config.mode.level_for_lines(0), 1);
        assert_eq!(config.mode.level_for_lines(39), 1);
        assert_eq!(config.mode.level_for_lines(40), 1);
        assert_eq!(config.mode.id(), "sprint");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn sprint_configuration_affects_deterministic_state() {
        let marathon = Game::with_config(7, GameConfig::guideline_marathon());
        let sprint = Game::with_config(7, GameConfig::guideline_sprint());
        assert_ne!(marathon.state_hash(), sprint.state_hash());
    }

    #[test]
    fn sprint_rejects_zero_target_or_fixed_level() {
        let mut config = GameConfig::guideline_sprint();
        config.mode = GameModeConfig::Sprint(SprintConfig {
            target_lines: 0,
            fixed_level: 1,
        });
        assert_eq!(config.validate(), Err(ConfigError::ZeroTargetLines));
        config.mode = GameModeConfig::Sprint(SprintConfig {
            target_lines: 40,
            fixed_level: 0,
        });
        assert_eq!(config.validate(), Err(ConfigError::ZeroFixedLevel));
    }
}
