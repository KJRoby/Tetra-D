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
pub use mode::{GameModeConfig, MarathonConfig};
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
        let marathon = self.mode.marathon_config();
        if marathon.lines_per_level == 0 {
            return Err(ConfigError::ZeroLinesPerLevel);
        }
        if marathon.maximum_level == 0 {
            return Err(ConfigError::ZeroMaximumLevel);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    ZeroSoftDropDivisor,
    ZeroArrTicks,
    ZeroBackToBackDenominator,
    ZeroLinesPerLevel,
    ZeroMaximumLevel,
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
