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
    match (&state.clan_match_detection, event) {
        // Assume the first RoundState with a reset is the match going live
        (
            ClanMatchDetection::WaitingForReset,
            AnalyzerEvent::UserMessage(Message::RoundState(RoundState::Reset)),
        ) => {
            state.clan_match_detection = ClanMatchDetection::WaitingForNormal {
                reset_time: state.current_time.clone(),
            };
        }

        // Players and teams are scoreless after a reset; we infer the match is live
        (
            ClanMatchDetection::WaitingForNormal { reset_time },
            AnalyzerEvent::UserMessage(Message::RoundState(RoundState::Normal)),
        ) if state
            .players
            .iter()
            .all(|player| matches!(player.stats, (0, _, _)))
            && state.team_scores.get_team_score(Team::Allies) == 0
            && state.team_scores.get_team_score(Team::Axis) == 0 =>
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

        // Too much time passed since the round reset. We infer that detector is stuck.
        (ClanMatchDetection::WaitingForNormal { reset_time }, _)
            if &state.current_time - reset_time > max_normal_duration_from_reset =>
        {
            state.clan_match_detection = ClanMatchDetection::WaitingForReset;
        }

        // Match is already live, but we observed a ClanTimer. We infer that match is restarting.
        (ClanMatchDetection::MatchIsLive, AnalyzerEvent::UserMessage(Message::ClanTimer(_))) => {
            state.clan_match_detection = ClanMatchDetection::WaitingForReset
        }

        _ => {}
    };
}
