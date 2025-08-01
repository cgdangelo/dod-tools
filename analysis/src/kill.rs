use dod::{Message, RoundState, Weapon};
use crate::{AnalyzerEvent, AnalyzerState};
use crate::time::GameTime;

#[derive(Debug, Default)]
pub struct KillStreak {
    pub kills: Vec<(GameTime, Weapon)>,
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