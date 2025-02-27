use crate::dod::{Class, Message, RoundState, Team, Weapon};
use dem::types::{EngineMessage, Frame, FrameData, MessageData, NetMessage};
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
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

impl Display for PlayerGlobalId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

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
    pub weapon_breakdown: HashMap<Weapon, (u32, u32)>,
}

#[derive(Debug)]
pub struct KillStreak {
    pub kills: Vec<(GameTime, Weapon)>,
}

impl Hash for Player {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.player_global_id.hash(state)
    }
}

impl PartialEq for Player {
    fn eq(&self, other: &Self) -> bool {
        self.player_global_id == other.player_global_id
    }
}

impl Eq for Player {}

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

#[derive(Debug)]
pub enum Round {
    Active {
        allies_kills: u32,
        axis_kills: u32,
        start_time: GameTime,
    },

    Completed {
        start_time: GameTime,
        end_time: GameTime,
        winner_stats: Option<(Team, u32)>,
    },
}

pub enum AnalyzerEvent<'a> {
    Initialization,
    EngineMessage(&'a EngineMessage),
    UserMessage(Message),
    TimeUpdate(f32),
    Finalization,
}

#[derive(Debug, Default)]
pub struct AnalyzerState {
    pub current_time: GameTime,
    pub players: Vec<Player>,
    pub rounds: Vec<Round>,
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

pub fn frame_to_events(frame: &Frame) -> Vec<AnalyzerEvent> {
    let mut events: Vec<AnalyzerEvent> = vec![];

    if let FrameData::NetworkMessage(frame_data) = &frame.frame_data {
        if let MessageData::Parsed(msgs) = &frame_data.1.messages {
            for net_msg in msgs {
                match net_msg {
                    NetMessage::UserMessage(user_msg) => {
                        if let Ok(dod_msg) = Message::try_from(user_msg) {
                            events.push(AnalyzerEvent::UserMessage(dod_msg));
                        }
                    }
                    NetMessage::EngineMessage(engine_msg) => {
                        events.push(AnalyzerEvent::EngineMessage(engine_msg));
                    }
                }
            }
        }
    } else {
        events.push(AnalyzerEvent::TimeUpdate(frame.time));
    }

    events
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

        let killer = state.find_player_by_client_index_mut(death_msg.killer_client_index - 1);

        if let Some(killer) = killer {
            let (kills, teamkills) = killer
                .weapon_breakdown
                .entry(death_msg.weapon.clone())
                .or_insert((0, 0));

            if is_teamkill {
                *teamkills += 1;
            } else {
                *kills += 1;
            }
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

pub fn use_rounds_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    if let AnalyzerEvent::Initialization = event {
        // Make sure demo starts with an active round if recording started after match start
        state.rounds.push(Round::Active {
            allies_kills: 0,
            axis_kills: 0,
            start_time: state.current_time.clone(),
        });
    } else if let AnalyzerEvent::UserMessage(Message::DeathMsg(death_msg)) = event {
        let kill_info = match (
            state.find_player_by_client_index(death_msg.killer_client_index),
            state.find_player_by_client_index(death_msg.victim_client_index),
        ) {
            (Some(killer), Some(victim)) => Some((
                killer.team.clone(),
                killer.team.is_some() && killer.team == victim.team,
            )),

            _ => None,
        };

        if let (
            Some(Round::Active {
                allies_kills,
                axis_kills,
                ..
            }),
            Some((team, is_teamkill)),
        ) = (state.rounds.last_mut(), kill_info)
        {
            if is_teamkill {
                return;
            }

            if let Some(Team::Allies) = team {
                *allies_kills += 1;
            } else if let Some(Team::Axis) = team {
                *axis_kills += 1;
            }
        };
    } else if let AnalyzerEvent::UserMessage(Message::RoundState(round_state)) = event {
        match round_state {
            RoundState::Reset => {
                // Infer first reset/normal as start of match
                if state.rounds.len() == 1 {
                    if let Some(Round::Active { .. }) = state.rounds.first() {
                        state.rounds.clear();
                    }
                }

                state.rounds.push(Round::Active {
                    allies_kills: 0,
                    axis_kills: 0,
                    start_time: state.current_time.clone(),
                });
            }

            RoundState::AlliesWin | RoundState::AxisWin => {
                let current_round = state.rounds.pop();

                if let Some(Round::Active {
                    allies_kills,
                    axis_kills,
                    start_time,
                }) = current_round
                {
                    let (winner, winner_kills) = if matches!(round_state, RoundState::AlliesWin) {
                        (Team::Allies, allies_kills)
                    } else {
                        (Team::Axis, axis_kills)
                    };

                    state.rounds.push(Round::Completed {
                        start_time,
                        end_time: state.current_time.clone(),
                        winner_stats: Some((winner, winner_kills)),
                    });
                }
            }

            _ => {}
        }
    } else if let AnalyzerEvent::Finalization = event {
        let current_round = state.rounds.pop();

        if let Some(Round::Active { start_time, .. }) = current_round {
            state.rounds.push(Round::Completed {
                start_time,
                end_time: state.current_time.clone(),
                winner_stats: None,
            });
        }
    }
}
