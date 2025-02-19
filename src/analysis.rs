use crate::dod::{Class, Message, Team, Weapon};
use dem::types::EngineMessage;
use std::collections::HashMap;
use std::str::from_utf8;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct GameTime {
    pub origin: Instant,
    pub offset: Duration,
}

impl Default for GameTime {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
            offset: Duration::from_secs(0),
        }
    }
}

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
    pub kill_streaks: Vec<KillStreak>,
}

#[derive(Debug)]
pub struct KillStreak {
    pub kills: Vec<(GameTime, Weapon)>,
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
            kill_streaks: vec![KillStreak { kills: vec![] }],
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

pub enum AnalyzerEvent<'a> {
    EngineMessage(&'a EngineMessage),
    UserMessage(Message),
    SetTime(f32),
}

#[derive(Debug, Default)]
pub struct AnalyzerState {
    pub current_time: GameTime,
    pub players: Vec<Player>,
}

impl AnalyzerState {
    fn find_player_by_client_index(&self, client_index: u8) -> Option<&Player> {
        self.players.iter().find(|player| {
            if let ConnectionStatus::Connected { client_id } = player.connection_status {
                client_id == client_index
            } else {
                false
            }
        })
    }

    fn find_player_by_client_index_mut(&mut self, client_index: u8) -> Option<&mut Player> {
        self.players.iter_mut().find(|player| {
            if let ConnectionStatus::Connected { client_id } = player.connection_status {
                client_id == client_index
            } else {
                false
            }
        })
    }

    fn find_player_by_global_id_mut(&mut self, global_id: &PlayerGlobalId) -> Option<&mut Player> {
        self.players
            .iter_mut()
            .find(|player| player.player_global_id == *global_id)
    }
}

pub fn use_timing_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    if let AnalyzerEvent::SetTime(frame_time_offset) = event {
        let mut next_time = state.current_time.clone();

        next_time.offset = Duration::from_secs_f32(*frame_time_offset);

        state.current_time = next_time;
    }
}

pub fn use_player_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    let svc_update_user_info = match event {
        AnalyzerEvent::EngineMessage(EngineMessage::SvcUpdateUserInfo(msg)) => Some(msg),
        _ => None,
    };

    if let Some(svc_update_user_info) = svc_update_user_info {
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
            state.find_player_by_client_index_mut(svc_update_user_info.index);

        if let Some(current_player) = existing_player_in_slot {
            // A new player has taken over the slot from an old player
            if current_player.player_global_id != player_global_id {
                // Indicate that the old player is disconnected now
                current_player.connection_status = ConnectionStatus::Disconnected;

                // Try to find an existing record of the player
                if let Some(player) = state.find_player_by_global_id_mut(&player_global_id) {
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

                    state.players.push(new_player);
                }
            } else {
                current_player.name = player_name;
            }
        } else {
            let mut new_player = Player::new(player_global_id);

            new_player
                .with_connection_status(ConnectionStatus::Connected {
                    client_id: svc_update_user_info.index,
                })
                .with_name("");

            state.players.push(new_player);
        }
    }
}

pub fn use_scoreboard_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    match event {
        AnalyzerEvent::UserMessage(Message::PClass(p_class)) => {
            let player = state.find_player_by_client_index_mut(p_class.client_index - 1);

            if let Some(player) = player {
                player.class = Some(p_class.class.clone());
            };
        }

        AnalyzerEvent::UserMessage(Message::PTeam(p_team)) => {
            let player = state.find_player_by_client_index_mut(p_team.client_index - 1);

            if let Some(player) = player {
                player.team = Some(p_team.team.clone());
            };
        }

        AnalyzerEvent::UserMessage(Message::ScoreShort(score_short)) => {
            let player = state.find_player_by_client_index_mut(score_short.client_index - 1);

            if let Some(player) = player {
                player.stats = (
                    score_short.score as i32,
                    score_short.kills as i32,
                    score_short.deaths as i32,
                );
            }
        }

        _ => {}
    };
}

pub fn use_kill_streak_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    if let AnalyzerEvent::UserMessage(Message::DeathMsg(death_msg)) = event {
        let current_time = state.current_time.clone();

        let killer = state.find_player_by_client_index(death_msg.killer_client_index - 1);
        let victim = state.find_player_by_client_index(death_msg.victim_client_index - 1);

        let is_teamkill = match (killer, victim) {
            (Some(fst), Some(snd)) => fst.team == snd.team,
            _ => false,
        };

        if is_teamkill {
            return;
        }

        let killer = state.find_player_by_client_index_mut(death_msg.killer_client_index - 1);

        if let Some(killer) = killer {
            if let Some(killer_current_streak) = killer.kill_streaks.iter_mut().last() {
                killer_current_streak
                    .kills
                    .push((current_time, death_msg.weapon.clone()));
            }
        }

        let victim = state.find_player_by_client_index_mut(death_msg.victim_client_index - 1);

        if let Some(victim) = victim {
            // End the current streak by adding a new record
            victim.kill_streaks.push(KillStreak { kills: vec![] });
        }
    }
}
