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
use std::sync::RwLock;

// Third Party Imports
use harmos_operations::execution::operations::ServingError;

// Local Imports
use super::failure;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Redaction {
    secrets: RwLock<Vec<String>>,
    maximum_bytes: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Redaction {
    pub fn new(maximum_bytes: usize) -> Self {
        Self {
            secrets: RwLock::new(Vec::new()),
            maximum_bytes,
        }
    }
    pub fn register(&self, secret: String) -> Result<(), ServingError> {
        if secret.is_empty() {
            return Err(failure("Status.Redaction", "secret is empty"));
        }
        let mut secrets = self.secrets.write().expect("redaction lock is healthy");
        if secrets.contains(&secret) {
            return Ok(());
        }
        if secrets.iter().map(String::len).sum::<usize>().saturating_add(secret.len()) > self.maximum_bytes {
            return Err(failure("Status.Redaction", "secret budget exceeded"));
        }
        secrets.push(secret);
        secrets.sort_by_key(|value| std::cmp::Reverse(value.len()));
        Ok(())
    }
    pub fn text(&self, text: &str) -> String {
        let secrets = self.secrets.read().expect("redaction lock is healthy");
        let mut result = text.to_owned();
        for secret in secrets.iter() {
            result = result.replace(secret, "<redacted>");
        }
        result
    }
    pub fn field(&self, name: &str, value: &str) -> String {
        let name = name.to_ascii_lowercase();
        if ["password", "secret", "token", "credential", "authorization", "private_key", "access_key"]
            .iter()
            .any(|part| name.contains(part))
        {
            return "<redacted>".into();
        }
        self.text(value)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
