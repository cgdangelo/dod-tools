use crate::{AnalyzerEvent, AnalyzerState, time::GameTime};
use dod::{Message, Team};
use std::collections::HashMap;

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

    pub(crate) fn reset(&mut self) {
        self.current_scores.clear();
        self.timeline.clear();
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
