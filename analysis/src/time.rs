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

    #[deprecated(note = "Choose an offset based on desired timing model")]
    pub offset: Duration,
}

impl Sub<GameTime> for GameTime {
    type Output = Duration;

    fn sub(self, rhs: GameTime) -> Self::Output {
        self.viewdemo_offset - rhs.viewdemo_offset
    }
}

impl<'a, 'b> Sub<&'a GameTime> for &'b GameTime {
    type Output = Duration;
    fn sub(self, rhs: &'a GameTime) -> Self::Output {
        self.viewdemo_offset - rhs.viewdemo_offset
    }
}

pub fn use_timing_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    if let AnalyzerEvent::EngineMessage(EngineMessage::SvcTime(svc_time)) = event
        && svc_time.time > 0.
    {
        state.current_time.viewdemo_offset = Duration::from_secs_f32(svc_time.time);
        state.current_time.offset = state.current_time.viewdemo_offset;
    } else if let AnalyzerEvent::RealTimeChange(real_offset) = event
        && *real_offset > 0.
    {
        state.current_time.real_offset = Duration::from_secs_f32(*real_offset);
    }
}
