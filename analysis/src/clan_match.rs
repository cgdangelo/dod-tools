use crate::{AnalyzerEvent, AnalyzerState};
use dod::{Message, RoundState, Team};
use std::time::Duration;
use crate::time::GameTime;

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
                    && state
                        .team_scores
                        .current_scores
                        .iter()
                        .all(|(_, score)| *score == 0)
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
    {
        if state.current_time.offset - reset_time.offset > max_normal_duration_from_reset {
            state.clan_match_detection = ClanMatchDetection::WaitingForReset;
        }
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

#[derive(Debug, Default)]
pub enum ClanMatchDetection {
    #[default]
    WaitingForReset,
    WaitingForNormal {
        reset_time: GameTime,
    },
    MatchIsLive,
}

pub fn use_rounds_updates(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    match event {
        AnalyzerEvent::Initialization => {
            if state.rounds.is_empty() {
                state.rounds.push(Round::Active {
                    allies_kills: 0,
                    axis_kills: 0,
                    start_time: state.current_time.clone(),
                });
            }
        }

        AnalyzerEvent::Finalization => {
            if let Some(Round::Active { start_time, .. }) = state.rounds.pop() {
                state.rounds.push(Round::Completed {
                    start_time: start_time.clone(),
                    end_time: state.current_time.clone(),
                    winner_stats: None,
                });
            }
        }

        AnalyzerEvent::UserMessage(Message::RoundState(round_state)) => {
            match round_state {
                RoundState::Reset => {
                    state.rounds.push(Round::Active {
                        allies_kills: 0,
                        axis_kills: 0,
                        start_time: state.current_time.clone(),
                    });
                }

                RoundState::AlliesWin | RoundState::AxisWin => {
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