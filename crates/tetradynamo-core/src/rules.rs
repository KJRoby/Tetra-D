//! Mechanical rules independent of objectives and progression.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RotationSystem {
    GuidelineSrs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RandomizerConfig {
    SevenBag,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GravityCurve {
    Guideline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimingConfig {
    pub ready_ticks: u32,
    pub countdown_ticks: u32,
    pub resume_countdown_ticks: u32,
    pub lock_delay_ticks: u32,
    pub clear_delay_ticks: u32,
    pub clear_notice_ticks: u32,
    pub max_lock_resets: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputConfig {
    pub das_ticks: u32,
    pub arr_ticks: u32,
    pub soft_drop_divisor: u32,
    pub minimum_soft_drop_ticks: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HoldConfig {
    pub enabled: bool,
    pub once_per_piece: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpinConfig {
    pub detect_t_spins: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoringConfig {
    pub back_to_back_multiplier_numerator: u32,
    pub back_to_back_multiplier_denominator: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulesetConfig {
    pub rotation: RotationSystem,
    pub randomizer: RandomizerConfig,
    pub gravity: GravityCurve,
    pub timing: TimingConfig,
    pub input: InputConfig,
    pub hold: HoldConfig,
    pub spins: SpinConfig,
    pub scoring: ScoringConfig,
}

impl RulesetConfig {
    pub fn guideline() -> Self {
        Self {
            rotation: RotationSystem::GuidelineSrs,
            randomizer: RandomizerConfig::SevenBag,
            gravity: GravityCurve::Guideline,
            timing: TimingConfig {
                ready_ticks: 1_500,
                countdown_ticks: 3_000,
                resume_countdown_ticks: 3_000,
                lock_delay_ticks: 500,
                clear_delay_ticks: 500,
                clear_notice_ticks: 1_000,
                max_lock_resets: 15,
            },
            input: InputConfig {
                das_ticks: 167,
                arr_ticks: 33,
                soft_drop_divisor: 20,
                minimum_soft_drop_ticks: 17,
            },
            hold: HoldConfig {
                enabled: true,
                once_per_piece: true,
            },
            spins: SpinConfig {
                detect_t_spins: true,
            },
            scoring: ScoringConfig {
                back_to_back_multiplier_numerator: 3,
                back_to_back_multiplier_denominator: 2,
            },
        }
    }
}
