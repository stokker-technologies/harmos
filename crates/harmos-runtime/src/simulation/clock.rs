/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::time::{Duration, SystemTime};
// Third Party Imports
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone)]
pub(crate) struct Clock {
    pub(crate) host: crate::clock::Clock,
    origin: tokio::time::Instant,
    pub(crate) controller: crate::clock::Controller,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Clock {
    pub(crate) fn new() -> Self {
        let (host, controller) = crate::clock::Clock::virtual_at(SystemTime::UNIX_EPOCH);
        let origin = host.now();
        Self { host, origin, controller }
    }

    pub(crate) async fn advance_ms(&self, milliseconds: u64) {
        self.controller.advance_by(Duration::from_millis(milliseconds)).await;
    }

    pub(crate) fn elapsed_ms(&self) -> u64 {
        self.host.now().saturating_duration_since(self.origin).as_millis().try_into().expect("simulation elapsed milliseconds fit u64")
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{FactSource, SystemFact};

    use crate::clock::tests::Model;

    #[test]
    fn evidence_dates_retain_the_host_epoch_across_reboot() {
        // Publish Evidence At The Fixed Epoch
        let mut sim = crate::Runtime::<Model>::simulate((), (), 7).unwrap();
        let runtime = sim.runtime.as_ref().unwrap();
        let fact = SystemFact::new(FactSource::Work("clock_test".to_owned()), "first".to_owned(), vec![]);
        let first = sim.executor.block_on(runtime.records.publish_system(fact)).unwrap();
        sim.executor.block_on(runtime.state.wait_until_stored(first.frontier));
        let first_date = sim.memory.shared.entries.lock().unwrap()[0].metadata.at;
        assert_eq!(first_date, SystemTime::UNIX_EPOCH);

        // Reboot After Advancing The Host Clock
        sim.advance_ms(5000);
        sim.reboot().unwrap();
        let runtime = sim.runtime.as_ref().unwrap();
        let fact = SystemFact::new(FactSource::Work("clock_test".to_owned()), "second".to_owned(), vec![]);
        let second = sim.executor.block_on(runtime.records.publish_system(fact)).unwrap();
        sim.executor.block_on(runtime.state.wait_until_stored(second.frontier));

        // Recovery Preserves The First Date And New Evidence Uses The Retained Clock
        let dates: Vec<_> = sim.memory.shared.entries.lock().unwrap().iter().map(|entry| entry.metadata.at).collect();
        assert_eq!(dates, [first_date, SystemTime::UNIX_EPOCH + Duration::from_secs(5)]);
        assert_eq!(sim.executor.elapsed_ms(), 5000);
        sim.assert_replay_equivalent();
        sim.stop().unwrap();
    }
}
