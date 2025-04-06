#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use crate::cli::Cli;
use crate::gui::Gui;
use analysis::{
    Analysis, AnalyzerEvent, AnalyzerState, DemoInfo, frame_to_events,
    use_clan_match_detection_updates, use_kill_streak_updates, use_player_updates,
    use_rounds_updates, use_scoreboard_updates, use_team_score_updates, use_timing_updates,
    use_weapon_breakdown_updates,
};
use dem::open_demo;
use filetime::FileTime;
use std::env::args;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

mod cli;
mod gui;

fn main() {
    let args = args().collect::<Vec<_>>();

    if args.contains(&"--cli".to_string()) {
        Cli::run(args)
    } else {
        Gui::run()
    }
}

pub struct FileInfo {
    pub created_at: SystemTime,
    pub name: String,
    pub path: String,
}

fn run_analyzer(demo_path: &PathBuf) -> (FileInfo, Analysis) {
    let demo = open_demo(demo_path).expect("Could not parse the demo");

    let events = vec![AnalyzerEvent::Initialization].into_iter().chain(
        demo.directory
            .entries
            .iter()
            .flat_map(|entry| entry.frames.iter().flat_map(frame_to_events))
            .chain(vec![AnalyzerEvent::Finalization]),
    );

    let analysis = events.fold(AnalyzerState::default(), |mut state, ref event| {
        use_timing_updates(&mut state, event);
        use_player_updates(&mut state, event);
        use_scoreboard_updates(&mut state, event);
        use_kill_streak_updates(&mut state, event);
        use_weapon_breakdown_updates(&mut state, event);
        use_team_score_updates(&mut state, event);
        use_rounds_updates(&mut state, event);
        use_clan_match_detection_updates(Duration::from_secs(10), &mut state, event);

        state
    });

    let created_at = fs::metadata(demo_path)
        .map_err(|_| ())
        .and_then(|metadata| FileTime::from_creation_time(&metadata).ok_or(()))
        .map(|file_time| {
            let creation_offset =
                Duration::new(file_time.unix_seconds() as u64, file_time.nanoseconds());

            SystemTime::UNIX_EPOCH + creation_offset
        })
        .unwrap();

    let file_info = FileInfo {
        created_at,
        name: demo_path
            .file_name()
            .and_then(|s| s.to_str())
            .map(String::from)
            .unwrap(),

        path: demo_path.to_str().map(String::from).unwrap(),
    };

    let map_name = demo
        .header
        .map_name
        .to_str()
        .map(|s| s.trim_end_matches('\x00'))
        .unwrap()
        .to_string();

    let demo_info = DemoInfo {
        demo_protocol: demo.header.demo_protocol,
        map_name,
        network_protocol: demo.header.network_protocol,
    };

    let analysis = Analysis {
        state: analysis,
        demo_info,
    };

    (file_info, analysis)
}
