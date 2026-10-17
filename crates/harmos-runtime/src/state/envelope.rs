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
pub use harmos_domain::state::{DefinitionId, Envelope, FactSource, Kind, SystemFact};

/* === Imports === */
use std::any::Any;
use std::sync::Arc;
use crate::Application;

/* === Enums === */
pub enum Recorded<A: Application> {
    Transaction(Arc<A::Transaction>),
    Record(A::Record),
    System(SystemFact),
}


/* === Implementations === */
impl<A: Application> Clone for Recorded<A>
where
    A::Record: Clone,
{
    fn clone(&self) -> Self {
        match self {
            Self::Transaction(value) => Self::Transaction(Arc::clone(value)),
            Self::Record(value) => Self::Record(value.clone()),
            Self::System(value) => Self::System(value.clone()),
        }
    }
}

impl<A: Application> Recorded<A> {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Transaction(_) => Kind::Transaction,
            Self::Record(_) => Kind::Record,
            Self::System(_) => Kind::Record,
        }
    }

    pub(crate) fn select<X: Clone + 'static>(&self) -> Option<X> {
        if let Some(value) = (self as &dyn Any).downcast_ref::<X>() {
            return Some(value.clone());
        }
        match self {
            Self::Transaction(value) => (&**value as &dyn Any).downcast_ref::<X>().cloned(),
            Self::Record(value) => (value as &dyn Any).downcast_ref::<X>().cloned(),
            Self::System(value) => (value as &dyn Any).downcast_ref::<X>().cloned(),
        }
    }
}

