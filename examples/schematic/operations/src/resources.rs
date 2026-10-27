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

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::Error;
use schematic_domain::model::schematic::progress::AnalysisProgress;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Publication = Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>;
pub type Publisher = Arc<dyn Fn(harmos::OperationContext, AnalysisProgress) -> Publication + Send + Sync>;
pub type HashRequest = <harmos::domain::operations::storage::HashFile as harmos::OperationDeclaration>::Request;
pub type HashResult = Pin<Box<dyn Future<Output = Result<harmos::domain::model::storage::FileHash, Error>> + Send>>;
pub type Hasher = Arc<dyn Fn(harmos::OperationContext, HashRequest) -> HashResult + Send + Sync>;

/* === Structs === */

pub struct AnalysisResources {
    pub(crate) analysis: tokio::sync::Semaphore,
    pub publisher: Publisher,
}

pub struct HashProvider {
    pub hasher: Hasher,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl AnalysisResources {
    pub fn new(publisher: Publisher) -> Self {
        Self { analysis: tokio::sync::Semaphore::new(1), publisher }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
