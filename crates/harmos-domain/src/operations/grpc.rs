/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grpc {
    pub package: Cow<'static, str>,
    pub service: Cow<'static, str>,
    pub method: Cow<'static, str>,
}

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

fn identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    value.len() <= 255
        && bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
