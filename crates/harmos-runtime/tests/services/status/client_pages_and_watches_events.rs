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

// Third Party Imports
use harmos_protocol::host::status as wire;
use tracing_subscriber::prelude::*;

// Local Imports
use super::support::*;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn client_pages_and_watches_events() -> Result {
    // Retain Redacted Log and Trace Events Under a Controlled Clock
    let host = Host::start().await?;
    let secret = "example-secret-material";
    host.status.redaction.register(secret.into())?;
    for index in 0..2 {
        let event = wire::Event {
            message: format!("event {index}: {secret}"),
            fields: std::collections::HashMap::from([("token".into(), "another-sensitive-value".into())]),
            ..Default::default()
        };
        host.status.record_log(event.clone())?;
        host.status.record_trace(event)?;
    }
    let mut logs = wire::logs_client::LogsClient::new(host.channel.clone());
    let mut traces = wire::trace_events_client::TraceEventsClient::new(host.channel.clone());
    let first = logs
        .list(wire::LogsListRequest {
            at: None,
            page: Some(page(1, String::new())),
        })
        .await?
        .into_inner();
    let trace_first = traces
        .list(wire::TraceEventsListRequest {
            at: None,
            page: Some(page(1, String::new())),
        })
        .await?
        .into_inner();
    assert_eq!(first.events.len(), 1);
    assert!(!first.next_page_token.is_empty());
    assert!(!first.events[0].message.contains(secret));
    assert!(!first.events[0].fields["token"].contains("another-sensitive-value"));
    assert_eq!(first.events[0].observed_at.as_ref().unwrap().seconds, 0);
    // New Events Do Not Move an Existing Page Boundary
    let third = wire::Event {
        message: format!("third {secret}"),
        ..Default::default()
    };
    host.status.record_log(third.clone())?;
    host.status.record_trace(third)?;
    let second = logs
        .list(wire::LogsListRequest {
            at: first.resolved_at.clone(),
            page: Some(page(1, first.next_page_token.clone())),
        })
        .await?
        .into_inner();
    let trace_second = traces
        .list(wire::TraceEventsListRequest {
            at: trace_first.resolved_at.clone(),
            page: Some(page(1, trace_first.next_page_token.clone())),
        })
        .await?
        .into_inner();
    assert_eq!(second.resolved_at, first.resolved_at);
    assert_eq!(second.events[0].sequence, 2);
    assert!(second.next_page_token.is_empty());
    assert_eq!(trace_second.events[0].sequence, 2);
    assert!(
        traces
            .list(wire::TraceEventsListRequest {
                at: None,
                page: Some(page(1, first.next_page_token))
            })
            .await
            .is_err()
    );
    let mut invented = first.resolved_at.clone().unwrap();
    invented.sequence = 900;
    assert!(
        logs.list(wire::LogsListRequest {
            at: Some(invented),
            page: Some(page(1, String::new()))
        })
        .await
        .is_err()
    );
    // Follow From the Page Cut Without Losing the Opening Race
    let mut log_watch = logs
        .watch(wire::LogsWatchRequest {
            at: first.resolved_at,
            max_bytes: 4096,
            max_events: 1,
        })
        .await?
        .into_inner();
    let mut trace_watch = traces
        .watch(wire::TraceEventsWatchRequest {
            at: trace_first.resolved_at,
            max_bytes: 4096,
            max_events: 1,
        })
        .await?
        .into_inner();
    assert!(log_watch.message().await?.unwrap().events.is_empty());
    assert!(trace_watch.message().await?.unwrap().events.is_empty());
    assert_eq!(log_watch.message().await?.unwrap().events[0].sequence, 3);
    assert_eq!(trace_watch.message().await?.unwrap().events[0].sequence, 3);
    let subscriber = tracing_subscriber::registry().with(host.status.layer());
    tracing::subscriber::with_default(subscriber, || {
        tracing::info!(target:"example.host",credential="unregistered-sensitive-value",message=%format!("fourth {secret}"));
        let span = tracing::info_span!("host.action", password = "span-secret-value");
        let _entered = span.enter();
    });
    let log = log_watch.message().await?.unwrap();
    let trace = trace_watch.message().await?.unwrap();
    assert_eq!(log.events[0].sequence, 4);
    assert_eq!(trace.events[0].sequence, 4);
    assert!(!log.events[0].message.contains(secret));
    assert!(!log.events[0].fields["credential"].contains("unregistered-sensitive-value"));
    let span_open = trace_watch.message().await?.unwrap();
    assert_eq!(span_open.events[0].fields["event"], "span_open");
    assert!(!span_open.events[0].fields["password"].contains("span-secret-value"));
    let span_close = trace_watch.message().await?.unwrap();
    assert_eq!(span_close.events[0].fields["event"], "span_close");
    // A Slow Watch Receives an Explicit Retention Gap
    let old = log.resolved_at;
    let mut slow = logs.watch(wire::LogsWatchRequest { at: old, max_bytes: 4096, max_events: 1 }).await?.into_inner();
    slow.message().await?;
    for _ in 0..20 {
        host.status.record_log(wire::Event {
            message: "retention event".into(),
            ..Default::default()
        })?;
    }
    assert!(slow.message().await.is_err());
    // A Fresh List Cannot Resolve an Evicted Issued Position
    assert!(
        logs.list(wire::LogsListRequest {
            at: second.resolved_at,
            page: Some(page(1, String::new()))
        })
        .await
        .is_err()
    );
    drop((slow, log_watch, trace_watch, logs, traces));
    host.close().await
}
