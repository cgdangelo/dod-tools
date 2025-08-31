use crate::{AnalyzerEvent, AnalyzerState, Player, time::GameTime};
use dod::UserMessage;

/// Represents whether something is alive.
#[derive(Debug, PartialEq)]
pub enum Mortality {
    Alive,
    Dead,
}

pub trait MortalityState {
    /// Returns true if the object is dead.
    fn is_dead(&self) -> bool {
        self.mortality()
            .map(|mortality| matches!(mortality, Mortality::Dead))
            .unwrap_or(false)
    }

    /// Returns the current [Mortality] state.
    fn mortality(&self) -> Option<&Mortality>;

    /// Invoked when the [Mortality] state has changed.
    fn mortality_changed(&mut self, change: MortalityChange);
}

/// Timed event when an object's [Mortality] has changed.
#[derive(Debug)]
pub struct MortalityChange(GameTime, Mortality);

impl MortalityChange {
    pub fn time(&self) -> &GameTime {
        &self.0
    }

    pub fn mortality(&self) -> &Mortality {
        &self.1
    }
}

impl MortalityState for Player {
    fn mortality(&self) -> Option<&Mortality> {
        self.mortality
            .iter()
            .last()
            .map(|change| change.mortality())
    }

    fn mortality_changed(&mut self, change: MortalityChange) {
        self.mortality.push(change);
    }
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
            player.mortality_changed(MortalityChange(current_time, mortality));
        }

        Some(())
    });
}
