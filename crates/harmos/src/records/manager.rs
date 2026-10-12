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
use std::sync::Arc;
// Third Party Imports
use tokio::sync::Semaphore;
// Local Imports
use super::{Definition, Registry};
use crate::state::dispatch::Dispatch;
use crate::storage::SchemaKey;
use crate::{
    Application, Error, Position, Prepared, PrincipalId, Receipt, Recorded, Result, StateManager,
    SystemFact,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct RecordManager<A: Application> {
    pub(super) state: StateManager<A>,
    pub(super) catalog: Arc<Dispatch<A, crate::Payload<A>>>,
    preparing: Arc<Semaphore>,
    pub(super) schemas: Arc<Registry>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<A: Application> RecordManager<A> {
    pub fn new(state: StateManager<A>) -> Self {
        let (catalog, preparing) = state.record_resources();
        let schemas = state.record_schemas();
        Self {
            state,
            catalog,
            preparing,
            schemas,
        }
    }

    pub async fn register(
        &self,
        owner: &str,
        schema: &SchemaKey,
        fields: &[crate::schema::Field],
    ) -> Result<Arc<Definition>> {
        let _preparation = Arc::clone(&self.preparing)
            .acquire_owned()
            .await
            .expect("record preparation admission remains owned by the original state");
        let pending = self.schemas.reserve(owner, schema, fields)?;
        let id = crate::DefinitionId::new(schema.id.as_str());
        if !matches!(
            self.catalog.has_record(&id, schema.version.get()),
            Ok(false)
        ) {
            return Err(Error::AlreadyRegistered {
                domain: "native record schema",
                name: schema.id.as_str().to_owned(),
            });
        }
        if !pending.is_committed()
            && let Some(storage) = &self.schemas.storage
        {
            storage
                .write_schema(pending.definition.schema(), pending.definition.fields())
                .await?;
        }
        Ok(pending.commit())
    }

    pub fn definition(&self, schema: &SchemaKey) -> Option<Arc<Definition>> {
        self.schemas.definition(schema)
    }

    pub fn definitions(&self) -> Vec<Arc<Definition>> {
        self.schemas.definitions()
    }

    pub async fn publish<R: Into<A::Record>>(&self, record: R) -> Result<Receipt> {
        let record = record.into();
        let preparation = Arc::clone(&self.preparing)
            .acquire_owned()
            .await
            .expect("record preparation admission remains owned by the original state");
        let prepared = Prepared::typed_record(&record).map_err(Error::Preparation)?;
        let declared = self
            .catalog
            .has_record(&prepared.id, prepared.version)
            .map_err(|source| Error::RecordDeclaration { source })?;
        if !declared {
            return Err(Error::RecordDeclaration {
                source: crate::DecodeError::UnknownDefinition { id: prepared.id },
            });
        }
        self.state
            .append_prepared(Recorded::Record(record), prepared, preparation)
            .await
    }

    pub async fn publish_system(&self, fact: SystemFact) -> Result<Receipt> {
        let preparation = Arc::clone(&self.preparing)
            .acquire_owned()
            .await
            .expect("record preparation admission remains owned by the original state");
        let prepared = Prepared::system(&fact).map_err(Error::Preparation)?;
        self.state
            .append_prepared(Recorded::System(fact), prepared, preparation)
            .await
    }

    pub fn as_principal(&self, principal: PrincipalId) -> Self {
        Self::new(self.state.as_principal(principal))
    }

    pub fn caused_by(&self, cause: Position) -> Self {
        Self::new(self.state.caused_by(cause))
    }
}

impl<A: Application> Clone for RecordManager<A> {
    fn clone(&self) -> Self {
        Self::new(self.state.clone())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
