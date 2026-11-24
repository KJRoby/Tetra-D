//! Dynamo Core is a deterministic, frontend-independent falling-block engine.
//!
//! [`Game::new`] preserves the original Guideline Marathon defaults. New
//! integrations can compose a [`GameConfig`] from a mechanical ruleset and a
//! game-mode objective with [`Game::with_config`].

pub mod board;
mod engine;
pub mod mode;
pub mod rules;

pub use board::Board;
pub use engine::*;
pub use mode::{GameModeConfig, MarathonConfig};
pub use rules::{
    GravityCurve, HoldConfig, InputConfig, RandomizerConfig, RotationSystem, RulesetConfig,
    ScoringConfig, SpinConfig, TimingConfig,
};

/// Immutable simulation choices. Board geometry and simulation frequency are
/// deliberately fixed in this early-alpha API.
#[derive(Clone, Debug, PartialEq, Eq)]
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
}

impl Default for GameConfig {
    fn default() -> Self {
        Self::guideline_marathon()
    }
}
