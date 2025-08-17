use crate::{AnalyzerEvent, AnalyzerState, time::GameTime};
use dod::{Team, UserMessage};
use std::cmp::Ordering;
use std::collections::HashMap;

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

pub fn use_scoreboard_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    match event {
        AnalyzerEvent::UserMessage(UserMessage::PClass(p_class)) => {
            let player = state.find_player_by_client_index_mut(p_class.client_index - 1);

            if let Some(player) = player {
                player.class = Some(p_class.class.clone());
            };
        }

        AnalyzerEvent::UserMessage(UserMessage::PTeam(p_team)) => {
            let player = state.find_player_by_client_index_mut(p_team.client_index - 1);

            if let Some(player) = player {
                player.team = Some(p_team.team.clone());
            };
        }

        AnalyzerEvent::UserMessage(UserMessage::ScoreShort(score_short)) => {
            let player = state.find_player_by_client_index_mut(score_short.client_index - 1);

            if let Some(player) = player {
                player.stats = (
                    score_short.score as i32,
                    score_short.kills as i32,
                    score_short.deaths as i32,
                );
            }
        }

        AnalyzerEvent::UserMessage(UserMessage::ObjScore(obj_score)) => {
            let player = state.find_player_by_client_index_mut(obj_score.client_index - 1);

            if let Some(player) = player {
                player.stats.0 = obj_score.score as i32;
            }
        }

        AnalyzerEvent::UserMessage(UserMessage::Frags(frags)) => {
            let player = state.find_player_by_client_index_mut(frags.client_index - 1);

            if let Some(player) = player {
                player.stats.1 = frags.frags as i32;
            }
        }

        AnalyzerEvent::Finalization => {
            state
                .players
                .sort_by(|left, right| match (&left.team, &right.team) {
                    (Some(left_team), Some(right_team)) if left_team == right_team => {
                        let by_points = left.stats.0.cmp(&right.stats.0).reverse();
                        let by_kills = left.stats.1.cmp(&right.stats.1).reverse();
                        let by_deaths = left.stats.2.cmp(&right.stats.2);

                        by_points.then(by_kills).then(by_deaths)
                    }

                    (Some(Team::Allies), _) => Ordering::Less,
                    (Some(Team::Axis), Some(Team::Spectators)) => Ordering::Less,
                    (Some(Team::Spectators) | None, _) => Ordering::Greater,

                    _ => Ordering::Equal,
                });
        }

        _ => {}
    };
}

pub fn use_team_score_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    if let AnalyzerEvent::UserMessage(UserMessage::TeamScore(team_score)) = event {
        state.team_scores.add_team_score(
            state.current_time.clone(),
            team_score.team.clone(),
            team_score.score as i32,
        );
    }
}
