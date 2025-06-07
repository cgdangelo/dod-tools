use dem::{
    open_demo_from_bytes,
    types::{EngineMessage, Frame, FrameData, MessageData, NetMessage},
};
use dod::{Class, Message, RoundState, Weapon};
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
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

impl PlayerGlobalId {
    pub fn as_steam_id(&self) -> Option<String> {
        // https://github.com/jpcy/coldemoplayer/blob/9c97ab128ac889739c1643baf0d5fdf884d8a65f/compLexity%20Demo%20Player/Common.cs#L364-L383
        let id64 = self.0.parse::<u64>().ok()?;
        let universe = 0; // Public

        let account_id = id64 - 76561197960265728;
        let server_id = if account_id % 2 == 0 { 0 } else { 1 };
        let account_id = (account_id - server_id) / 2;

        let steam_id = format!("STEAM_{}:{}:{}", universe, account_id & 1, account_id);

        Some(steam_id)
    }
}

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
    pub id: PlayerGlobalId,
    pub connection_status: ConnectionStatus,
    pub name: String,
    pub team: Option<Team>,
    pub class: Option<Class>,
    pub stats: (i32, i32, i32),
    pub kill_streaks: Vec<KillStreak>,
    pub weapon_breakdown: HashMap<Weapon, (u32, u32)>,
}

#[derive(Debug, Default)]
pub struct KillStreak {
    pub kills: Vec<(GameTime, Weapon)>,
}

impl Hash for Player {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state)
    }
}

impl PartialEq for Player {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Player {}

impl Player {
    fn new(id: PlayerGlobalId) -> Self {
        Self {
            connection_status: ConnectionStatus::Disconnected,
            name: String::new(),
            id,
            team: None,
            class: None,
            stats: (0, 0, 0),
            kill_streaks: vec![],
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

    fn with_team(&mut self, team: Option<Team>) -> &mut Self {
        self.team = team;
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
    Finalization,
}

#[derive(Debug, Default)]
pub struct AnalyzerState {
    pub clan_match_detection: ClanMatchDetection,
    pub current_time: GameTime,
    pub players: Vec<Player>,
    pub rounds: Vec<Round>,
    pub team_scores: TeamScores,
}

pub struct DemoInfo {
    pub demo_protocol: i32,
    pub network_protocol: i32,
    pub map_name: String,
}

pub struct Analysis {
    pub demo_info: DemoInfo,
    pub state: AnalyzerState,
}

pub use dod::Team;

#[derive(Debug, Default)]
pub struct TeamScores {
    current_scores: HashMap<Team, i32>,
    timeline: Vec<(GameTime, Team, i32)>,
}

impl TeamScores {
    pub fn get_team_score(&self, team: Team) -> i32 {
        self.timeline
            .iter()
            .rfind(|(_, t, _)| *t == team)
            .map(|(_, _, points)| *points)
            .unwrap_or(0)
    }

    pub fn add_team_score(&mut self, game_time: GameTime, team: Team, points: i32) {
        self.timeline.push((game_time, team, points));
    }

    pub fn iter(&self) -> impl Iterator<Item = &(GameTime, Team, i32)> {
        self.timeline.iter()
    }

    fn reset(&mut self) {
        self.current_scores.clear();
        self.timeline.clear();
    }
}

#[derive(Debug, Default)]
pub enum ClanMatchDetection {
    #[default]
    WaitingForReset,
    WaitingForNormal {
        reset_time: GameTime,
    },
    MatchIsLive,
}

impl Analysis {
    pub fn from_bytes(i: &[u8]) -> Self {
        let demo = open_demo_from_bytes(i).expect("Could not parse the file");

        let events = vec![AnalyzerEvent::Initialization].into_iter().chain(
            demo.directory
                .entries
                .iter()
                .flat_map(|entry| entry.frames.iter().flat_map(frame_to_events))
                .chain(vec![AnalyzerEvent::Finalization]),
        );

        let state = events.fold(AnalyzerState::default(), |mut state, ref event| {
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

        Analysis { state, demo_info }
    }
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

    fn find_player_by_id(&self, id: &PlayerGlobalId) -> Option<&Player> {
        self.players.iter().find(|player| player.id == *id)
    }

    fn find_player_by_id_mut(&mut self, id: &PlayerGlobalId) -> Option<&mut Player> {
        self.players.iter_mut().find(|player| player.id == *id)
    }
}

pub fn frame_to_events(frame: &Frame) -> Vec<AnalyzerEvent> {
    let mut events: Vec<AnalyzerEvent> = vec![];

    if let FrameData::NetworkMessage(frame_data) = &frame.frame_data {
        if let MessageData::Parsed(msgs) = &frame_data.1.messages {
            for net_msg in msgs {
                match net_msg {
                    NetMessage::UserMessage(user_msg) => {
                        if let Ok(dod_msg) = Message::new(&user_msg.name, &user_msg.data) {
                            events.push(AnalyzerEvent::UserMessage(dod_msg));
                        }
                    }
                    NetMessage::EngineMessage(engine_msg) => {
                        events.push(AnalyzerEvent::EngineMessage(engine_msg));
                    }
                }
            }
        }
    }

    events
}

pub fn use_timing_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    if let AnalyzerEvent::EngineMessage(EngineMessage::SvcTime(svc_time)) = event {
        if svc_time.time > 0. {
            let mut next_time = state.current_time.clone();

            next_time.offset = Duration::from_secs_f32(svc_time.time);

            state.current_time = next_time;
        }
    }
}

pub fn use_player_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    let svc_update_user_info = match event {
        AnalyzerEvent::EngineMessage(EngineMessage::SvcUpdateUserInfo(msg)) => Some(msg),
        _ => None,
    };

    if let Some(svc_update_user_info) = svc_update_user_info {
        let fields = svc_update_user_info
            .user_info
            .to_str()
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

        let id = fields
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
        if state.find_player_by_id(&id).is_none() {
            let insert_id = id.clone();
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
        if let Some(player) = state.find_player_by_id_mut(&id) {
            player
                .with_connection_status(ConnectionStatus::Connected {
                    client_id: svc_update_user_info.index,
                })
                .with_name(player_name)
                .with_team(
                    fields
                        .get("team")
                        .and_then(|team| Team::try_from(*team).ok()),
                );
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

        AnalyzerEvent::UserMessage(Message::ObjScore(obj_score)) => {
            let player = state.find_player_by_client_index_mut(obj_score.client_index - 1);

            if let Some(player) = player {
                player.stats.0 = obj_score.score as i32;
            }
        }

        AnalyzerEvent::UserMessage(Message::Frags(frags)) => {
            let player = state.find_player_by_client_index_mut(frags.client_index - 1);

            if let Some(player) = player {
                player.stats.1 = frags.frags as i32;
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

        let victim = state.find_player_by_client_index_mut(death_msg.victim_client_index - 1);

        if let Some(victim) = victim {
            // End the victim's current streak by adding a new record
            victim.kill_streaks.push(KillStreak::default());
        }

        if is_teamkill {
            return;
        }

        let killer = state.find_player_by_client_index_mut(death_msg.killer_client_index - 1);

        if let Some(killer) = killer {
            if killer.kill_streaks.is_empty() {
                killer.kill_streaks.push(KillStreak::default());
            }

            if let Some(killer_current_streak) = killer.kill_streaks.iter_mut().last() {
                killer_current_streak
                    .kills
                    .push((current_time, death_msg.weapon.clone()));
            }
        }
    } else if let AnalyzerEvent::UserMessage(Message::RoundState(RoundState::Reset)) = event {
        // Active kill streaks must be terminated when round is reset (i.e., after all objectives are captured)
        for player in state.players.iter_mut() {
            player.kill_streaks.push(KillStreak::default());
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
        state.team_scores.add_team_score(
            state.current_time.clone(),
            team_score.team.clone(),
            team_score.score as i32,
        );
    }
}

pub fn use_rounds_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    match event {
        AnalyzerEvent::Initialization => {
            if state.rounds.is_empty() {
                println!(
                    "t={:<20?} Initializing active round in case match is already live",
                    state.current_time.offset
                );

                state.rounds.push(Round::Active {
                    allies_kills: 0,
                    axis_kills: 0,
                    start_time: state.current_time.clone(),
                });
            }
        }

        AnalyzerEvent::Finalization => {
            if let Some(Round::Active { start_time, .. }) = state.rounds.pop() {
                println!(
                    "t={:<20?} Completing last round: start_time={:?}",
                    state.current_time.offset, start_time.offset
                );

                state.rounds.push(Round::Completed {
                    start_time: start_time.clone(),
                    end_time: state.current_time.clone(),
                    winner_stats: None,
                });
            }

            println!(
                "t={:<20?} Final state: sizeof={}",
                state.current_time.offset,
                state.rounds.len()
            );
        }

        AnalyzerEvent::UserMessage(Message::RoundState(round_state)) => {
            match round_state {
                RoundState::Reset => {
                    println!(
                        "t={:<20?} Starting new round from reset",
                        state.current_time.offset
                    );

                    state.rounds.push(Round::Active {
                        allies_kills: 0,
                        axis_kills: 0,
                        start_time: state.current_time.clone(),
                    });
                }

                RoundState::AlliesWin | RoundState::AxisWin => {
                    println!(
                        "t={:<20?} Observed a round win: {:?}",
                        state.current_time.offset, round_state
                    );

                    let active_round = state
                        .rounds
                        .pop()
                        .expect("Got a RoundState(Win) with no active round");

                    if let Round::Active {
                        start_time,
                        allies_kills,
                        axis_kills,
                    } = active_round
                    {
                        let winner_stats = if matches!(round_state, RoundState::AlliesWin) {
                            (Team::Allies, allies_kills)
                        } else {
                            (Team::Axis, axis_kills)
                        };

                        let completed_round = Round::Completed {
                            start_time,
                            end_time: state.current_time.clone(),
                            winner_stats: Some(winner_stats),
                        };

                        println!(
                            "t={:<20?} Adding completed round: {:?}",
                            state.current_time.offset, completed_round
                        );

                        state.rounds.push(completed_round);
                    } else {
                        panic!("Got a RoundState(Win) with no active round")
                    }
                }

                _ => {}
            };
        }

        AnalyzerEvent::UserMessage(Message::DeathMsg(death_msg)) => {
            let killer = state.find_player_by_client_index(death_msg.killer_client_index - 1);
            let victim = state.find_player_by_client_index(death_msg.victim_client_index - 1);

            let kill_info = match (killer, victim) {
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
                } else {
                    *axis_kills += 1;
                }
            }
        }

        _ => {}
    };
}

pub fn use_clan_match_detection_updates(
    max_normal_duration_from_reset: Duration,
    state: &mut AnalyzerState,
    event: &AnalyzerEvent,
) {
    if let AnalyzerEvent::UserMessage(Message::ClanTimer(_)) = event {
        // Match is already live, but we observed a ClanTimer. We infer that match is restarting.
        if let ClanMatchDetection::MatchIsLive = state.clan_match_detection {
            println!(
                "t={:<20?} Possible match restart detected via timer; waiting for reset",
                state.current_time.offset
            );

            state.clan_match_detection = ClanMatchDetection::WaitingForReset;
        }
    } else if let AnalyzerEvent::UserMessage(Message::RoundState(round_state)) = event {
        match (&state.clan_match_detection, round_state) {
            (ClanMatchDetection::WaitingForReset, RoundState::Reset) => {
                println!(
                    "t={:<20?} Possible match start detected; waiting for normal",
                    state.current_time.offset
                );

                state.clan_match_detection = ClanMatchDetection::WaitingForNormal {
                    reset_time: state.current_time.clone(),
                };
            }

            (ClanMatchDetection::WaitingForNormal { reset_time }, RoundState::Normal) => {
                println!(
                    "t={:<20?} Round reset freeze ended, checking player scores",
                    state.current_time.offset
                );

                let current_time = state.current_time.clone();

                // Players and teams have no score; we infer that this is the match start point
                if state
                    .players
                    .iter()
                    .all(|player| matches!(player.stats, (0, _, _)))
                    && state
                        .team_scores
                        .current_scores
                        .iter()
                        .all(|(_, score)| *score == 0)
                {
                    println!(
                        "t={:<20?} Match start detected via scores heuristic; clearing state",
                        current_time.offset
                    );

                    state.rounds.clear();
                    state.rounds.push(Round::Active {
                        allies_kills: 0,
                        axis_kills: 0,
                        start_time: reset_time.clone(),
                    });

                    state.team_scores.reset();

                    for player in state.players.iter_mut() {
                        player.kill_streaks.clear();
                        player.weapon_breakdown.clear();
                    }

                    state.clan_match_detection = ClanMatchDetection::MatchIsLive;
                } else {
                    println!(
                        "t={:<20?} Players with non-zero score!",
                        current_time.offset
                    );

                    for player in &state.players {
                        println!("\t{} {:?}", player.name, player.stats);
                    }
                }
            }

            _ => {}
        }
    } else if let ClanMatchDetection::WaitingForNormal { reset_time } = &state.clan_match_detection
    {
        if state.current_time.offset - reset_time.offset > max_normal_duration_from_reset {
            println!(
                "t={:<20?} Reached normal threshold, ignoring previous reset",
                state.current_time.offset,
            );

            state.clan_match_detection = ClanMatchDetection::WaitingForReset;
        }
    }
}
