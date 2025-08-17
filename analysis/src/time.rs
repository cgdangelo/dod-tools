use crate::{AnalyzerEvent, AnalyzerState};
use dem::types::EngineMessage;
use std::{ops::Sub, time::Duration};

/// A moment in time when something happened in game.
#[derive(Clone, Debug, Default)]
pub struct GameTime {
    /// Timestamp that represents the amount opf time relative to 0 (recording start).
    pub real_offset: Duration,

    /// Timestamp that represents the value shown in the `viewdemo` window.
    pub viewdemo_offset: Duration,
}

impl Sub<GameTime> for GameTime {
    type Output = Duration;

    fn sub(self, rhs: GameTime) -> Self::Output {
        self.viewdemo_offset - rhs.viewdemo_offset
    }
}

impl<'a> Sub<&'a GameTime> for &GameTime {
    type Output = Duration;

    fn sub(self, rhs: &'a GameTime) -> Self::Output {
        self.viewdemo_offset - rhs.viewdemo_offset
    }
}

pub fn use_timing_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    if let AnalyzerEvent::EngineMessage(EngineMessage::SvcTime(svc_time)) = event {
        let offset = Duration::from_secs_f32(svc_time.time);

        if offset > state.current_time.real_offset
            || state.current_time.real_offset == Duration::ZERO
        {
            state.current_time.viewdemo_offset = offset;
        }
    } else if let AnalyzerEvent::RealTimeChange(value) = event {
        let offset = Duration::from_secs_f32(*value);

        if offset > state.current_time.real_offset
            || state.current_time.real_offset == Duration::ZERO
        {
            state.current_time.real_offset = offset;
        }
    }
}
