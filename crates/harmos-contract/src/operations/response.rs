/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Response Handles Retain Their Original Payload Owner Through Delivery And Encoding.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::any::{Any, TypeId};
use std::borrow::Borrow;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */
type Projection = fn(&dyn Any) -> Option<&dyn Any>;

/* === Structs === */
pub struct Response<T: Send + Sync + 'static> {
    erased: ErasedResponse,
    payload: PhantomData<fn() -> T>,
}

// Only Typed Constructors Can Associate An Owner With Its Payload Projection.
#[derive(Clone)]
pub struct ErasedResponse {
    owner: Arc<dyn Any + Send + Sync>,
    projection: Projection,
    payload: TypeId,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<T: Send + Sync + 'static> Response<T> {
    pub fn new(value: T) -> Self {
        Self::from_owner(Arc::new(value))
    }

    pub fn from_owner<U: Borrow<T> + Send + Sync + 'static>(owner: Arc<U>) -> Self {
        Self {
            erased: ErasedResponse {
                owner,
                projection: project::<T, U>,
                payload: TypeId::of::<T>(),
            },
            payload: PhantomData,
        }
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.erased.owner, &other.erased.owner)
    }

    pub fn try_unwrap(self) -> Result<T, Self> {
        // Projected Owners Keep Their Loan; Only A Direct Payload Can Be Moved Out.
        if !self.erased.owner.is::<T>() {
            return Err(self);
        }
        let owner = match self.erased.owner.downcast::<T>() {
            Ok(owner) => owner,
            Err(_) => unreachable!("checked direct response owner type"),
        };
        Arc::try_unwrap(owner).map_err(Self::from_owner)
    }

    pub fn into_erased(self) -> ErasedResponse {
        self.erased
    }
}

impl<T: Send + Sync + 'static> From<T> for Response<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

impl<T: Send + Sync + 'static> From<Arc<T>> for Response<T> {
    fn from(value: Arc<T>) -> Self {
        Self::from_owner(value)
    }
}

impl<T: Send + Sync + 'static> Clone for Response<T> {
    fn clone(&self) -> Self {
        Self {
            erased: self.erased.clone(),
            payload: PhantomData,
        }
    }
}

impl<T: Send + Sync + 'static> Deref for Response<T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.erased
            .value::<T>()
            .expect("response retains its declared payload")
    }
}

impl<T: Send + Sync + 'static> AsRef<T> for Response<T> {
    fn as_ref(&self) -> &T {
        self
    }
}

impl<T: Send + Sync + 'static> Borrow<T> for Response<T> {
    fn borrow(&self) -> &T {
        self
    }
}

impl<T: fmt::Debug + Send + Sync + 'static> fmt::Debug for Response<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deref().fmt(formatter)
    }
}

impl<T: fmt::Display + Send + Sync + 'static> fmt::Display for Response<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deref().fmt(formatter)
    }
}

impl<T: PartialEq + Send + Sync + 'static> PartialEq for Response<T> {
    fn eq(&self, other: &Self) -> bool {
        self.deref() == other.deref()
    }
}

impl<T: Eq + Send + Sync + 'static> Eq for Response<T> {}

impl ErasedResponse {
    pub fn restore<T: Send + Sync + 'static>(self) -> Option<Response<T>> {
        // Typed Constructors Seal This Association; Restoration Must Not Run Borrow Code.
        if self.payload != TypeId::of::<T>() {
            return None;
        }
        Some(Response {
            erased: self,
            payload: PhantomData,
        })
    }

    pub(super) fn value<T: Send + Sync + 'static>(&self) -> Option<&T> {
        if self.payload != TypeId::of::<T>() {
            return None;
        }
        (self.projection)(&*self.owner)?.downcast_ref::<T>()
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn project<T: Send + Sync + 'static, U: Borrow<T> + Send + Sync + 'static>(
    owner: &dyn Any,
) -> Option<&dyn Any> {
    let owner = owner.downcast_ref::<U>()?;
    Some(owner.borrow())
}

/* === Tests === */
