use crate::analysis::{
    frame_to_events, use_kill_streak_updates, use_player_updates, use_scoreboard_updates,
    use_team_score_updates, use_timing_updates, use_weapon_breakdown_updates, AnalyzerEvent,
    AnalyzerState,
};
use crate::reporting::{FileInfo, Report};
use dem::open_demo;
use filetime::FileTime;
use std::env::args;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

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
        .flat_map(|entry| {
            let mut events = entry
                .frames
                .iter()
                .flat_map(frame_to_events)
                .collect::<Vec<_>>();

            events.insert(0, AnalyzerEvent::Initialization);
            events.push(AnalyzerEvent::Finalization);
            events
        })
        .fold(AnalyzerState::default(), |mut state, ref event| {
            use_timing_updates(&mut state, event);
            use_player_updates(&mut state, event);
            use_scoreboard_updates(&mut state, event);
            use_kill_streak_updates(&mut state, event);
            use_weapon_breakdown_updates(&mut state, event);
            use_team_score_updates(&mut state, event);

            state
        });

    let created_at = fs::metadata(&demo_path)
        .map_err(|_| ())
        .and_then(|metadata| FileTime::from_creation_time(&metadata).ok_or(()))
        .map(|file_time| {
            let creation_offset =
                Duration::new(file_time.unix_seconds() as u64, file_time.nanoseconds());

            SystemTime::UNIX_EPOCH + creation_offset
        })
        .unwrap();

    let reporter = Report {
        file_info: FileInfo {
            created_at: &created_at,
            path: &demo_path,
        },
        demo: &demo,
        analysis: &analysis,
    };

    println!("{}", reporter);
}
