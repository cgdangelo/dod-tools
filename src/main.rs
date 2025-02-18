use crate::dod::{Class, Message, Team};
use dem::open_demo;
use dem::types::{EngineMessage, FrameData, MessageData, NetMessage};
use std::collections::{HashMap, HashSet};
use std::convert::identity;
use std::env::args;
use std::str::from_utf8;
use uuid::Uuid;

mod dod;

#[derive(Debug, Eq, Hash, PartialEq)]
struct PlayerGlobalId(String);

/// A participant in the game.
#[derive(Debug, Eq, Hash, PartialEq)]
struct Player {
    global_id: PlayerGlobalId,
}

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
struct PlayerState {
    connection_status: ConnectionStatus,
    name: String,
    team: Team,
    class: Option<Class>,
}

#[derive(Default, Debug)]
struct AnalyzerState<'a> {
    players: HashSet<Player>,
    player_states: HashMap<&'a Player, PlayerState>,
}

enum AnalyzerEvent<'a> {
    EngineMessage(&'a EngineMessage),
    UserMessage(Message),
}

fn main() {
    let args = args().collect::<Vec<_>>();
    let demo_path = args.get(1).unwrap();

    let demo = open_demo(demo_path).unwrap();

    let analysis = demo
        .directory
        .entries
        .iter()
        .flat_map(|entry| &entry.frames)
        .filter_map(|frame| match &frame.frame_data {
            FrameData::NetworkMessage(frame_data) => Some(frame_data),
            _ => None,
        })
        .filter_map(|frame_data| {
            if let MessageData::Parsed(messages) = &frame_data.1.messages {
                Some(messages)
            } else {
                None
            }
        })
        .flat_map(identity)
        .filter_map(|net_msg| match net_msg {
            NetMessage::UserMessage(user_msg) => {
                let dod_msg = Message::try_from(user_msg).ok()?;

                Some(AnalyzerEvent::UserMessage(dod_msg))
            }
            NetMessage::EngineMessage(engine_msg) => Some(AnalyzerEvent::EngineMessage(engine_msg)),
        })
        .fold(AnalyzerState::default(), |mut acc, event| {
            match event {
                AnalyzerEvent::EngineMessage(engine_msg) => match engine_msg {
                    EngineMessage::SvcUpdateUserInfo(svc_update_user_info) => {
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

                        let global_id = fields
                            .get("*sid")
                            .map(|x| PlayerGlobalId(x.to_string()))
                            .unwrap_or(PlayerGlobalId(Uuid::new_v4().to_string()));

                        let player = Player { global_id };

                        acc.players.insert(player);
                    }
                    _ => {}
                },

                AnalyzerEvent::UserMessage(user_msg) => match user_msg {
                    _ => {}
                },
            };

            acc
        });

    println!("{:?}", analysis);

    // for entry in &demo.directory.entries {
    //     for frame in &entry.frames {
    //         match &frame.frame_data {
    //             FrameData::NetworkMessage(frame_data) => {
    //                 if let MessageData::Parsed(messages) = &frame_data.1.messages {
    //                     for message in messages {
    //                         if let NetMessage::UserMessage(user_msg) = message {
    //                             if let Ok(dod_msg) = crate::dod::Message::try_from(user_msg) {
    //                                 println!("{:?}", dod_msg)
    //                             } else {
    //                                 println!(
    //                                     "{:?} {:?}",
    //                                     unsafe {
    //                                         String::from_utf8_unchecked(user_msg.name.clone())
    //                                     }
    //                                     .trim_end_matches('\x00'),
    //                                     user_msg.data
    //                                 );
    //                             }
    //                         }
    //                     }
    //                 }
    //             }
    //             _ => {}
    //         }
    //     }
    // }
}
