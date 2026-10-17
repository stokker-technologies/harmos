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

// Third Party Imports
use serde::{Serialize, de::DeserializeOwned};
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Copy, Debug)]
pub struct SchemaSettings {
    pub preparation_bytes: std::num::NonZeroUsize,
    pub preparation_work: std::num::NonZeroUsize,
}

#[derive(Clone, Copy, Debug)]
pub struct StreamSettings {
    pub items: std::num::NonZeroUsize,
    pub bytes: std::num::NonZeroUsize,
}

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("artifact configuration could not be encoded: {0}")]
    Encode(serde_json::Error),
    #[error("artifact configuration could not be decoded: {0}")]
    Decode(serde_json::Error),
}

/* === Traits === */

/* === Implementations === */
impl Default for SchemaSettings {
    fn default() -> Self {
        Self {
            // Complete Checkpoint Schemas Fit Below 128 KiB; Retain Headroom For Larger Catalogs.
            preparation_bytes: std::num::NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
            preparation_work: std::num::NonZeroUsize::new(1024 * 1024).unwrap(),
        }
    }
}

/* === Public Functions === */
pub fn decode_config<C: DeserializeOwned>(bytes: &[u8]) -> Result<C, ConfigError> {
    serde_json::from_slice(bytes).map_err(ConfigError::Decode)
}

pub fn validate_config(bytes: &[u8]) -> Result<(), ConfigError> {
    decode_config::<serde::de::IgnoredAny>(bytes).map(|_| ())
}

pub fn encode_config<C: Serialize>(config: &C) -> Result<Vec<u8>, ConfigError> {
    serde_json::to_vec(config).map_err(ConfigError::Encode)
}

/* === Private Functions === */

/* === Tests === */
