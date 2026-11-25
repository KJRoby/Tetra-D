//! Objectives and progression, separate from mechanical rules.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarathonConfig {
    pub target_lines: u32,
    pub lines_per_level: u32,
    pub maximum_level: u32,
}

/// A fixed-speed, line-clear race.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SprintConfig {
    pub target_lines: u32,
    pub fixed_level: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum GameModeConfig {
    Marathon(MarathonConfig),
    Sprint(SprintConfig),
}

impl GameModeConfig {
    pub fn marathon() -> Self {
        Self::Marathon(MarathonConfig {
            target_lines: 150,
            lines_per_level: 10,
            maximum_level: 15,
        })
    }

    pub fn sprint() -> Self {
        Self::Sprint(SprintConfig {
            target_lines: 40,
            fixed_level: 1,
        })
    }

    pub fn target_lines(self) -> u32 {
        match self {
            Self::Marathon(config) => config.target_lines,
            Self::Sprint(config) => config.target_lines,
        }
    }

    pub fn level_for_lines(self, lines: u32) -> u32 {
        match self {
            Self::Marathon(config) => {
                (1 + lines / config.lines_per_level).min(config.maximum_level)
            }
            Self::Sprint(config) => config.fixed_level,
        }
    }

    /// Stable, presentation-safe identifier for menus and replay filenames.
    pub fn id(self) -> &'static str {
        match self {
            Self::Marathon(_) => "marathon",
            Self::Sprint(_) => "sprint",
        }
    }
}
