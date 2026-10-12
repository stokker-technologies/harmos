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
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
// Third Party Imports

// Local Imports
use harmos::operations::{Context, Operation, host};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Endpoint {
    pub answering: Duration,
    pub calls: Arc<AtomicUsize>,
    pub prepared: Arc<AtomicUsize>,
}

pub struct Egress {
    resources: Arc<Endpoint>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl Operation for Egress {
    type Declaration = host::Egress;
    type Resources = Endpoint;
    type Error = harmos::Error;
    type Endpoints = ();
    type Context<'a> = Context<'a>;
    fn new(resources: Arc<Endpoint>) -> harmos::Result<Self> {
        resources.prepared.fetch_add(1, Ordering::SeqCst);
        Ok(Self { resources })
    }
    async fn run<'ctx>(
        &mut self,
        context: Self::Context<'ctx>,
        request: &mut host::EgressRequest,
        _endpoints: &mut Self::Endpoints,
    ) -> harmos::Result<host::Fetched> {
        // Admission Already Checked The Caller's Egress Grant; The Provider
        // Owns Which Destinations Exist And How Their Paths Resolve.
        context.caller_scope().ok_or(harmos::Error::InvalidInput {
            field: "fixture egress caller",
            reason: "original plugin scope is required",
        })?;
        if request.name != "vendor"
            || !request.path.starts_with('/')
            || request.path.starts_with("//")
            || request.path.contains('\\')
            || request.path.contains('#')
            || request
                .path
                .split('?')
                .next()
                .unwrap()
                .split('/')
                .any(|part| matches!(part, "." | ".."))
            || request.path.bytes().any(|byte| byte.is_ascii_control())
            || request.headers.len() > 64
            || request
                .headers
                .iter()
                .map(|header| header.name.len() + header.value.len())
                .sum::<usize>()
                > 64 * 1024
            || request.body.len() > 8 * 1024 * 1024
        {
            return Err(harmos::Error::InvalidInput {
                field: "fixture egress",
                reason: "request differs from its exact selected bounded endpoint",
            });
        }
        self.resources.calls.fetch_add(1, Ordering::SeqCst);
        if !self.resources.answering.is_zero() {
            tokio::time::sleep(self.resources.answering).await;
        }
        Ok(host::Fetched {
            status: 200,
            headers: vec![],
            body: vec![],
        })
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
