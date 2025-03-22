use crate::analysis::{
    frame_to_events, use_clan_match_detection_updates, use_kill_streak_updates, use_player_updates,
    use_rounds_updates, use_scoreboard_updates, use_team_score_updates, use_timing_updates,
    use_weapon_breakdown_updates, AnalyzerEvent, AnalyzerState,
};
use crate::gui::Gui;
use crate::reporting::{DemoInfo, FileInfo, Report};
use dem::open_demo;
use filetime::FileTime;
use std::env::args;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

mod analysis;
mod dod;
mod gui;
mod reporting;

fn main() {
    let args = args().collect::<Vec<_>>();

    if args.contains(&"--cli".to_string()) {
        run_cli(args)
    } else {
        run_gui()
    }
}

fn run_cli(args: Vec<String>) {
    for arg in &args[1..] {
        if arg == "--cli" {
            continue;
        }

        let demo_path = PathBuf::from(arg);
        let report = run_analyzer(&demo_path);

        println!("{}", report);
    }
}

#[tokio::main]
async fn run_gui() {
    eframe::run_native(
        env!("CARGO_PKG_NAME"),
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::<Gui>::default())),
    )
    .expect("Could not run the GUI");
}

fn run_analyzer(demo_path: &PathBuf) -> Report {
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

    let map_name = String::from_utf8(demo.header.map_name).unwrap();
    let map_name = map_name.trim_end_matches('\x00').to_string();
    let demo_info = DemoInfo {
        demo_protocol: demo.header.demo_protocol,
        map_name,
        network_protocol: demo.header.network_protocol,
    };

    Report {
        analysis,
        file_info,
        demo_info,
    }
}
