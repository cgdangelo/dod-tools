use crate::analysis::{AnalyzerEvent, AnalyzerState};
use crate::dod::{Message, Team};
use dem::{
    open_demo,
    types::{FrameData, MessageData, NetMessage},
};
use std::cmp::Ordering;
use std::convert::identity;
use std::env::args;
use std::fmt::Write;
use std::path::PathBuf;
use tabled::{builder::Builder, settings::Style};

mod analysis;
#[allow(dead_code)]
mod dod;

fn main() {
    let args = args().collect::<Vec<_>>();
    let demo_path = args.get(1).map(PathBuf::from).unwrap();

    let demo = open_demo(&demo_path).unwrap();

    let analysis = demo
        .directory
        .entries
        .iter()
        .flat_map(|entry| &entry.frames)
        .filter_map(|frame| match &frame.frame_data {
            FrameData::NetworkMessage(frame_data) => Some(frame_data),
            _ => None,
        })
        .filter_map(|frame_data| match &frame_data.1.messages {
            MessageData::Parsed(msgs) => Some(msgs),
            _ => None,
        })
        .flat_map(identity)
        .filter_map(|net_msg| match net_msg {
            NetMessage::UserMessage(user_msg) => {
                let dod_msg = Message::try_from(user_msg).ok()?;

                Some(AnalyzerEvent::UserMessage(dod_msg))
            }
            NetMessage::EngineMessage(engine_msg) => Some(AnalyzerEvent::EngineMessage(engine_msg)),
        })
        .fold(AnalyzerState::default(), |mut state, event| {
            state.mutate_from_analyzer_event(event);
            state
        });

    let mut output = String::new();

    let mut table_builder = Builder::default();
    table_builder.push_record(["ID", "Name", "Team", "Class", "Score", "Kills", "Deaths"]);

    let mut table_data = Vec::from(analysis.players);
    table_data.sort_by(|left, right| match (&left.team, &right.team) {
        (Some(left_team), Some(right_team)) if left_team == right_team => {
            left.stats.0.cmp(&right.stats.0).reverse()
        }

        (Some(Team::Allies), _) => Ordering::Less,
        (Some(Team::Axis), Some(Team::Spectators)) => Ordering::Less,
        (Some(Team::Spectators) | None, _) => Ordering::Greater,

        _ => Ordering::Equal,
    });

    for player in &table_data {
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

    let mut table = table_builder.build();
    table.with(Style::markdown());

    let file_name = &demo_path.to_str().unwrap();

    writeln!(&mut output, "# Summary for: {}\n", file_name).unwrap();

    let map_name = String::from_utf8(demo.header.map_name).unwrap();
    let map_name = map_name.trim_end_matches('\x00');
    writeln!(&mut output, "- Map name: {}", map_name).unwrap();
    writeln!(&mut output).unwrap();

    writeln!(&mut output, "{}", table).unwrap();

    println!("{}", output);
}
