//! Objectives and progression, separate from mechanical rules.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarathonConfig {
    pub target_lines: u32,
    pub lines_per_level: u32,
    pub maximum_level: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameModeConfig {
    Marathon(MarathonConfig),
}

impl GameModeConfig {
    pub fn marathon() -> Self {
        Self::Marathon(MarathonConfig {
            target_lines: 150,
            lines_per_level: 10,
            maximum_level: 15,
        })
    }

    pub fn marathon_config(self) -> MarathonConfig {
        match self {
            Self::Marathon(config) => config,
        }
    }
}
