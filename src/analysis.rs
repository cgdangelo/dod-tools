use crate::dod::{Class, Message, Team, Weapon};
use dem::types::EngineMessage;
use std::collections::HashMap;
use std::str::from_utf8;
use std::time::{Duration, Instant};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct GameTime {
    #[allow(dead_code)]
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
    pub weapon_breakdown: HashMap<Weapon, i32>,
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
            weapon_breakdown: HashMap::new(),
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
    TimeUpdate(f32),
}

#[derive(Debug, Default)]
pub struct AnalyzerState {
    pub current_time: GameTime,
    pub players: Vec<Player>,
    pub team_scores: HashMap<Team, i32>,
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

    fn find_player_by_global_id(&self, global_id: &PlayerGlobalId) -> Option<&Player> {
        self.players
            .iter()
            .find(|player| player.player_global_id == *global_id)
    }

    fn find_player_by_global_id_mut(&mut self, global_id: &PlayerGlobalId) -> Option<&mut Player> {
        self.players
            .iter_mut()
            .find(|player| player.player_global_id == *global_id)
    }
}

pub fn use_timing_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    if let AnalyzerEvent::TimeUpdate(frame_time_offset) = event {
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
            .map(|s| s.trim_matches(['\0', '\\']).split("\\").collect::<Vec<_>>())
            .unwrap_or_default()
            .chunks_exact(2)
            .fold(HashMap::new(), |mut map, chunk| {
                if let [key, value] = chunk {
                    map.insert(*key, *value);
                }

                map
            });

        // Missing fields indicates that the user has disconnected, so we only update their
        // connection status and preserve the last known details.
        if fields.is_empty() {
            let player = state.find_player_by_client_index_mut(svc_update_user_info.index);

            if let Some(disconnected_player) = player {
                disconnected_player.connection_status = ConnectionStatus::Disconnected;
                return;
            }
        }

        // HLTV clients have this field set to 1. We can skip them because whatever slot it occupies
        // will never be referenced by game events, unless someone else takes that slot.
        if let Some(&"1") = fields.get("*hltv") {
            return;
        }

        let player_global_id = fields
            .get("*sid")
            .map(|s| s.to_string())
            .or_else(|| {
                let mut uuid_seed = vec![];

                let server_id_bytes = svc_update_user_info.id.to_le_bytes();

                uuid_seed.extend_from_slice(&server_id_bytes);
                uuid_seed.extend_from_slice(&server_id_bytes);
                uuid_seed.extend_from_slice(&server_id_bytes);
                uuid_seed.extend_from_slice(&server_id_bytes);

                let uuid = Uuid::from_slice(&uuid_seed)
                    .unwrap_or(Uuid::new_v4())
                    .simple();

                Some(uuid.to_string())
            })
            .map(PlayerGlobalId)
            .unwrap_or_else(|| {
                panic!(
                    "Could not resolve a global id for player {} in slot {}",
                    svc_update_user_info.id, svc_update_user_info.index
                )
            });

        let player_name = fields
            .get("name")
            .map(|x| x.to_string())
            .unwrap_or(format!("Player {}", svc_update_user_info.id));

        // Make sure a record of this player exists first
        if state.find_player_by_global_id(&player_global_id).is_none() {
            let insert_id = player_global_id.clone();
            let new_player = Player::new(insert_id);

            state.players.push(new_player);
        };

        // Flush any existing player from this slot
        if let Some(player_in_slot) =
            state.find_player_by_client_index_mut(svc_update_user_info.index)
        {
            player_in_slot.with_connection_status(ConnectionStatus::Disconnected);
        }

        // Find the player from the message, and assign it to the slot
        state
            .find_player_by_global_id_mut(&player_global_id)
            .unwrap()
            .with_connection_status(ConnectionStatus::Connected {
                client_id: svc_update_user_info.index,
            })
            .with_name(player_name);
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

pub fn use_weapon_breakdown_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    if let AnalyzerEvent::UserMessage(Message::DeathMsg(death_msg)) = event {
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
            let kills_by_weapon = killer
                .weapon_breakdown
                .entry(death_msg.weapon.clone())
                .or_insert(0);

            *kills_by_weapon += 1;
        }
    }
}

pub fn use_team_score_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    if let AnalyzerEvent::UserMessage(Message::TeamScore(team_score)) = event {
        let team_entry = state
            .team_scores
            .entry(team_score.team.clone())
            .or_insert(0);

        *team_entry = team_score.score as i32;
    }
}
