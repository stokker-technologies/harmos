/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Scoped Activation Publication Lifetime

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::Arc;

// Third Party Imports
use tokio::sync::watch;

// Local Imports
use super::{Activation, Error, Outcome, publish::Publications};
use crate::{
    Application, Runtime,
    clock::Clock,
    operations::{Manager, clock::Unit},
};
use harmos_operations::clock::ClockAccess;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/// Owns One Trusted Scope's Clock, Calls and Samples on an Admitted Activation.
#[must_use = "retain the binding until its scoped work has settled"]
pub struct ScopedBinding {
    operations: Manager,
    close: watch::Sender<bool>,
    completed: watch::Receiver<Option<Outcome>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<A: Application> Runtime<A> {
    /// Bind Samples and Descendant Time Calls Without Opening Another Connection.
    pub async fn bind_grpc(&self, activation: &Activation, scope: impl Into<String>, clock: Clock) -> Result<ScopedBinding, Error> {
        let source =
            activation.publications.get().ok_or_else(|| Error::Invalid("Bind Scoped Plugin: activation is not admitted".into()))?;
        if !Arc::ptr_eq(&self.operations.registry, &source.operations.registry) {
            return Err(Error::Invalid("Bind Scoped Plugin: activation belongs to another runtime".into()));
        }
        if *source.close.borrow() || !activation.is_connected() {
            return Err(Error::Disconnected);
        }
        let scope = scope.into();
        let operations =
            source.operations.with_scope(scope.clone()).map_err(|error| Error::Invalid(format!("Bind Scoped Plugin: {error}")))?;
        let unit = Arc::new(Unit::new(clock.clone()));
        let mut context = operations.call_context().clone();
        context
            .insert(ClockAccess(Arc::new(crate::operations::clock::Clock(clock.clone()))))
            .map_err(|error| Error::Invalid(error.to_string()))?;
        context.insert(unit.clone()).map_err(|error| Error::Invalid(error.to_string()))?;
        let operations = operations.with_context(context);
        let (close, closing) = watch::channel(false);
        let (completed, completion) = watch::channel(None);
        {
            let mut scopes = source.scopes.lock().expect("scoped publications lock is healthy");
            scopes.retain(|_, completed| completed.borrow().is_none() && completed.has_changed().is_ok());
            if scopes.contains_key(&scope) {
                return Err(Error::Invalid("Bind Scoped Plugin: scope already has an active binding".into()));
            }
            scopes.insert(scope, completion.clone());
        }
        let prepared = Publications::prepare(&self.streams, activation, &source.settings, &operations, true).await?;
        let mut publications = Publications::scoped(prepared, source, activation, &clock, &operations);
        let parent = source.close.clone();
        tokio::spawn(async move {
            tokio::select! {
                () = super::disconnected(closing.clone()) => {},
                () = super::disconnected(parent) => {},
            }
            let pending = unit.stop();
            let mut result = publications.close().await.map_err(Arc::new);
            for instance in pending {
                if let Err(error) = instance.settle().await {
                    result = Err(Arc::new(Error::Invalid(format!("Close Scoped Plugin: {error}"))));
                }
            }
            completed.send_replace(Some(result));
        });
        Ok(ScopedBinding { operations, close, completed: completion })
    }
}

impl ScopedBinding {
    pub fn operations(&self) -> &Manager {
        &self.operations
    }

    /// Stop Scoped Calls and Await Accepted Ingestion and Producer Cleanup.
    pub async fn close(mut self) -> Outcome {
        self.close.send_replace(true);
        loop {
            if let Some(result) = self.completed.borrow_and_update().clone() {
                return result;
            }
            if self.completed.changed().await.is_err() {
                return Err(Arc::new(Error::Disconnected));
            }
        }
    }
}

impl Drop for ScopedBinding {
    fn drop(&mut self) {
        self.close.send_replace(true);
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
