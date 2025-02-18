use crate::dod::{Class, Message, Team};
use dem::{
    open_demo,
    types::{EngineMessage, FrameData, MessageData, NetMessage, SvcUpdateUserInfo},
};
use std::cmp::{Ordering, PartialEq};
use std::collections::HashMap;
use std::convert::identity;
use std::env::args;
use std::fmt::{Debug, Write};
use std::path::PathBuf;
use std::str::from_utf8;
use tabled::{builder::Builder, settings::Style};
use uuid::Uuid;

mod dod;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct PlayerGlobalId(String);

/// Represents whether a [Player] is connected to the server.
#[derive(Debug)]
enum ConnectionStatus {
    /// Player is currently connected to the server.
    Connected {
        /// Identifier assigned by the server that represents the [Player]'s connection.
        client_id: u8,
    },

    Disconnected,
}

#[derive(Debug)]
struct Player {
    connection_status: ConnectionStatus,
    name: String,
    player_global_id: PlayerGlobalId,
    team: Option<Team>,
    class: Option<Class>,
    stats: (i32, i32, i32),
}

#[derive(Default, Debug)]
struct AnalyzerState {
    players: Vec<Player>,
}

impl AnalyzerState {
    fn find_player_by_client_index_mut(&mut self, client_index: u8) -> Option<&mut Player> {
        self.players
            .iter_mut()
            .find(|player| match player.connection_status {
                ConnectionStatus::Connected { client_id } => client_id == client_index,
                _ => false,
            })
    }

    fn find_player_by_global_id_mut(&mut self, global_id: &PlayerGlobalId) -> Option<&mut Player> {
        self.players
            .iter_mut()
            .find(|player| player.player_global_id == *global_id)
    }

    fn mutate_from_analyzer_event(&mut self, event: AnalyzerEvent) {
        match event {
            AnalyzerEvent::EngineMessage(engine_msg) => match engine_msg {
                EngineMessage::SvcUpdateUserInfo(svc_update_user_info) => {
                    self.use_player_updates(svc_update_user_info);
                }

                _ => {}
            },

            AnalyzerEvent::UserMessage(user_msg) => match user_msg {
                Message::PClass(p_class) => {
                    let player = self.find_player_by_client_index_mut(p_class.client_index - 1);

                    if let Some(player) = player {
                        player.class = Some(p_class.class);
                    };
                }

                Message::PTeam(p_team) => {
                    let player = self.find_player_by_client_index_mut(p_team.client_index - 1);

                    if let Some(player) = player {
                        player.team = Some(p_team.team);
                    };
                }

                Message::ScoreShort(score_short) => {
                    let player = self.find_player_by_client_index_mut(score_short.client_index - 1);

                    if let Some(player) = player {
                        player.stats = (
                            score_short.score as i32,
                            score_short.kills as i32,
                            score_short.deaths as i32,
                        );
                    }
                }

                _ => {}
            },
        };
    }

    fn use_player_updates(&mut self, svc_update_user_info: &SvcUpdateUserInfo) {
        let fields = from_utf8(&svc_update_user_info.user_info)
            .map(|s| s.trim_matches(['\0', '\\']).split("\\").collect())
            .unwrap_or(vec![])
            .chunks_exact(2)
            .fold(HashMap::new(), |mut map, chunk| {
                if let [key, value] = chunk {
                    map.insert(*key, *value);
                }

                map
            });

        let player_global_id = fields
            .get("*sid")
            .map(|x| PlayerGlobalId(x.to_string()))
            .unwrap_or(PlayerGlobalId(Uuid::new_v4().to_string()));

        let player_name = fields
            .get("name")
            .map(|x| x.to_string())
            .unwrap_or(format!("Player {}", svc_update_user_info.index));

        let existing_player_in_slot =
            self.find_player_by_client_index_mut(svc_update_user_info.index);

        match existing_player_in_slot {
            Some(current_player) => {
                // A new player has taken over the slot from an old player
                if current_player.player_global_id != player_global_id {
                    // Indicate that the old player is disconnected now
                    current_player.connection_status = ConnectionStatus::Disconnected;

                    // Try to find an existing record of the player
                    if let Some(player) = self.find_player_by_global_id_mut(&player_global_id) {
                        player.name = player_name;
                        player.connection_status = ConnectionStatus::Connected {
                            client_id: svc_update_user_info.index,
                        };
                    } else {
                        panic!("Could not find player {}", &player_global_id.0);
                    }
                } else {
                    current_player.name = player_name;
                }
            }

            None => {
                self.players.push(Player {
                    connection_status: ConnectionStatus::Connected {
                        client_id: svc_update_user_info.index,
                    },
                    name: player_name,
                    player_global_id,
                    team: None,
                    class: None,
                    stats: (0, 0, 0),
                });
            }
        }
    }
}

enum AnalyzerEvent<'a> {
    EngineMessage(&'a EngineMessage),
    UserMessage(Message),
}

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
