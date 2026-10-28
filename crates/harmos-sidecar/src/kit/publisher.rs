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
use std::marker::PhantomData;
use std::sync::Arc;
// Third Party Imports

// Local Imports
use super::{CallError, ObserverContext, ObserverHost, ObserverPublisher, ObserverStreamPublisher, PublicationError, Supervisor};
use harmos_domain::{Message, StreamSpec, plugin::Declared};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Publisher<P> {
    supervisor: Supervisor,
    _plugin: PhantomData<fn() -> P>,
}

// Rows for one stream, each encoded once when pushed and stamped with the
// time it was produced: `push` reads the kit clock, never earlier than the
// previous stamp, and `push_at` takes an explicit stamp. Publishing sends the
// stamps, not the send time.
pub struct Batch<S: StreamSpec> {
    payloads: Vec<Vec<u8>>,
    stamps: Vec<Option<i64>>,
    latest: Option<i64>,
    clock: fn() -> Option<i64>,
    stream: PhantomData<fn(S)>,
}

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error(transparent)]
    Publication(#[from] PublicationError),

    #[error(transparent)]
    Call(#[from] CallError),
}

/* === Traits === */

/* === Implementations === */

impl From<PublishError> for harmos_domain::Refusal {
    fn from(error: PublishError) -> Self {
        match error {
            PublishError::Call(error) => error.into(),
            error => Self::of(error),
        }
    }
}

impl<P> Clone for Publisher<P> {
    fn clone(&self) -> Self {
        Self {
            supervisor: self.supervisor.clone(),
            _plugin: PhantomData,
        }
    }
}

impl Supervisor {
    pub fn publisher<P: Declared>(&self) -> Publisher<P> {
        Publisher {
            supervisor: self.clone(),
            _plugin: PhantomData,
        }
    }
}

impl<P: Declared> Publisher<P> {
    pub fn supervisor(&self) -> &Supervisor {
        &self.supervisor
    }

    pub async fn publish_on<S: StreamSpec>(&self, row: &S::Row) -> Result<(), PublishError>
    where
        S::Row: Message,
    {
        self.publish_on_context::<S>(row, None).await
    }

    pub async fn publish_on_context<S: StreamSpec>(&self, row: &S::Row, context: Option<&str>) -> Result<(), PublishError>
    where
        S::Row: Message,
    {
        Self::declared::<S>()?;
        let stamps = [self.supervisor.now()];
        self.supervisor.publication::<S>(&[row.encode()], &stamps, context).await
    }

    pub fn batch<S: StreamSpec>(&self) -> Batch<S> {
        Batch {
            payloads: Vec::new(),
            stamps: Vec::new(),
            latest: None,
            clock: self.supervisor.clock(),
            stream: PhantomData,
        }
    }

    pub async fn publish_batch_on<S: StreamSpec>(&self, batch: &Batch<S>) -> Result<(), PublishError> {
        self.publish_batch_on_context::<S>(batch, None).await
    }

    // Publish rows in order with contiguous sequences and their own stamps,
    // as one `host.publish` call where they fit (at most one window of
    // rows). Each call answers once the host's ingest has taken in every row
    // published before it on the stream, so no row is dropped unread. Rows
    // before a refused row stay published.
    pub async fn publish_batch_on_context<S: StreamSpec>(&self, batch: &Batch<S>, context: Option<&str>) -> Result<(), PublishError> {
        Self::declared::<S>()?;
        self.supervisor.publication::<S>(&batch.payloads, &batch.stamps, context).await
    }

    fn declared<S: StreamSpec>() -> Result<(), PublicationError> {
        if P::STREAMS.iter().any(|entry| entry.id == S::ID && entry.schema == S::VERSION) {
            Ok(())
        } else {
            Err(PublicationError::Undeclared { stream: S::ID })
        }
    }
}

impl<S: StreamSpec> Batch<S>
where
    S::Row: Message,
{
    pub fn push(&mut self, row: S::Row) {
        let stamp = match ((self.clock)(), self.latest) {
            (Some(now), Some(latest)) => Some(now.max(latest)),
            (now, latest) => now.or(latest),
        };
        self.stamped(row.encode(), stamp);
    }

    pub fn push_at(&mut self, row: S::Row, published_unix_micros: i64) {
        self.stamped(row.encode(), Some(published_unix_micros));
    }
}

impl<S: StreamSpec> Batch<S> {
    // A row its producer already encoded as the stream's row message; the host
    // validates it against the stream schema like any other publication.
    pub fn push_encoded_at(&mut self, payload: Vec<u8>, published_unix_micros: i64) {
        self.stamped(payload, Some(published_unix_micros));
    }

    pub fn payloads(&self) -> &[Vec<u8>] {
        &self.payloads
    }

    pub fn len(&self) -> usize {
        self.payloads.len()
    }

    pub fn is_empty(&self) -> bool {
        self.payloads.is_empty()
    }

    // Empty the batch for reuse; later `push` stamps still never go back.
    pub fn clear(&mut self) {
        self.payloads.clear();
        self.stamps.clear();
    }

    fn stamped(&mut self, payload: Vec<u8>, stamp: Option<i64>) {
        self.latest = self.latest.max(stamp);
        self.payloads.push(payload);
        self.stamps.push(stamp);
    }
}

impl<P: Declared> ObserverHost for Publisher<P> {
    fn bind(_: Arc<harmos_sdk::ExecutionContext>, supervisor: Supervisor) -> Self {
        supervisor.publisher::<P>()
    }
}

impl<V: StreamSpec<Row = V> + harmos_domain::schema::Described + Message, P: Declared> ObserverPublisher<V> for Publisher<P> {
    type Error = PublishError;
    const STREAM: Option<harmos_domain::inventory::publications::Entry> = Some(harmos_domain::inventory::publications::Entry::of::<V>());

    async fn publish(&self, _: &ObserverContext<'_, Self>, value: V) -> Result<(), PublishError> {
        self.publish_on::<V>(&value).await
    }
}

impl<S: StreamSpec, P: Declared> ObserverStreamPublisher<S> for Publisher<P>
where
    S::Row: Message,
{
    type Error = PublishError;

    async fn publish_on(&self, _: &ObserverContext<'_, Self>, row: &S::Row) -> Result<(), PublishError> {
        Publisher::publish_on::<S>(self, row).await
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::Batch;
    use harmos_domain::{Message, StreamSpec};
    use std::marker::PhantomData;

    fn rows(batch: &Batch<Samples>) -> Vec<u8> {
        batch.payloads().iter().map(|payload| u8::decode(payload).unwrap()).collect()
    }

    struct Samples;

    impl StreamSpec for Samples {
        type Row = u8;
        const ID: &'static str = "batch.clock";
        const VERSION: u32 = 1;
        const WINDOW: usize = 8;
    }

    #[test]
    fn clock_never_regresses() {
        let mut batch = Batch::<Samples> {
            payloads: Vec::new(),
            stamps: Vec::new(),
            latest: None,
            clock: || None,
            stream: PhantomData,
        };
        batch.push(0);
        batch.clock = || Some(100);
        batch.push(1);
        batch.clock = || Some(90);
        batch.push(2);
        batch.clock = || None;
        batch.push(3);
        batch.clock = || Some(120);
        batch.push(4);
        assert_eq!(rows(&batch), [0, 1, 2, 3, 4]);
        assert_eq!(batch.stamps, [None, Some(100), Some(100), Some(100), Some(120)]);
        batch.clear();
        assert!(batch.is_empty());
        batch.clock = || Some(110);
        batch.push(5);
        assert_eq!(batch.stamps, [Some(120)], "reuse retains the previous high watermark");
    }

    #[test]
    fn explicit_source_stamps() {
        let mut batch = Batch::<Samples> {
            payloads: Vec::new(),
            stamps: Vec::new(),
            latest: None,
            clock: || panic!("explicit stamps do not consult the kit clock"),
            stream: PhantomData,
        };
        batch.push_at(1, 100);
        batch.push_at(2, 90);
        batch.push_encoded_at(7_u8.encode(), 95);
        assert_eq!(rows(&batch), [1, 2, 7]);
        assert_eq!(batch.stamps, [Some(100), Some(90), Some(95)]);
        batch.clock = || Some(80);
        batch.push(3);
        assert_eq!(batch.stamps, [Some(100), Some(90), Some(95), Some(100)]);
    }

    #[test]
    fn publication_retains_encoding() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };

        struct Counted(Arc<AtomicUsize>);
        impl Message for Counted {
            const CODEC: &'static str = u8::CODEC;
            fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_domain::MessageError> {
                u8::decoding_memory(bytes)
            }
            fn encode(&self) -> Vec<u8> {
                self.0.fetch_add(1, Ordering::Relaxed);
                7_u8.encode()
            }
            fn encoded_size(&self) -> usize {
                panic!("publication must size retained bytes");
            }
            fn decode(_: &[u8]) -> Result<Self, harmos_domain::MessageError> {
                panic!("publication must forward retained bytes");
            }
        }
        struct Stream;
        impl StreamSpec for Stream {
            type Row = Counted;
            const ID: &'static str = "batch.once";
            const VERSION: u32 = 1;
            const WINDOW: usize = 8;
        }

        let encodes = Arc::new(AtomicUsize::new(0));
        let mut batch = Batch::<Stream> {
            payloads: Vec::new(),
            stamps: Vec::new(),
            latest: None,
            clock: || None,
            stream: PhantomData,
        };
        batch.push_at(Counted(Arc::clone(&encodes)), 100);
        batch.push_encoded_at(8_u8.encode(), 90);

        let (outgoing, _receiver) = tokio::sync::mpsc::channel(1);
        let host = super::Supervisor::new(outgoing, || None);
        let taken = host
            .with_publication::<Stream, super::CallError>(batch.payloads(), &batch.stamps, Some("scope"), 0, |request| {
                let wire = request.encode();
                let received = harmos_operations::streams::Publish::decode(&wire).unwrap();
                assert_eq!(received.payload, [7]);
                assert_eq!(received.following[0].payload, [8]);
                assert_eq!(received.published_unix_micros, Some(100));
                assert_eq!(received.following[0].published_delta_micros, Some(-10));
                assert_eq!(received.context.as_deref(), Some("scope"));
                Ok(())
            })
            .unwrap();
        assert_eq!(taken, 2);
        assert_eq!(encodes.load(Ordering::Relaxed), 1);
    }
}
