use crate::{AnalyzerEvent, AnalyzerState};
use dem::types::EngineMessage;
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct GameTime {
    pub real_offset: Duration,
    pub viewdemo_offset: Duration,

    #[deprecated(note = "Choose an offset based on desired timing model")]
    pub offset: Duration,
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
