use crate::{AnalyzerEvent, AnalyzerState, kill::KillStreak};
use dem::types::EngineMessage;
use dod::{Class, Team, Weapon};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PlayerGlobalId(String);

impl Display for PlayerGlobalId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SteamId(String);

impl Display for SteamId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<&PlayerGlobalId> for SteamId {
    type Error = std::num::ParseIntError;

    fn try_from(value: &PlayerGlobalId) -> Result<Self, Self::Error> {
        // https://github.com/jpcy/coldemoplayer/blob/9c97ab128ac889739c1643baf0d5fdf884d8a65f/compLexity%20Demo%20Player/Common.cs#L364-L383
        let id64 = value.to_string().parse::<u64>()?;
        let universe = 0; // Public

        let account_id = id64 - 76561197960265728;
        let server_id = if account_id % 2 == 0 { 0 } else { 1 };
        let account_id = (account_id - server_id) / 2;

        let steam_id = format!("STEAM_{}:{}:{}", universe, account_id & 1, account_id);

        Ok(SteamId(steam_id))
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
                // When present, *fid still seems unique to players across demos. Can it be mapped
                // to a SteamID64?
                //
                // ("93",      76561197960269086, "STEAM_0:0:1679"),  // Las1k
                // ("117",     76561197960269100, "STEAM_0:0:1686"),  // Money-B
                // ("100",     76561197960269104, "STEAM_0:0:1688"),  // scrd?
                // ("2761379", 76561197960366973, "STEAM_0:1:50622"), // jdub
                fields.get("*fid").map(|fid| format!("PLAYER_{fid}"))
            })
            .or_else(|| Some(format!("CONNECTION_{}", svc_update_user_info.id)))
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
