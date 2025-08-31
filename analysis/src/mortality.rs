use crate::{AnalyzerEvent, AnalyzerState, Player, time::GameTime};
use dod::UserMessage;
use std::time::Duration;

/// Represents whether something is alive.
#[derive(Debug, PartialEq)]
pub enum Mortality {
    Alive,
    Dead,
}

pub trait MortalityState {
    /// Invoked when the [Mortality] state has changed.
    fn mortality_changed(&mut self, change: MortalityChange);

    /// Returns a history of when the [Mortality] state has changed.
    fn mortality_changes(&self) -> impl Iterator<Item = &MortalityChange>;

    /// Returns true if the object is alive.
    fn is_alive(&self) -> bool {
        self.mortality()
            .map(|mortality| matches!(mortality, Mortality::Alive))
            .unwrap_or(false)
    }

    /// Returns true if the object is dead.
    fn is_dead(&self) -> bool {
        self.mortality()
            .map(|mortality| matches!(mortality, Mortality::Dead))
            .unwrap_or(false)
    }

    /// Returns the current [Mortality] state.
    fn mortality(&self) -> Option<&Mortality> {
        self.mortality_changes()
            .last()
            .map(|change| change.mortality())
    }

    fn avg_lifespan(&self) -> Duration {
        #[derive(Default)]
        struct State<'a> {
            spawn_time: Option<&'a GameTime>,
            total_lifespan: Duration,
            num_spawns: usize,
        }

        let agg_state = self
            .mortality_changes()
            .fold(State::default(), |mut state, change| {
                match change.mortality() {
                    Mortality::Alive => {
                        if state.spawn_time.is_none() {
                            state.spawn_time = Some(change.time());
                            state.num_spawns += 1;
                        }
                    }

                    Mortality::Dead => {
                        if let Some(spawn_time) = state.spawn_time {
                            state.total_lifespan += change.time() - spawn_time;
                            state.spawn_time = None;
                        };
                    }
                };

                state
            });

        if agg_state.total_lifespan.is_zero() || agg_state.num_spawns == 0 {
            return Duration::ZERO;
        }

        agg_state.total_lifespan / agg_state.num_spawns as u32
    }
}

/// Timed event when an object's [Mortality] has changed.
#[derive(Debug)]
pub struct MortalityChange(GameTime, Mortality);

impl MortalityChange {
    pub fn new(time: GameTime, mortality: Mortality) -> Self {
        Self(time, mortality)
    }

    pub fn time(&self) -> &GameTime {
        &self.0
    }

    pub fn mortality(&self) -> &Mortality {
        &self.1
    }
}

impl MortalityState for Player {
    fn mortality_changed(&mut self, change: MortalityChange) {
        self.mortality.push(change);
    }

    fn mortality_changes(&self) -> impl Iterator<Item = &MortalityChange> {
        self.mortality.iter()
    }
}

pub fn with_mortality_detection(state: &mut AnalyzerState, event: &AnalyzerEvent) {
    if let AnalyzerEvent::Finalization = event {
        state.players.iter_mut().for_each(|player| {
            // Alive players need to be killed to get their final lifespans
            if player.is_alive() {
                player.mortality_changed(MortalityChange::new(
                    state.current_time.clone(),
                    Mortality::Dead,
                ));
            }
        });

        return;
    };

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
