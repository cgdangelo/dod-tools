use crate::{AnalyzerEvent, AnalyzerState};
use dem::types::EngineMessage;
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
pub struct GameTime {
    #[allow(dead_code)]
    pub origin: Instant,
    pub offset: Duration,
}

impl Default for GameTime {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
            offset: Duration::from_secs(0),
        }
    }
}

pub fn use_timing_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    if let AnalyzerEvent::EngineMessage(EngineMessage::SvcTime(svc_time)) = event {
        if svc_time.time > 0. {
            let mut next_time = state.current_time.clone();

            next_time.offset = Duration::from_secs_f32(svc_time.time);

            state.current_time = next_time;
        }
    }
}
