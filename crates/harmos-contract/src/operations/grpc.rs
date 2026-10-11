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
}

/* === Enums === */

/* === Traits === */
#[cfg(feature = "grpc")]
pub trait GrpcVisitor: Sized {
    type Entry;

    fn unary<O>(self) -> Self::Entry
    where
        O: super::WireOperation,
        O::Request: crate::Protobuf + crate::MemorySize,
        O::Response: crate::Protobuf;

    fn upload<O>(self) -> Self::Entry
    where
        O: super::WireOperation + super::InputDeclaration,
        O::Request: crate::Protobuf + crate::MemorySize,
        O::Response: crate::Protobuf,
        <O as super::InputDeclaration>::Item: crate::Protobuf + crate::MemorySize;

    fn download<O>(self) -> Self::Entry
    where
        O: super::WireOperation + super::OutputDeclaration,
        O::Request: crate::Protobuf + crate::MemorySize,
        O::Response: crate::Protobuf,
        <O as super::OutputDeclaration>::Item: crate::Protobuf + crate::MemorySize;

    fn exchange<O>(self) -> Self::Entry
    where
        O: super::WireOperation + super::InputDeclaration + super::OutputDeclaration,
        O::Request: crate::Protobuf + crate::MemorySize,
        O::Response: crate::Protobuf,
        <O as super::InputDeclaration>::Item: crate::Protobuf + crate::MemorySize,
        <O as super::OutputDeclaration>::Item: crate::Protobuf + crate::MemorySize;
}

/* === Implementations === */
impl Grpc {
    pub const fn new(package: &'static str, service: &'static str, method: &'static str) -> Self {
        Self {
            package: Cow::Borrowed(package),
            service: Cow::Borrowed(service),
            method: Cow::Borrowed(method),
        }
    }

    pub fn validate(&self) -> Result<(), super::ContractError> {
        if self.package.is_empty()
            || self.package.len() > 255
            || !self.package.split('.').all(identifier)
            || !identifier(&self.service)
            || !identifier(&self.method)
            || self.package.starts_with("grpc.")
        {
            return Err(super::ContractError::Grpc);
        }
        Ok(())
    }

    pub fn path(&self) -> String {
        format!("/{}.{}/{}", self.package, self.service, self.method)
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    value.len() <= 255
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/* === Tests === */
