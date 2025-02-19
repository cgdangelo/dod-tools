use crate::analysis::{
    use_kill_streak_updates, use_player_updates, use_scoreboard_updates, use_timing_updates,
    use_weapon_breakdown_updates, AnalyzerEvent, AnalyzerState,
};
use crate::dod::Message;
use crate::reporting::Report;
use dem::{
    open_demo,
    types::{FrameData, MessageData, NetMessage},
};
use std::convert::identity;
use std::env::args;
use std::path::PathBuf;

mod analysis;
#[allow(dead_code)]
mod dod;
mod reporting;

fn main() {
    let os_args = args().collect::<Vec<_>>();

    for path_str in &os_args[1..] {
        run_analyzer(path_str);
    }
}

fn run_analyzer(path_str: &str) {
    let demo_path = PathBuf::from(path_str);
    let demo = open_demo(&demo_path).unwrap();

    let analysis = demo
        .directory
        .entries
        .iter()
        .flat_map(|entry| &entry.frames)
        .filter_map(|frame| match &frame.frame_data {
            FrameData::NetworkMessage(frame_data) => {
                let messages = match &frame_data.1.messages {
                    MessageData::Parsed(msgs) => Some(msgs),
                    _ => None,
                }?;

                let events = messages.iter().fold(vec![], |mut acc, net_msg| {
                    match net_msg {
                        NetMessage::UserMessage(user_msg) => {
                            if let Ok(dod_msg) = Message::try_from(user_msg) {
                                acc.push(AnalyzerEvent::UserMessage(dod_msg));
                            }
                        }
                        NetMessage::EngineMessage(engine_msg) => {
                            acc.push(AnalyzerEvent::EngineMessage(engine_msg));
                        }
                    }

                    acc
                });

                Some(events)
            }

            _ => Some(vec![AnalyzerEvent::SetTime(frame.time)]),
        })
        .flat_map(identity)
        .fold(AnalyzerState::default(), |mut state, event| {
            use_timing_updates(&mut state, &event);
            use_player_updates(&mut state, &event);
            use_scoreboard_updates(&mut state, &event);
            use_kill_streak_updates(&mut state, &event);
            use_weapon_breakdown_updates(&mut state, &event);

            state
        });

    let reporter = Report {
        file_path: &demo_path,
        demo: &demo,
        analysis: &analysis,
    };

    println!("{}", reporter);
}
