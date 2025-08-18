mod clan_match;
mod kill;
mod player;
mod round;
mod scoreboard;
mod time;

use crate::{
    clan_match::{ClanMatchDetection, use_clan_match_detection_updates},
    kill::{use_kill_streak_updates, use_weapon_breakdown_updates},
    player::use_player_updates,
    round::use_rounds_updates,
    scoreboard::{TeamScores, use_scoreboard_updates, use_team_score_updates},
    time::{GameTime, use_timing_updates},
};
use dem::{
    open_demo_from_bytes,
    types::{Demo, EngineMessage, Frame, FrameData, MessageData, NetMessage},
};
use dod::UserMessage;
use std::time::Duration;

pub use crate::{
    player::{ConnectionStatus, Player, PlayerGlobalId, SteamId},
    round::Round,
};
pub use dod::Team;

pub enum AnalyzerEvent<'a> {
    Initialization,
    Finalization,

    Frame(&'a Frame),
    EngineMessage(&'a EngineMessage),
    UserMessage(UserMessage),
}

impl<'a> AnalyzerEvent<'a> {
    fn from_dem(frame: &'a Frame) -> Vec<Self> {
        let mut events: Vec<Self> = vec![];

        events.push(AnalyzerEvent::Frame(frame));

        if let FrameData::NetworkMessage(box_type) = &frame.frame_data {
            match &box_type.1.messages {
                MessageData::Parsed(msgs) => msgs.iter(),
                _ => [].iter(),
            }
            .filter_map(|net_msg| match net_msg {
                NetMessage::EngineMessage(engine_msg) => Some(Self::EngineMessage(engine_msg)),
                NetMessage::UserMessage(user_msg) => {
                    UserMessage::new(&user_msg.name, &user_msg.data)
                        .ok()
                        .map(Self::UserMessage)
                }
            })
            .for_each(|event| events.push(event))
        };

        events
    }
}

#[derive(Debug, Default)]
pub struct AnalyzerState {
    clan_match_detection: ClanMatchDetection,
    current_time: GameTime,

    pub players: Vec<Player>,
    pub rounds: Vec<Round>,
    pub team_scores: TeamScores,
}

pub struct DemoInfo {
    /// Version of the demo protocol used to encode the demo.
    pub demo_protocol: i32,

    /// Name of the map the demo was recorded on.
    pub map_name: String,

    /// Version of the network protocol used during the game.
    pub network_protocol: i32,
}

impl From<Demo> for DemoInfo {
    fn from(value: Demo) -> Self {
        let map_name = value
            .header
            .map_name
            .to_str()
            .map(|s| s.trim_end_matches('\x00'))
            .unwrap()
            .to_string();

        Self {
            demo_protocol: value.header.demo_protocol,
            map_name,
            network_protocol: value.header.network_protocol,
        }
    }
}

pub struct Analysis {
    pub demo_info: DemoInfo,
    pub state: AnalyzerState,
}

impl Analysis {
    fn new(demo_info: DemoInfo, state: AnalyzerState) -> Self {
        Self { demo_info, state }
    }
}

impl<'a> From<&'a [u8]> for Analysis {
    fn from(value: &'a [u8]) -> Self {
        let demo = open_demo_from_bytes(value).expect("Could not parse the file");

        let events = vec![AnalyzerEvent::Initialization]
            .into_iter()
            .chain(
                demo.directory
                    .entries
                    .iter()
                    .flat_map(|entry| entry.frames.iter())
                    .flat_map(AnalyzerEvent::from_dem),
            )
            .chain(vec![AnalyzerEvent::Finalization]);

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

        Analysis::new(demo.into(), state)
    }
}

impl AnalyzerState {
    fn find_player_by_client_index(&self, client_index: u8) -> Option<&Player> {
        self.players
            .iter()
            .find(|player| match player.connection_status {
                ConnectionStatus::Connected { client_id } => client_id == client_index,
                _ => false,
            })
    }

    fn find_player_by_client_index_mut(&mut self, client_index: u8) -> Option<&mut Player> {
        self.players
            .iter_mut()
            .find(|player| match player.connection_status {
                ConnectionStatus::Connected { client_id } => client_id == client_index,
                _ => false,
            })
    }

    fn find_player_by_id(&self, id: &PlayerGlobalId) -> Option<&Player> {
        self.players.iter().find(|player| player.id == *id)
    }

    fn find_player_by_id_mut(&mut self, id: &PlayerGlobalId) -> Option<&mut Player> {
        self.players.iter_mut().find(|player| player.id == *id)
    }
}
