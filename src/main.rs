use crate::analysis::{
    use_kill_streak_updates, use_player_updates, use_scoreboard_updates, use_timing_updates,
    AnalyzerEvent, AnalyzerState,
};
use crate::dod::{Message, Team};
use dem::{
    open_demo,
    types::{FrameData, MessageData, NetMessage},
};
use humantime::format_duration;
use std::cmp::Ordering;
use std::convert::identity;
use std::env::args;
use std::fmt::Write;
use std::path::PathBuf;
use std::time::Duration;
use tabled::{builder::Builder, settings::Style};

mod analysis;
#[allow(dead_code)]
mod dod;

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

            state
        });

    // Players sorted by team then kills
    let mut ordered_players = Vec::from_iter(&analysis.players);

    ordered_players.sort_by(|left, right| match (&left.team, &right.team) {
        (Some(left_team), Some(right_team)) if left_team == right_team => {
            left.stats.0.cmp(&right.stats.0).reverse()
        }

        (Some(Team::Allies), _) => Ordering::Less,
        (Some(Team::Axis), Some(Team::Spectators)) => Ordering::Less,
        (Some(Team::Spectators) | None, _) => Ordering::Greater,

        _ => Ordering::Equal,
    });

    let mut output = String::new();

    // Header section
    writeln!(&mut output, "# Summary\n").unwrap();

    let file_name = &demo_path.to_str().unwrap();
    writeln!(&mut output, "- File name: `{}`", file_name).unwrap();
    let map_name = String::from_utf8(demo.header.map_name).unwrap();
    let map_name = map_name.trim_end_matches('\x00');
    writeln!(&mut output, "- Map name: {}\n", map_name).unwrap();

    // Player scoreboard section
    let mut table_builder = Builder::default();
    table_builder.push_record(["ID", "Name", "Team", "Class", "Score", "Kills", "Deaths"]);

    for player in &ordered_players {
        table_builder.push_record([
            player.player_global_id.0.to_string(),
            player.name.to_string(),
            match &player.team {
                None => "Unknown",
                Some(Team::Allies) => "Allies",
                Some(Team::Axis) => "Axis",
                Some(Team::Spectators) => "Spectators",
            }
            .to_string(),
            match &player.class {
                None => "Unknown".to_string(),
                Some(x) => format!("{:?}", x),
            },
            player.stats.0.to_string(),
            player.stats.1.to_string(),
            player.stats.2.to_string(),
        ]);
    }

    writeln!(&mut output, "## Scoreboard\n").unwrap();

    let mut table = table_builder.build();
    table.with(Style::markdown());

    writeln!(&mut output, "{}\n", table).unwrap();

    // Kill streaks section
    writeln!(&mut output, "## Kill Streaks\n").unwrap();

    for player in &ordered_players {
        writeln!(&mut output, "### {}\n", player.name).unwrap();

        let mut table_builder = Builder::default();
        table_builder.push_record([
            "Wave",
            "Total Kills",
            "Start Time",
            "Duration",
            "Weapons Used",
        ]);

        for (wave, kill_streak) in player.kill_streaks.iter().enumerate() {
            if let (Some((start_time, _)), Some((end_time, _))) =
                (kill_streak.kills.first(), kill_streak.kills.last())
            {
                let start_time_offset = Duration::new(start_time.offset.as_secs(), 0);
                let streak_duration =
                    Duration::new((end_time.offset - start_time.offset).as_secs(), 0);

                let weapons_used = kill_streak
                    .kills
                    .iter()
                    .map(|(_, weapon)| format!("{:?}", weapon))
                    .collect::<Vec<_>>()
                    .join(", ");

                table_builder.push_record([
                    (wave + 1).to_string(),
                    kill_streak.kills.len().to_string(),
                    format_duration(start_time_offset).to_string(),
                    format_duration(streak_duration).to_string(),
                    weapons_used,
                ]);
            }
        }

        let mut table = table_builder.build();
        table.with(Style::markdown());

        writeln!(&mut output, "{}\n", table).unwrap();
    }

    println!("{}", output);
}
