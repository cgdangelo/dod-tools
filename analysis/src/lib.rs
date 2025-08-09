mod clan_match;
mod kill;
mod player;
mod scoreboard;
mod time;

use crate::{
    clan_match::{ClanMatchDetection, use_clan_match_detection_updates, use_rounds_updates},
    kill::{use_kill_streak_updates, use_weapon_breakdown_updates},
    player::use_player_updates,
    scoreboard::{TeamScores, use_scoreboard_updates, use_team_score_updates},
    time::{GameTime, use_timing_updates},
};
use dem::{
    open_demo_from_bytes,
    types::{EngineMessage, Frame, FrameData, MessageData, NetMessage},
};
use dod::Message;
use std::time::Duration;

pub use crate::{
    clan_match::Round,
    player::{ConnectionStatus, Player, PlayerGlobalId, SteamId},
};
pub use dod::Team;

pub enum AnalyzerEvent<'a> {
    Initialization,
    EngineMessage(&'a EngineMessage),
    UserMessage(Message),
    Finalization,
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
    pub demo_protocol: i32,
    pub network_protocol: i32,
    pub map_name: String,
}

pub struct Analysis {
    pub demo_info: DemoInfo,
    pub state: AnalyzerState,
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
