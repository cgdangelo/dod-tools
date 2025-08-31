use crate::time::GameTime;
use crate::{AnalyzerEvent, AnalyzerState, Player};
use dod::UserMessage;

trait MortalityState {
    fn is_dead(&self) -> bool;

    fn mortality(&self) -> Option<&Mortality>;

    fn mortality_changed(&mut self, change: MortalityChange);
}

#[derive(Debug)]
pub struct MortalityChange(GameTime, Mortality);

impl MortalityState for Player {
    fn is_dead(&self) -> bool {
        self.mortality()
            .map(|mortality| *mortality == Mortality::Dead)
            .unwrap_or(false)
    }

    fn mortality(&self) -> Option<&Mortality> {
        self.mortality.iter().last().map(|change| &change.1)
    }

    fn mortality_changed(&mut self, change: MortalityChange) {
        self.mortality.push(change);
    }
}

/// Represents whether a [Player] is alive.
#[derive(Debug, PartialEq)]
pub enum Mortality {
    Alive,
    Dead,
}

pub fn with_mortality_detection(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    let mortality_change = match event {
        AnalyzerEvent::UserMessage(UserMessage::DeathMsg(death_msg)) => {
            Some((death_msg.victim_client_index - 1, Mortality::Dead))
        }

        AnalyzerEvent::UserMessage(UserMessage::PStatus(p_status)) => {
            Some((p_status.client_index - 1, Mortality::Alive))
        }

        _ => None,
    };

    mortality_change.and_then(|(client_index, mortality)| {
        let current_time = state.current_time.clone();
        let player = state.find_player_by_client_index_mut(client_index)?;

        if player.mortality() != Some(&mortality) {
            player
                .mortality
                .push(MortalityChange(current_time, mortality));
        }

        Some(())
    });
}
