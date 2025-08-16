use crate::{round::Round, time::GameTime, AnalyzerEvent, AnalyzerState};
use dod::{Message, RoundState, Team};
use std::time::Duration;

#[derive(Debug, Default)]
pub enum ClanMatchDetection {
    #[default]
    WaitingForReset,
    WaitingForNormal {
        reset_time: GameTime,
    },
    MatchIsLive,
}

pub fn use_clan_match_detection_updates(
    max_normal_duration_from_reset: Duration,
    state: &mut AnalyzerState,
    event: &AnalyzerEvent,
) {
    if let AnalyzerEvent::UserMessage(Message::ClanTimer(_)) = event {
        // Match is already live, but we observed a ClanTimer. We infer that match is restarting.
        if let ClanMatchDetection::MatchIsLive = state.clan_match_detection {
            state.clan_match_detection = ClanMatchDetection::WaitingForReset;
        }
    } else if let AnalyzerEvent::UserMessage(Message::RoundState(round_state)) = event {
        match (&state.clan_match_detection, round_state) {
            (ClanMatchDetection::WaitingForReset, RoundState::Reset) => {
                state.clan_match_detection = ClanMatchDetection::WaitingForNormal {
                    reset_time: state.current_time.clone(),
                };
            }

            (ClanMatchDetection::WaitingForNormal { reset_time }, RoundState::Normal) => {
                // Players and teams have no score; we infer that this is the match start point
                if state
                    .players
                    .iter()
                    .all(|player| matches!(player.stats, (0, _, _)))
                    && state.team_scores.get_team_score(Team::Allies) == 0
                    && state.team_scores.get_team_score(Team::Axis) == 0
                {
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
                }
            }

            _ => {}
        }
    } else if let ClanMatchDetection::WaitingForNormal { reset_time } = &state.clan_match_detection
        && state.current_time.offset - reset_time.offset > max_normal_duration_from_reset
    {
        state.clan_match_detection = ClanMatchDetection::WaitingForReset;
    }
}
