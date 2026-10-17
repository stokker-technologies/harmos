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
use std::num::NonZeroUsize;
use std::sync::Arc;
// Third Party Imports
use tokio::io::AsyncWriteExt;
// Local Imports
use super::{Opening, Supervisor, catalog, local};
use crate::inventory::{Declaration, Facility};
use crate::{Attach, Attached, PluginCatalog, Refusal, StreamDeclaration};
use harmos_sdk::plugin::{Declared, Owner, Plugin};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct PluginNative<P> {
    declaration: Declaration,
    capacity: NonZeroUsize,
    _plugin: PhantomData<fn() -> P>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<P: Plugin<Context = Supervisor> + Declared> PluginNative<P> {
    pub fn new(capacity: NonZeroUsize) -> std::io::Result<Self> {
        let markers = std::hint::black_box(P::markers().map_err(std::io::Error::other)?);
        let declaration = Declaration::inspect(&markers.concat(), Facility::Sidecar).map_err(std::io::Error::other)?;
        harmos_domain::inventory::publications::validate_match(P::STREAMS, &declaration.streams).map_err(std::io::Error::other)?;
        let manifest = &declaration.contracts;
        let provides = crate::operations::Contracts::from_tree(P::PROVIDES).map_err(std::io::Error::other)?;
        let requires = harmos_domain::inventory::contracts::Required::from_tree(P::REQUIRES).map_err(std::io::Error::other)?;
        let listeners = harmos_domain::listeners::Declarations::from_tree(P::LISTENERS).map_err(std::io::Error::other)?;
        harmos_domain::inventory::listeners::validate(&listeners, &provides).map_err(std::io::Error::other)?;
        let pair = [
            crate::operations::lifecycle::Descriptor::of_initialize::<P>().map_err(std::io::Error::other)?,
            crate::operations::lifecycle::Descriptor::of_shutdown::<P>().map_err(std::io::Error::other)?,
        ];
        if manifest.owner != P::ID || manifest.provides != provides || manifest.requires != requires || manifest.lifecycle != pair {
            return Err(std::io::Error::other("plugin SDK manifest differs from its authored contracts"));
        }
        if declaration.listeners != listeners {
            return Err(std::io::Error::other("listener manifest differs from authored selection"));
        }
        let mut observers = Vec::new();
        harmos_domain::catalog::try_for_each(P::OBSERVERS, |entry| {
            observers.push(entry.declaration());
            Ok::<(), std::io::Error>(())
        })?;
        if declaration.observers != observers {
            return Err(std::io::Error::other("observer manifest differs from authored selection"));
        }
        if declaration.sections != P::CONTRIBUTIONS {
            return Err(std::io::Error::other("contribution sections differ from authored selection"));
        }
        Ok(Self { declaration, capacity, _plugin: PhantomData })
    }
}

impl<P> Clone for PluginNative<P> {
    fn clone(&self) -> Self {
        Self { declaration: self.declaration.clone(), capacity: self.capacity, _plugin: PhantomData }
    }
}

impl<P: Plugin<Context = Supervisor> + Declared> Attach for PluginNative<P> {
    type Error = std::io::Error;

    fn owner(&self) -> &str {
        &self.declaration.id
    }

    fn publications(&self) -> Vec<StreamDeclaration> {
        self.declaration.streams.clone()
    }

    fn operations(&self) -> harmos_domain::Result<Vec<crate::operations::Definition>> {
        Ok(self.declaration.operations.clone())
    }

    fn operation_contracts(&self) -> harmos_domain::Result<Option<Vec<crate::operations::Contract>>> {
        Ok(Some(self.declaration.contracts.provides.clone()))
    }

    fn required_contracts(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::inventory::contracts::Required>>> {
        Ok(Some(self.declaration.contracts.requires.clone()))
    }

    fn sdk_lifecycle(&self) -> harmos_domain::Result<Option<[crate::operations::lifecycle::Descriptor; 2]>> {
        Ok(Some(self.declaration.contracts.lifecycle.clone()))
    }

    fn observers(&self) -> Vec<crate::inventory::observers::Declaration> {
        self.declaration.observers.clone()
    }

    fn observe_contract(&self) -> harmos_domain::Result<Option<crate::operations::Contract>> {
        Ok(self.declaration.observe_contract())
    }

    fn listener_declarations(&self) -> harmos_domain::Result<Option<Vec<harmos_domain::listeners::Definition>>> {
        Ok(Some(self.declaration.listeners.clone()))
    }

    async fn attach(&self) -> std::io::Result<Attached> {
        Ok(local::binary(opening(self)?))
    }
}

/* === Public Functions === */
pub async fn run_plugin<P>(capacity: NonZeroUsize) -> std::io::Result<()>
where
    P: Plugin<Context = Supervisor> + Declared,
{
    std::hint::black_box(P::markers().map_err(std::io::Error::other)?);
    if catalog::requested(std::env::args_os().skip(1)) {
        let bytes = catalog::encode::<P>()?;
        let mut stdout = tokio::io::stdout();
        stdout.write_all(&bytes).await?;
        return stdout.flush().await;
    }
    #[cfg(any(unix, windows))]
    if let Some(control) = crate::attach::control::inherited()? {
        return serve_plugin_channels::<P>(capacity, (Box::new(tokio::io::stdin()), Box::new(tokio::io::stdout())), control).await;
    }
    let _ = capacity;
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "stdio launch requires an attached binary control channel"))
}

pub async fn serve_plugin_channels<P>(
    capacity: NonZeroUsize, data: crate::transport::Io, control: crate::transport::Io,
) -> std::io::Result<()>
where
    P: Plugin<Context = Supervisor> + Declared,
{
    let native = PluginNative::<P>::new(capacity)?;
    let opening = Arc::new(tokio::sync::Mutex::new(opening(&native)?));
    super::connection::run_binary(opening, data, control, std::future::pending()).await
}

/* === Private Functions === */
fn opening<P>(native: &PluginNative<P>) -> std::io::Result<Opening>
where
    P: Plugin<Context = Supervisor> + Declared,
{
    let definitions = native.declaration.operations.clone();
    let contracts = native.declaration.contracts.provides.clone();
    let registrations = catalog::registrations::<P>().map_err(std::io::Error::other)?;
    let (owner, table) =
        Owner::<P>::new_with_observers(contracts, native.capacity, native.declaration.observers.clone()).map_err(std::io::Error::other)?;
    let interrupted_owner = Arc::clone(&owner);
    Ok(Opening {
        operations: table,
        retirement: Box::new(move || {
            let owner = Arc::clone(&owner);
            Box::pin(async move { owner.retire().await.map_err(Refusal::of) })
        }),
        quiescence: Box::new(move || {
            let owner = Arc::clone(&interrupted_owner);
            Box::pin(async move { owner.quiesce().await.map_err(Refusal::of) })
        }),
        catalog: PluginCatalog { registrations, operations: definitions, observers: native.declaration.observers.clone() },
    })
}

/* === Tests === */
