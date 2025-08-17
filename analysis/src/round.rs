use crate::time::GameTime;
use crate::{AnalyzerEvent, AnalyzerState};
use dod::{RoundState, Team, UserMessage};

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

        AnalyzerEvent::UserMessage(UserMessage::RoundState(round_state)) => {
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

        AnalyzerEvent::UserMessage(UserMessage::DeathMsg(death_msg)) => {
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
