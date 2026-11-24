//! Frontend-independent deterministic recording, playback, and verification.

use crate::{
    Command, ConfigError, Game, GameConfig, GameEvent, GameSnapshot, Phase, StepError, StepResult,
    TICK_HZ,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

pub const REPLAY_FORMAT_VERSION: u32 = 1;
pub const SIMULATION_COMPATIBILITY_VERSION: u32 = 1;
pub const STATE_HASH_ALGORITHM_VERSION: u32 = 1;
pub const EVENT_HASH_ALGORITHM_VERSION: u32 = 1;
pub const DEFAULT_CHECKPOINT_INTERVAL_TICKS: u64 = 1_000;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleAction {
    StartWithSeed { seed: u64 },
    RestartWithSeed { seed: u64 },
    ReturnToTitle,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayFrame {
    pub tick: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifecycle: Vec<LifecycleAction>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<Command>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_hash: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayCheckpoint {
    /// Number of simulation steps completed when this hash was produced.
    pub tick: u64,
    pub state_hash: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayResult {
    pub phase: Phase,
    pub score: u64,
    pub lines: u32,
    pub level: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayFinal {
    pub timeline_ticks: u64,
    pub simulation_ticks: u64,
    pub state_hash: u64,
    pub result: ReplayResult,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub terminal_lifecycle: Vec<LifecycleAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_event_hash: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Replay {
    pub format_version: u32,
    pub simulation_compatibility_version: u32,
    pub state_hash_algorithm_version: u32,
    pub event_hash_algorithm_version: u32,
    pub tick_hz: u32,
    pub initial_seed: u64,
    pub config: GameConfig,
    pub checkpoint_interval_ticks: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<ReplayFrame>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checkpoints: Vec<ReplayCheckpoint>,
    pub final_state: ReplayFinal,
}

#[derive(Debug)]
pub enum ReplayError {
    Json(String),
    Config(ConfigError),
    Invalid(String),
    Step(StepError),
    EventMismatch {
        tick: u64,
        expected: u64,
        actual: u64,
    },
    CheckpointMismatch {
        tick: u64,
        expected: u64,
        actual: u64,
    },
    FinalMismatch {
        expected: u64,
        actual: u64,
    },
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "invalid replay JSON: {error}"),
            Self::Config(error) => write!(f, "invalid replay configuration: {error}"),
            Self::Invalid(error) => write!(f, "invalid replay: {error}"),
            Self::Step(error) => write!(f, "replay command batch failed: {error:?}"),
            Self::EventMismatch {
                tick,
                expected,
                actual,
            } => write!(
                f,
                "event hash mismatch at tick {tick}: expected {expected:016x}, got {actual:016x}"
            ),
            Self::CheckpointMismatch {
                tick,
                expected,
                actual,
            } => write!(
                f,
                "state hash mismatch at tick {tick}: expected {expected:016x}, got {actual:016x}"
            ),
            Self::FinalMismatch { expected, actual } => write!(
                f,
                "final state hash mismatch: expected {expected:016x}, got {actual:016x}"
            ),
        }
    }
}

impl std::error::Error for ReplayError {}

impl From<ConfigError> for ReplayError {
    fn from(value: ConfigError) -> Self {
        Self::Config(value)
    }
}
impl From<StepError> for ReplayError {
    fn from(value: StepError) -> Self {
        Self::Step(value)
    }
}

impl Replay {
    pub fn to_json_pretty(&self) -> Result<String, ReplayError> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|error| ReplayError::Json(error.to_string()))
    }

    pub fn from_json(json: &str) -> Result<Self, ReplayError> {
        let replay: Self =
            serde_json::from_str(json).map_err(|error| ReplayError::Json(error.to_string()))?;
        replay.validate()?;
        Ok(replay)
    }

    pub fn player(&self) -> Result<ReplayPlayer, ReplayError> {
        ReplayPlayer::new(self.clone())
    }

    pub fn verify(&self) -> Result<ReplayVerification, ReplayError> {
        let mut player = self.player()?;
        while player.advance()?.is_some() {}
        player.finish()
    }

    pub fn validate(&self) -> Result<(), ReplayError> {
        if self.format_version != REPLAY_FORMAT_VERSION {
            return Err(ReplayError::Invalid(format!(
                "unsupported format version {}",
                self.format_version
            )));
        }
        if self.simulation_compatibility_version != SIMULATION_COMPATIBILITY_VERSION {
            return Err(ReplayError::Invalid(format!(
                "unsupported simulation compatibility version {}",
                self.simulation_compatibility_version
            )));
        }
        if self.state_hash_algorithm_version != STATE_HASH_ALGORITHM_VERSION {
            return Err(ReplayError::Invalid(format!(
                "unsupported state hash algorithm version {}",
                self.state_hash_algorithm_version
            )));
        }
        if self.event_hash_algorithm_version != EVENT_HASH_ALGORITHM_VERSION {
            return Err(ReplayError::Invalid(format!(
                "unsupported event hash algorithm version {}",
                self.event_hash_algorithm_version
            )));
        }
        if self.tick_hz != TICK_HZ {
            return Err(ReplayError::Invalid(format!(
                "tick rate {} does not match core rate {TICK_HZ}",
                self.tick_hz
            )));
        }
        if self.checkpoint_interval_ticks == 0 {
            return Err(ReplayError::Invalid(
                "checkpoint interval must be nonzero".into(),
            ));
        }
        self.config.validate()?;
        let mut previous = None;
        for frame in &self.frames {
            if frame.tick >= self.final_state.timeline_ticks {
                return Err(ReplayError::Invalid(format!(
                    "frame tick {} is outside the timeline",
                    frame.tick
                )));
            }
            if previous.is_some_and(|tick| tick >= frame.tick) {
                return Err(ReplayError::Invalid(
                    "frame ticks must be strictly ascending".into(),
                ));
            }
            if frame.commands.len() > crate::MAX_COMMANDS_PER_TICK {
                return Err(ReplayError::Invalid(format!(
                    "frame {} contains too many commands",
                    frame.tick
                )));
            }
            previous = Some(frame.tick);
        }
        let mut previous_checkpoint = None;
        for checkpoint in &self.checkpoints {
            if checkpoint.tick == 0 || checkpoint.tick > self.final_state.timeline_ticks {
                return Err(ReplayError::Invalid(format!(
                    "checkpoint tick {} is outside the timeline",
                    checkpoint.tick
                )));
            }
            if previous_checkpoint.is_some_and(|tick| tick >= checkpoint.tick) {
                return Err(ReplayError::Invalid(
                    "checkpoint ticks must be strictly ascending".into(),
                ));
            }
            previous_checkpoint = Some(checkpoint.tick);
        }
        Ok(())
    }
}

pub struct ReplayRecorder {
    game: Game,
    initial_seed: u64,
    config: GameConfig,
    checkpoint_interval_ticks: u64,
    timeline_ticks: u64,
    frames: Vec<ReplayFrame>,
    checkpoints: Vec<ReplayCheckpoint>,
    pending_lifecycle: Vec<LifecycleAction>,
    pending_events: Vec<GameEvent>,
}

impl ReplayRecorder {
    pub fn new(seed: u64, config: GameConfig) -> Result<Self, ReplayError> {
        Self::with_checkpoint_interval(seed, config, DEFAULT_CHECKPOINT_INTERVAL_TICKS)
    }
    pub fn with_checkpoint_interval(
        seed: u64,
        config: GameConfig,
        checkpoint_interval_ticks: u64,
    ) -> Result<Self, ReplayError> {
        if checkpoint_interval_ticks == 0 {
            return Err(ReplayError::Invalid(
                "checkpoint interval must be nonzero".into(),
            ));
        }
        let game = Game::try_with_config(seed, config.clone())?;
        Ok(Self {
            game,
            initial_seed: seed,
            config,
            checkpoint_interval_ticks,
            timeline_ticks: 0,
            frames: vec![],
            checkpoints: vec![],
            pending_lifecycle: vec![],
            pending_events: vec![],
        })
    }
    pub fn game(&self) -> &Game {
        &self.game
    }
    pub fn snapshot(&self) -> GameSnapshot {
        self.game.snapshot()
    }
    pub fn start_with_seed(&mut self, seed: u64) -> StepResult {
        self.lifecycle(LifecycleAction::StartWithSeed { seed })
    }
    pub fn restart_with_seed(&mut self, seed: u64) -> StepResult {
        self.lifecycle(LifecycleAction::RestartWithSeed { seed })
    }
    pub fn return_to_title(&mut self) -> StepResult {
        self.lifecycle(LifecycleAction::ReturnToTitle)
    }
    fn lifecycle(&mut self, action: LifecycleAction) -> StepResult {
        let result = apply_lifecycle(&mut self.game, &action);
        self.pending_lifecycle.push(action);
        self.pending_events.extend(result.events.iter().cloned());
        result
    }
    pub fn step(&mut self, commands: &[Command]) -> Result<StepResult, StepError> {
        let result = self.game.step(commands)?;
        let mut events = std::mem::take(&mut self.pending_events);
        events.extend(result.events.iter().cloned());
        if !self.pending_lifecycle.is_empty() || !commands.is_empty() || !events.is_empty() {
            self.frames.push(ReplayFrame {
                tick: self.timeline_ticks,
                lifecycle: std::mem::take(&mut self.pending_lifecycle),
                commands: commands.to_vec(),
                event_hash: (!events.is_empty()).then(|| event_hash(&events)),
            });
        }
        self.timeline_ticks += 1;
        if self
            .timeline_ticks
            .is_multiple_of(self.checkpoint_interval_ticks)
        {
            self.checkpoints.push(ReplayCheckpoint {
                tick: self.timeline_ticks,
                state_hash: self.game.state_hash(),
            });
        }
        Ok(result)
    }
    pub fn replay(&self) -> Replay {
        let terminal_event_hash =
            (!self.pending_events.is_empty()).then(|| event_hash(&self.pending_events));
        Replay {
            format_version: REPLAY_FORMAT_VERSION,
            simulation_compatibility_version: SIMULATION_COMPATIBILITY_VERSION,
            state_hash_algorithm_version: STATE_HASH_ALGORITHM_VERSION,
            event_hash_algorithm_version: EVENT_HASH_ALGORITHM_VERSION,
            tick_hz: TICK_HZ,
            initial_seed: self.initial_seed,
            config: self.config.clone(),
            checkpoint_interval_ticks: self.checkpoint_interval_ticks,
            frames: self.frames.clone(),
            checkpoints: self.checkpoints.clone(),
            final_state: ReplayFinal {
                timeline_ticks: self.timeline_ticks,
                simulation_ticks: self.game.snapshot().timers.simulation_ticks,
                state_hash: self.game.state_hash(),
                result: result_of(&self.game),
                terminal_lifecycle: self.pending_lifecycle.clone(),
                terminal_event_hash,
            },
        }
    }
}

pub struct ReplayPlayer {
    replay: Replay,
    game: Game,
    frames: BTreeMap<u64, ReplayFrame>,
    checkpoints: BTreeMap<u64, u64>,
    timeline_tick: u64,
    terminal_applied: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayVerification {
    pub final_state_hash: u64,
    pub timeline_ticks: u64,
    pub simulation_ticks: u64,
    pub result: ReplayResult,
}

impl ReplayPlayer {
    fn new(replay: Replay) -> Result<Self, ReplayError> {
        replay.validate()?;
        let game = Game::try_with_config(replay.initial_seed, replay.config.clone())?;
        Ok(Self {
            frames: replay
                .frames
                .iter()
                .cloned()
                .map(|frame| (frame.tick, frame))
                .collect(),
            checkpoints: replay
                .checkpoints
                .iter()
                .map(|checkpoint| (checkpoint.tick, checkpoint.state_hash))
                .collect(),
            replay,
            game,
            timeline_tick: 0,
            terminal_applied: false,
        })
    }
    pub fn game(&self) -> &Game {
        &self.game
    }
    pub fn timeline_tick(&self) -> u64 {
        self.timeline_tick
    }
    pub fn advance(&mut self) -> Result<Option<StepResult>, ReplayError> {
        if self.timeline_tick >= self.replay.final_state.timeline_ticks {
            return Ok(None);
        }
        let frame = self.frames.remove(&self.timeline_tick);
        let mut events = Vec::new();
        let commands = if let Some(frame) = &frame {
            for action in &frame.lifecycle {
                events.extend(apply_lifecycle(&mut self.game, action).events);
            }
            frame.commands.as_slice()
        } else {
            &[]
        };
        let result = self.game.step(commands)?;
        events.extend(result.events.iter().cloned());
        if let Some(expected) = frame.and_then(|frame| frame.event_hash) {
            let actual = event_hash(&events);
            if expected != actual {
                return Err(ReplayError::EventMismatch {
                    tick: self.timeline_tick,
                    expected,
                    actual,
                });
            }
        }
        self.timeline_tick += 1;
        if let Some(expected) = self.checkpoints.remove(&self.timeline_tick) {
            let actual = self.game.state_hash();
            if expected != actual {
                return Err(ReplayError::CheckpointMismatch {
                    tick: self.timeline_tick,
                    expected,
                    actual,
                });
            }
        }
        Ok(Some(result))
    }
    pub fn finish(&mut self) -> Result<ReplayVerification, ReplayError> {
        if self.timeline_tick != self.replay.final_state.timeline_ticks {
            return Err(ReplayError::Invalid(
                "replay has not reached its final tick".into(),
            ));
        }
        if !self.terminal_applied {
            let mut events = Vec::new();
            for action in &self.replay.final_state.terminal_lifecycle {
                events.extend(apply_lifecycle(&mut self.game, action).events);
            }
            if let Some(expected) = self.replay.final_state.terminal_event_hash {
                let actual = event_hash(&events);
                if expected != actual {
                    return Err(ReplayError::EventMismatch {
                        tick: self.timeline_tick,
                        expected,
                        actual,
                    });
                }
            }
            self.terminal_applied = true;
        }
        let actual = self.game.state_hash();
        if actual != self.replay.final_state.state_hash {
            return Err(ReplayError::FinalMismatch {
                expected: self.replay.final_state.state_hash,
                actual,
            });
        }
        let result = result_of(&self.game);
        if result != self.replay.final_state.result
            || self.game.snapshot().timers.simulation_ticks
                != self.replay.final_state.simulation_ticks
        {
            return Err(ReplayError::Invalid(
                "final result metadata does not match reconstructed simulation".into(),
            ));
        }
        Ok(ReplayVerification {
            final_state_hash: actual,
            timeline_ticks: self.timeline_tick,
            simulation_ticks: self.game.snapshot().timers.simulation_ticks,
            result,
        })
    }
}

fn apply_lifecycle(game: &mut Game, action: &LifecycleAction) -> StepResult {
    match action {
        LifecycleAction::StartWithSeed { seed } => game.start_with_seed(*seed),
        LifecycleAction::RestartWithSeed { seed } => game.restart_with_seed(*seed),
        LifecycleAction::ReturnToTitle => game.return_to_title(),
    }
}
fn result_of(game: &Game) -> ReplayResult {
    ReplayResult {
        phase: game.phase,
        score: game.score,
        lines: game.lines,
        level: game.level,
    }
}

fn event_hash(events: &[GameEvent]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    let mut put = |value: u64| {
        h ^= value;
        h = h.wrapping_mul(0x100000001b3);
    };
    put(events.len() as u64);
    for event in events {
        match event {
            GameEvent::Started => put(1),
            GameEvent::Restarted { seed } => {
                put(2);
                put(*seed);
            }
            GameEvent::Paused => put(3),
            GameEvent::Resumed => put(4),
            GameEvent::ReadyStarted => put(5),
            GameEvent::CountdownStarted { resume } => {
                put(6);
                put(*resume as u64);
            }
            GameEvent::ReturnedToTitle => put(7),
            GameEvent::Spawned { kind } => {
                put(8);
                put(kind.id() as u64);
            }
            GameEvent::Moved { dx, dy } => {
                put(9);
                put(*dx as u8 as u64);
                put(*dy as u8 as u64);
            }
            GameEvent::Rotated { rotation, kick } => {
                put(10);
                put(*rotation as u64);
                put(kick.0 as u8 as u64);
                put(kick.1 as u8 as u64);
            }
            GameEvent::Held {
                outgoing,
                incoming,
                hold,
            } => {
                put(11);
                put(outgoing.id() as u64);
                put(incoming.id() as u64);
                put(hold.map_or(255, |piece| piece.id()) as u64);
            }
            GameEvent::HardDropped { distance } => {
                put(12);
                put(*distance as u64);
            }
            GameEvent::Locked { kind } => {
                put(13);
                put(kind.id() as u64);
            }
            GameEvent::LinesCleared {
                rows,
                clear,
                spin,
                back_to_back,
            } => {
                put(14);
                put(rows.len() as u64);
                for row in rows {
                    put(*row as u64);
                }
                put(*clear as u64);
                put(spin.map_or(255, |value| value as u8) as u64);
                put(*back_to_back as u64);
            }
            GameEvent::ScoreAwarded { points, total } => {
                put(15);
                put(*points as u64);
                put(*total);
            }
            GameEvent::LevelChanged { from, to } => {
                put(16);
                put(*from as u64);
                put(*to as u64);
            }
            GameEvent::Combo { count } => {
                put(17);
                put(*count as u64);
            }
            GameEvent::BackToBack { active } => {
                put(18);
                put(*active as u64);
            }
            GameEvent::GameOver => put(19),
            GameEvent::Completed => put(20),
        }
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_reconstructs_lifecycle_and_commands() {
        let mut recorder =
            ReplayRecorder::with_checkpoint_interval(7, GameConfig::default(), 10).unwrap();
        recorder.start_with_seed(42);
        for tick in 0..4_600 {
            let commands = match tick {
                1_000 => vec![Command::LeftPress, Command::RotateCw],
                1_001 => vec![Command::LeftRelease],
                4_501 => vec![Command::Pause],
                4_502 => vec![Command::Resume],
                _ => vec![],
            };
            recorder.step(&commands).unwrap();
        }
        let replay = Replay::from_json(&recorder.replay().to_json_pretty().unwrap()).unwrap();
        let verified = replay.verify().unwrap();
        assert_eq!(verified.final_state_hash, replay.final_state.state_hash);
        assert!(!replay.checkpoints.is_empty());
    }
    #[test]
    fn rejects_corruption_and_invalid_config() {
        let recorder = ReplayRecorder::new(1, GameConfig::default()).unwrap();
        let mut replay = recorder.replay();
        replay.tick_hz = 60;
        assert!(matches!(replay.verify(), Err(ReplayError::Invalid(_))));

        let mut config = GameConfig::default();
        config.rules.input.arr_ticks = 0;
        assert!(ReplayRecorder::new(1, config).is_err());
    }
    #[test]
    fn restart_keeps_the_replay_timeline_monotonic() {
        let mut recorder =
            ReplayRecorder::with_checkpoint_interval(1, GameConfig::default(), 1).unwrap();
        recorder.start_with_seed(1);
        recorder.step(&[]).unwrap();
        recorder.restart_with_seed(99);
        recorder.step(&[Command::RotateCw]).unwrap();
        let replay = recorder.replay();
        assert_eq!(replay.final_state.timeline_ticks, 2);
        assert_eq!(replay.final_state.simulation_ticks, 1);
        replay.verify().unwrap();
    }

    #[test]
    fn persistent_fixture_verifies() {
        Replay::from_json(include_str!("../tests/fixtures/ready-v1.tdr.json"))
            .unwrap()
            .verify()
            .unwrap();
    }
}
