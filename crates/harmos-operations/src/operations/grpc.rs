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
use std::borrow::Cow;

// Third Party Imports
use serde::{Deserialize, Serialize};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grpc {
    pub package: Cow<'static, str>,
    pub service: Cow<'static, str>,
    pub method: Cow<'static, str>,
    pub managed: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Grpc {
    pub const fn new(package: &'static str, service: &'static str, method: &'static str) -> Self {
        Self {
            package: Cow::Borrowed(package),
            service: Cow::Borrowed(service),
            method: Cow::Borrowed(method),
            managed: false,
        }
    }

    pub fn validate(&self) -> Result<(), super::ContractError> {
        let id = format!("{}.{}/{}", self.package, self.service, self.method);
        use harmos_protocol::schema::{Executor, operation_identity};
        if operation_identity(&id).map_err(|_| super::ContractError::Grpc)? != Executor::Host {
            return Err(super::ContractError::Grpc);
        }
        if self.package.is_empty()
            || self.package.len() > 255
            || self.service.len() > 255
            || self.service.contains('.')
            || self.method.len() > 255
            || self.package.starts_with("grpc.")
        {
            return Err(super::ContractError::Grpc);
        }
        Ok(())
    }

    /// Promise A Cooperative Final Result On This Method's Control Stream.
    pub const fn managed(mut self) -> Self {
        self.managed = true;
        self
    }

    pub fn path(&self) -> String {
        format!("/{}.{}/{}", self.package, self.service, self.method)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
