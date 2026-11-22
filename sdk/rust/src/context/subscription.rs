/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Rows Over the Existing Child Watch.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::marker::PhantomData;
use std::num::NonZeroUsize;

// Third Party Imports
use prost::Message;
use prost_types::Timestamp;

// Local Imports
use super::{Context, Watching};
use crate::{Error, Result, message};
use harmos_protocol::plugin::{SubscriptionsSubscribeRequest, SubscriptionsSubscribeResponse, subscriptions_subscribe_response};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct SubscriptionRow<R> {
    pub value: R,
    pub source: Timestamp,
    pub received: Timestamp,
}

#[must_use]
pub struct Subscription<R: message::Message> {
    watching: Watching<SubscriptionsSubscribeResponse>,
    row: PhantomData<fn() -> R>,
}

/* === Enums === */

pub enum SubscriptionItem<R> {
    Row(SubscriptionRow<R>),
    Gap { missed: u64 },
}

/* === Traits === */

/* === Implementations === */

impl Context {
    /// Subscribe Under This Call's Inherited Admission and Cancellation Scope.
    pub async fn subscribe<R: message::Message>(&self, stream: impl Into<String>, max_item_bytes: NonZeroUsize) -> Result<Subscription<R>> {
        let request = SubscriptionsSubscribeRequest {
            stream: stream.into(),
            row: R::name(),
            types: R::types().encode_to_vec(),
        };
        let mut watching = self.watch(request, max_item_bytes).await?;
        if !matches!(
            watching.next().await?,
            Some(SubscriptionsSubscribeResponse {
                item: Some(subscriptions_subscribe_response::Item::Opened(true))
            })
        ) {
            return Err(Error::Invalid("Subscribe to Stream: host must acknowledge the admitted demand".into()));
        }
        Ok(Subscription { watching, row: PhantomData })
    }
}

impl<R: message::Message> Subscription<R> {
    pub async fn next(&mut self) -> Result<Option<SubscriptionItem<R>>> {
        let Some(response) = self.watching.next().await? else {
            return Ok(None);
        };
        let item = match response.item {
            Some(subscriptions_subscribe_response::Item::Missed(missed)) => SubscriptionItem::Gap { missed },
            Some(subscriptions_subscribe_response::Item::Row(row)) => SubscriptionItem::Row(SubscriptionRow {
                value: R::decode(row.payload.as_slice()).map_err(|error| Error::Invalid(format!("Read Subscription Row: {error}")))?,
                source: row.source.ok_or_else(|| Error::Invalid("Read Subscription Row: source timestamp is required".into()))?,
                received: row.received.ok_or_else(|| Error::Invalid("Read Subscription Row: receipt timestamp is required".into()))?,
            }),
            _ => return Err(Error::Invalid("Read Subscription: row or gap item is required".into())),
        };
        Ok(Some(item))
    }

    /// Release Only This Demand; Other Subscriptions and the Provider Continue.
    pub fn close(&mut self) {
        self.watching.close();
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::context::ChildOutput;
    use harmos_protocol::host::TimeGetResponse;
    use harmos_protocol::plugin::{InstanceInvokeResponse, Request, SubscriptionRow};
    use std::collections::{BTreeMap, HashMap, VecDeque};
    use std::sync::Arc;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn typed_rows_gaps_and_close_use_independent_child_demands() {
        let fingerprint = message::request_fingerprint::<SubscriptionsSubscribeRequest>().unwrap();
        let mut context = Context::new(
            Request::default(),
            "fixture".into(),
            Arc::new(HashMap::from([("harmos.plugin.v1.Subscriptions/Subscribe".into(), fingerprint)])),
        );
        let (output, mut input) = mpsc::channel(1);
        context.queue.output = Some(ChildOutput::Invoke(output));
        let peer = context.clone();
        let source = Timestamp { seconds: 7, nanos: 123456789 };
        let received = Timestamp { seconds: 11, nanos: 987654321 };
        let driver = tokio::spawn(async move {
            let mut responses = BTreeMap::<u64, VecDeque<SubscriptionsSubscribeResponse>>::new();
            while let Some(Ok(InstanceInvokeResponse { frame: Some(frame) })) = input.recv().await {
                match frame {
                    harmos_protocol::plugin::instance_invoke_response::Frame::Child(child) => {
                        responses.insert(
                            child.call,
                            VecDeque::from([
                                SubscriptionsSubscribeResponse {
                                    item: Some(subscriptions_subscribe_response::Item::Opened(true)),
                                },
                                SubscriptionsSubscribeResponse {
                                    item: Some(subscriptions_subscribe_response::Item::Row(SubscriptionRow {
                                        payload: TimeGetResponse { now: Some(source) }.encode_to_vec(),
                                        source: Some(source),
                                        received: Some(received),
                                    })),
                                },
                                SubscriptionsSubscribeResponse {
                                    item: Some(subscriptions_subscribe_response::Item::Missed(17)),
                                },
                            ]),
                        );
                    }
                    harmos_protocol::plugin::instance_invoke_response::Frame::ChildDemand(demand) => {
                        let response = responses.get_mut(&demand.call).unwrap().pop_front().unwrap();
                        let sender = peer.queue.calls.lock().unwrap().watches[&demand.call].sender.clone();
                        sender.send(Ok(Some(response.encode_to_vec()))).await.unwrap();
                    }
                    harmos_protocol::plugin::instance_invoke_response::Frame::ChildCancel(_) => {}
                    _ => panic!("Unexpected Fixture Carrier Frame"),
                }
            }
        });
        let limit = NonZeroUsize::new(4096).unwrap();
        let mut first = context.subscribe::<TimeGetResponse>("fixture.samples", limit).await.unwrap();
        let mut second = context.subscribe::<TimeGetResponse>("fixture.samples", limit).await.unwrap();
        let Some(SubscriptionItem::Row(row)) = first.next().await.unwrap() else {
            panic!("Expected Typed Row");
        };
        assert_eq!((row.value.now, row.source, row.received), (Some(source), source, received));
        assert!(matches!(first.next().await.unwrap(), Some(SubscriptionItem::Gap { missed: 17 })));
        let first_call = first.watching.call;
        let second_call = second.watching.call;
        first.close();
        first.close();
        assert!(first.next().await.unwrap().is_none());
        assert!(!context.queue.calls.lock().unwrap().watches.contains_key(&first_call));
        assert!(context.queue.calls.lock().unwrap().watches.contains_key(&second_call));
        assert!(matches!(second.next().await.unwrap(), Some(SubscriptionItem::Row(_))));
        context.cancel_calls("Fixture Scope Ended").await.unwrap();
        assert!(second.next().await.is_err());
        driver.abort();
        assert!(driver.await.unwrap_err().is_cancelled());
    }
}
