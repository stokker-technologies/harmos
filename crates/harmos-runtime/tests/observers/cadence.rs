/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Runtime Cadence Is An Activation Snapshot Over The Producing Resources.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::pin::pin;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

// Third Party Imports
use tokio::sync::mpsc;

// Local Imports
use super::pending;
use harmos_runtime::sidecar::{Instance, ObserverContext, ObserverError, ObserverPublisher};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Settings {
    interval: Mutex<Resolution>,
    resolutions: AtomicUsize,
    attempts: AtomicUsize,
}

struct Configured;

#[derive(Default)]
struct DeclaredCadence;

struct Publisher(mpsc::UnboundedSender<(u64, Duration)>);

/* === Enums === */

#[derive(Clone, Copy)]
enum Resolution {
    Interval(Duration),
    Refused,
    Panicked,
}

/* === Traits === */

/* === Implementations === */

impl ObserverPublisher<(u64, Duration)> for Publisher {
    type Error = harmos_runtime::sidecar::PublicationError;

    async fn publish(&self, _: &ObserverContext<'_, Self>, sample: (u64, Duration)) -> Result<(), Self::Error> {
        self.0.send(sample).unwrap();
        Ok(())
    }
}

#[harmos_runtime::observer(interval_ms = 101)]
impl Configured {
    fn interval(context: &harmos_sdk::ExecutionContext) -> Result<Duration, std::io::Error> {
        let settings = context.resource::<Settings>().map_err(std::io::Error::other)?;
        settings.resolutions.fetch_add(1, Ordering::SeqCst);
        let resolution = *settings.interval.lock().unwrap();
        match resolution {
            Resolution::Interval(interval) => Ok(interval),
            Resolution::Refused => Err(std::io::Error::from_raw_os_error(73)),
            Resolution::Panicked => panic!("cadence callback panicked"),
        }
    }

    async fn initialize(context: &harmos_sdk::ExecutionContext) -> Result<Self, std::io::Error> {
        let settings = context.resource::<Settings>().map_err(std::io::Error::other)?;
        if settings.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
            return Err(std::io::Error::other("retry acquisition"));
        }
        Ok(Self)
    }

    async fn observe(&mut self, context: &ObserverContext<'_, Publisher>) -> Result<(), std::io::Error> {
        context.publish((context.tick().ordinal(), context.tick().elapsed())).await.map_err(std::io::Error::other)
    }
}

#[harmos_runtime::observer(interval_ms = 23)]
impl DeclaredCadence {
    async fn observe(&mut self, context: &ObserverContext<'_, Publisher>) -> Result<(), std::io::Error> {
        context.publish((context.tick().ordinal(), context.tick().elapsed())).await.map_err(std::io::Error::other)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn resources(interval: Resolution) -> Arc<Settings> {
    Arc::new(Settings {
        interval: Mutex::new(interval),
        resolutions: AtomicUsize::new(0),
        attempts: AtomicUsize::new(0),
    })
}

/* === Tests === */

#[tokio::test(start_paused = true)]
async fn resource_cadence() {
    // Declared Cadence Without An Override
    use harmos_runtime::sidecar::Observer;

    let cadence = <DeclaredCadence as Observer<Publisher>>::INTERVAL;
    let (published, mut samples) = mpsc::unbounded_channel();
    let mut activity = Instance::<Publisher>::of::<DeclaredCadence>()
        .activate(Arc::new(harmos_sdk::ExecutionContext::default()), Publisher(published))
        .unwrap();
    {
        let mut observing = pin!(activity.observe(|| {}));
        pending::parked(observing.as_mut()).await;
        for ordinal in 1..=3 {
            tokio::time::advance(cadence - Duration::from_millis(1)).await;
            pending::parked(observing.as_mut()).await;
            assert!(samples.try_recv().is_err());
            tokio::time::advance(Duration::from_millis(1)).await;
            pending::parked(observing.as_mut()).await;
            assert_eq!(samples.try_recv().unwrap(), (u64::from(ordinal), cadence * ordinal));
        }
    }
    activity.stop().await.unwrap();
    activity.cleanup().await.unwrap();

    // Resource Override And Reactivation
    let cadence = Duration::from_millis(37);
    let settings = resources(Resolution::Interval(cadence));
    let (published, mut samples) = mpsc::unbounded_channel();
    let instance = Instance::<Publisher>::of::<Configured>();
    let mut activity = instance.activate(super::observer_context(settings.clone()), Publisher(published)).unwrap();
    assert_eq!(settings.resolutions.load(Ordering::SeqCst), 1);
    assert_eq!(settings.attempts.load(Ordering::SeqCst), 0);
    {
        let mut observing = pin!(activity.observe(|| {}));
        pending::parked(observing.as_mut()).await;
        assert_eq!(settings.attempts.load(Ordering::SeqCst), 1);
        *settings.interval.lock().unwrap() = Resolution::Interval(Duration::from_millis(2));

        tokio::time::advance(cadence - Duration::from_millis(1)).await;
        pending::parked(observing.as_mut()).await;
        assert_eq!(settings.attempts.load(Ordering::SeqCst), 1);
        assert!(samples.try_recv().is_err());
        tokio::time::advance(Duration::from_millis(1)).await;
        pending::parked(observing.as_mut()).await;
        assert_eq!(settings.attempts.load(Ordering::SeqCst), 2);
        assert!(samples.try_recv().is_err());

        tokio::time::advance(cadence - Duration::from_millis(1)).await;
        pending::parked(observing.as_mut()).await;
        assert!(samples.try_recv().is_err());
        tokio::time::advance(Duration::from_millis(1)).await;
        pending::parked(observing.as_mut()).await;
        assert_eq!(samples.try_recv().unwrap(), (1, cadence));

        tokio::time::advance(cadence * 5).await;
        pending::parked(observing.as_mut()).await;
        assert_eq!(samples.try_recv().unwrap(), (2, cadence * 6));
        assert!(samples.try_recv().is_err(), "missed ticks do not burst");
        tokio::time::advance(cadence).await;
        pending::parked(observing.as_mut()).await;
        assert_eq!(samples.try_recv().unwrap(), (3, cadence * 7));
    }
    activity.stop().await.unwrap();
    activity.cleanup().await.unwrap();
    assert!(!activity.is_initialized());
    assert_eq!(settings.resolutions.load(Ordering::SeqCst), 1);

    let renewed_cadence = Duration::from_millis(2);
    let (published, mut samples) = mpsc::unbounded_channel();
    let mut renewed = instance.activate(super::observer_context(settings.clone()), Publisher(published)).unwrap();
    assert_eq!(settings.resolutions.load(Ordering::SeqCst), 2);
    {
        let mut observing = pin!(renewed.observe(|| {}));
        pending::parked(observing.as_mut()).await;
        assert_eq!(settings.attempts.load(Ordering::SeqCst), 3);
        assert!(samples.try_recv().is_err());
        tokio::time::advance(renewed_cadence).await;
        pending::parked(observing.as_mut()).await;
        assert_eq!(samples.try_recv().unwrap(), (1, renewed_cadence));
    }
    renewed.stop().await.unwrap();
    renewed.cleanup().await.unwrap();

    for resolution in [Resolution::Interval(Duration::ZERO), Resolution::Refused, Resolution::Panicked] {
        let settings = resources(resolution);
        let (published, mut samples) = mpsc::unbounded_channel();
        let failure = instance.activate(super::observer_context(settings.clone()), Publisher(published)).err().unwrap();
        match resolution {
            Resolution::Interval(_) => assert!(matches!(failure, ObserverError::InvalidInterval)),
            Resolution::Refused => {
                let ObserverError::IntervalFailed(source) = failure else { panic!("authored error is preserved") };
                assert_eq!(source.downcast_ref::<std::io::Error>().unwrap().raw_os_error(), Some(73));
            }
            Resolution::Panicked => assert!(matches!(failure, ObserverError::IntervalPanicked)),
        }
        tokio::time::advance(cadence * 10).await;
        assert_eq!(settings.resolutions.load(Ordering::SeqCst), 1);
        assert_eq!(settings.attempts.load(Ordering::SeqCst), 0);
        assert!(samples.recv().await.is_none());
    }
}
