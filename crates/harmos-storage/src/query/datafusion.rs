/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Spike: Bounded Single-Partition Plans Over Catalog-Selected IPC Streams

/* === Modules === */

mod quantiles;

/* === Re-Exports === */

pub use ::datafusion as engine;
pub use quantiles::Quantiles;

/* === Imports === */

// Standard Library Imports
use std::{fmt, sync::Arc};

// Third Party Imports
use arrow_schema::SchemaRef;
use datafusion::{
    catalog::{Session, TableProvider},
    error::Result,
    execution::{
        TaskContext,
        context::{SessionConfig, SessionContext},
        memory_pool::GreedyMemoryPool,
        runtime_env::RuntimeEnvBuilder,
    },
    logical_expr::{Expr, TableType},
    physical_plan::{
        ExecutionPlan, SendableRecordBatchStream,
        streaming::{PartitionStream, StreamingTableExec},
    },
};
use futures_util::future::BoxFuture;

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

pub type Open = Arc<dyn Fn() -> SendableRecordBatchStream + Send + Sync>;

/* === Structs === */

pub struct IpcTable {
    schema: SchemaRef,
    open: Open,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl IpcTable {
    pub fn new(schema: SchemaRef, open: Open) -> Self {
        Self { schema, open }
    }
}

impl fmt::Debug for IpcTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IpcTable").field("schema", &self.schema).finish_non_exhaustive()
    }
}

impl TableProvider for IpcTable {
    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }
    fn table_type(&self) -> TableType {
        TableType::Base
    }
    fn scan<'life0, 'life1, 'life2, 'life3, 'async_trait>(
        &'life0 self, _state: &'life1 dyn Session, projection: Option<&'life2 Vec<usize>>, _filters: &'life3 [Expr], limit: Option<usize>,
    ) -> BoxFuture<'async_trait, Result<Arc<dyn ExecutionPlan>>>
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        'life2: 'async_trait,
        'life3: 'async_trait,
        Self: 'async_trait,
    {
        let partition: Arc<dyn PartitionStream> = Arc::new(Self {
            schema: self.schema.clone(),
            open: self.open.clone(),
        });
        let result = StreamingTableExec::try_new(self.schema.clone(), vec![partition], projection, [], false, limit)
            .map(|plan| Arc::new(plan) as Arc<dyn ExecutionPlan>);
        Box::pin(async move { result })
    }
}

impl PartitionStream for IpcTable {
    fn schema(&self) -> &SchemaRef {
        &self.schema
    }
    fn execute(&self, _context: Arc<TaskContext>) -> SendableRecordBatchStream {
        (self.open)()
    }
}

/* === Public Functions === */

pub fn context(memory_bytes: usize) -> Result<SessionContext> {
    let runtime = RuntimeEnvBuilder::new().with_memory_pool(Arc::new(GreedyMemoryPool::new(memory_bytes))).build_arc()?;
    let mut config = SessionConfig::new().with_target_partitions(1).with_batch_size(1024);
    config.options_mut().optimizer.repartition_aggregations = false;
    config.options_mut().execution.sort_spill_reservation_bytes = memory_bytes / 8;
    Ok(SessionContext::new_with_config_rt(config, runtime))
}

/* === Private Functions === */

/* === Tests === */
