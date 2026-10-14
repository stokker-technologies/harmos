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
use std::{
    num::NonZeroUsize,
    sync::{Arc, atomic::Ordering},
};
// Third Party Imports
// Local Imports
use super::{
    Owner,
    contract::{Download, Exchange, Upload},
};
use harmos_contract::operations::streaming::{StreamInput, StreamOutput, StreamSettings};
use harmos_guest::Context;
use harmos_guest::operations::{Operation, ServingError, WireStreamLimits, WireTable};

/* === Constants === */

/* === Macros === */
macro_rules! serving {
    ($owner:ident, $declaration:ident, $endpoints:ty, $request:ident, $ports:ident, $run:block) => {
        struct $owner(Arc<Owner>);
        impl Operation for $owner {
            type Declaration = $declaration;
            type Resources = Owner;
            type Error = std::io::Error;
            type Endpoints = $endpoints;
            type Context<'a> = &'a Context;
            async fn initialize<'a>(
                counts: Arc<Owner>,
                _: Self::Context<'a>,
            ) -> Result<Self, Self::Error> {
                counts.initialized.fetch_add(1, Ordering::SeqCst);
                Ok(Self(counts))
            }
            async fn run<'a>(
                &mut self,
                _: Self::Context<'a>,
                $request: &mut u32,
                $ports: &mut Self::Endpoints,
            ) -> Result<harmos_guest::operations::Response<u32>, Self::Error> {
                let result: ::core::result::Result<u32, Self::Error> = async move {
                    self.0.ran.fetch_add(1, Ordering::SeqCst);
                    $run
                }
                .await;
                result.map(::core::convert::Into::into)
            }
            async fn stop(&mut self) -> Result<(), Self::Error> {
                self.0.stopped.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            async fn cleanup(&mut self) -> Result<(), Self::Error> {
                self.0.finalized.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
    };
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */
serving!(Uploading, Upload, StreamInput<u32>, request, input, {
    let mut total = *request;
    while let Some(item) = input.recv().await {
        total += *item;
    }
    Ok(total)
});
serving!(Downloading, Download, StreamOutput<u32>, request, output, {
    for value in [0, *request] {
        output
            .send_with(4, || Ok(value))
            .await
            .map_err(std::io::Error::other)?;
    }
    Ok(*request)
});
serving!(
    Exchanging,
    Exchange,
    (StreamInput<u32>, StreamOutput<u32>),
    request,
    ports,
    {
        let mut total = *request;
        while let Some(item) = ports.0.recv().await {
            total += *item;
            ports
                .1
                .send_with(4, || Ok(total))
                .await
                .map_err(std::io::Error::other)?;
        }
        Ok(total)
    }
);

/* === Public Functions === */
pub(super) fn bind(table: &mut WireTable<Context>, counts: Arc<Owner>) -> Result<(), ServingError> {
    let direction = StreamSettings {
        items: NonZeroUsize::MIN,
        bytes: NonZeroUsize::new(64).unwrap(),
    };
    let limits = WireStreamLimits {
        input: direction,
        output: direction,
    };
    table.bind_streaming_operation::<Uploading>(counts.clone(), limits)?;
    table.bind_streaming_operation::<Downloading>(counts.clone(), limits)?;
    table.bind_streaming_operation::<Exchanging>(counts, limits)
}

/* === Private Functions === */

/* === Tests === */
