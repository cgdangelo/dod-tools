use crate::dod::{Class, Message, Team};
use dem::types::{EngineMessage, SvcUpdateUserInfo};
use std::collections::HashMap;
use std::str::from_utf8;
use uuid::Uuid;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PlayerGlobalId(pub String);

/// Represents whether a [Player] is connected to the server.
#[derive(Debug)]
pub enum ConnectionStatus {
    /// Player is currently connected to the server.
    Connected {
        /// Identifier assigned by the server that represents the [Player]'s connection.
        client_id: u8,
    },

    Disconnected,
}

#[derive(Debug)]
pub struct Player {
    pub connection_status: ConnectionStatus,
    pub name: String,
    pub player_global_id: PlayerGlobalId,
    pub team: Option<Team>,
    pub class: Option<Class>,
    pub stats: (i32, i32, i32),
}

impl Player {
    fn new(global_id: PlayerGlobalId) -> Self {
        Self {
            connection_status: ConnectionStatus::Disconnected,
            name: String::new(),
            player_global_id: global_id,
            team: None,
            class: None,
            stats: (0, 0, 0),
        }
    }

    fn with_connection_status(&mut self, connection_status: ConnectionStatus) -> &mut Self {
        self.connection_status = connection_status;
        self
    }

    fn with_name(&mut self, name: impl ToString) -> &mut Self {
        self.name = name.to_string();
        self
    }
}

#[derive(Default, Debug)]
pub struct AnalyzerState {
    pub players: Vec<Player>,
}

impl AnalyzerState {
    pub fn find_player_by_client_index_mut(&mut self, client_index: u8) -> Option<&mut Player> {
        self.players
            .iter_mut()
            .find(|player| match player.connection_status {
                ConnectionStatus::Connected { client_id } => client_id == client_index,
                _ => false,
            })
    }

    pub fn find_player_by_global_id_mut(&mut self, global_id: &PlayerGlobalId) -> Option<&mut Player> {
        self.players
            .iter_mut()
            .find(|player| player.player_global_id == *global_id)
    }

    pub fn mutate_from_analyzer_event(&mut self, event: AnalyzerEvent) {
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
            .map(|s| s.to_string())
            .or_else(|| {
                // CD key hash also happens to be 16 bytes, so we can use those to generate a UUID.
                let uuid = Uuid::from_slice(&svc_update_user_info.cd_key_hash)
                    .unwrap()
                    .simple();

                Some(uuid.to_string())
            })
            .map(PlayerGlobalId)
            .expect(
                format!(
                    "Could not resolve a global id for player {} in slot {}",
                    svc_update_user_info.id, svc_update_user_info.index
                )
                .as_str(),
            );

        let player_name = fields
            .get("name")
            .map(|x| x.to_string())
            .unwrap_or(format!("Player {}", svc_update_user_info.id));

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
                        let mut new_player = Player::new(player_global_id);

                        new_player
                            .with_connection_status(ConnectionStatus::Connected {
                                client_id: svc_update_user_info.index,
                            })
                            .with_name("");

                        self.players.push(new_player);
                    }
                } else {
                    current_player.name = player_name;
                }
            }

            None => {
                let mut new_player = Player::new(player_global_id);

                new_player
                    .with_connection_status(ConnectionStatus::Connected {
                        client_id: svc_update_user_info.index,
                    })
                    .with_name("");

                self.players.push(new_player);
            }
        }
    }
}

pub enum AnalyzerEvent<'a> {
    EngineMessage(&'a EngineMessage),
    UserMessage(Message),
}
