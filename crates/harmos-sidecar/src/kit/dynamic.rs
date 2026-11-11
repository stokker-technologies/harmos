/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Runtime-Defined Operations Share The Typed Sidecar Lifecycle And Framed Carrier.

/* === Modules === */

mod artifact;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::sync::Arc;

// Third Party Imports
use serde_json::Value;
use tokio::io::AsyncWriteExt;

// Local Imports
use super::{Opening, Serving, Supervisor};
use harmos_domain::inventory::{Declaration, Facility};
use harmos_domain::model::operations::lifecycle;
use harmos_domain::model::storage::ArrowIpc;
use harmos_domain::operations::{Codec, Contract, FailureKind, OperationDefinition, OperationPolicy, ReplaySafety, Visibility};
use harmos_domain::schema::{Described, graph::Schema, json::JsonCodec};
use harmos_domain::{Failure, Identifier, Message, Refusal, StreamDeclaration};
use harmos_sdk::__private::{ErasedResponse, ServiceFuture, WireTable};
use harmos_sdk::operations::{ExecutionContext, NativeInstance, Response, ServingError};
use harmos_sdk::plugin::{Attachment, Owner, Plugin};

/* === Constants === */

// Build Tooling Exports Inert Metadata Before The Host Ever Launches The Plugin.
pub const ARTIFACT_EXPORT_FLAG: &str = "--harmos-export-artifact-v1";

/* === Macros === */

/* === Type Aliases === */

type Run = dyn Fn(Context, Value) -> Result<Value, Failure> + Send + Sync;

/* === Structs === */

pub struct DynamicPlugin {
    id: String,
    capacity: NonZeroUsize,
    operations: Vec<Arc<Operation>>,
    streams: Vec<StreamDeclaration>,
}

struct Registry {
    operations: Vec<Arc<Operation>>,
    streams: Vec<StreamDeclaration>,
}

struct Operation {
    contract: Contract,
    request: JsonCodec,
    response: JsonCodec,
    run: Arc<Run>,
}

struct RuntimePlugin(Arc<Registry>);

struct Invocation {
    operation: Arc<Operation>,
    registry: Arc<Registry>,
    request: Value,
    worker: Option<tokio::task::JoinHandle<Result<Vec<u8>, ServingError>>>,
    cancellation: Option<harmos_sdk::operations::Cancellation>,
}

#[derive(Clone)]
pub struct Context {
    execution: ExecutionContext,
    host: Supervisor,
    registry: Arc<Registry>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl DynamicPlugin {
    pub fn new(id: &str, capacity: NonZeroUsize) -> std::io::Result<Self> {
        if !harmos_domain::identity::segment(id) {
            return Err(std::io::Error::other("invalid plugin id"));
        }
        Ok(Self {
            id: id.to_owned(),
            capacity,
            operations: Vec::new(),
            streams: Vec::new(),
        })
    }

    pub fn operation(
        &mut self, id: &str, schema: Schema, policy: OperationPolicy,
        run: impl Fn(Context, Value) -> Result<Value, Failure> + Send + Sync + 'static,
    ) -> std::io::Result<()> {
        let definition = OperationDefinition {
            id: Identifier::new(id).map_err(std::io::Error::other)?,
            description: String::new(),
            policy,
            visibility: Visibility::Private,
            replay: ReplaySafety::Unknown,
            fingerprint: harmos_domain::Fingerprint([0; 32]),
        };
        let settings = harmos_domain::config::SchemaSettings::default();
        let contract = Contract::from_schema(definition, Codec::Postcard, schema, &settings).map_err(std::io::Error::other)?;
        if contract.input().is_some() || contract.output().is_some() {
            return Err(std::io::Error::other("only unary operations are supported"));
        }
        if self.operations.iter().any(|operation| operation.contract.operation().id == contract.operation().id) {
            return Err(std::io::Error::other("duplicate operation"));
        }
        let request = JsonCodec::new(contract.request(), &settings).map_err(std::io::Error::other)?;
        let response = JsonCodec::new(contract.response(), &settings).map_err(std::io::Error::other)?;
        self.operations.push(Arc::new(Operation {
            contract,
            request,
            response,
            run: Arc::new(run),
        }));
        Ok(())
    }

    pub fn stream(&mut self, name: &str, window: NonZeroUsize) -> std::io::Result<()> {
        let id = format!("{}.{name}", self.id);
        if self.streams.iter().any(|stream| stream.id == id) {
            return Err(std::io::Error::other("duplicate stream"));
        }
        let metadata = harmos_domain::Metadata::new(Identifier::new(&id).map_err(std::io::Error::other)?, name, None);
        let row = Schema::from_declaration(ArrowIpc::DEFINITION, &Default::default()).map_err(std::io::Error::other)?;
        let stream = StreamDeclaration {
            metadata,
            row_metadata: Vec::new(),
            id,
            window: window.get(),
            row,
        };
        harmos_domain::inventory::publications::validate(&stream).map_err(std::io::Error::other)?;
        self.streams.push(stream);
        Ok(())
    }

    pub async fn serve(mut self) -> std::io::Result<()> {
        let args: Vec<_> = std::env::args_os().skip(1).collect();
        if args.len() == 1 && args[0] == ARTIFACT_EXPORT_FLAG {
            let bytes = artifact::encode(&self)?;
            let mut stdout = tokio::io::stdout();
            stdout.write_all(&bytes).await?;
            return stdout.flush().await;
        }
        if super::catalog::requested(args.into_iter()) {
            let mut stdout = tokio::io::stdout();
            stdout.write_all(b"[]").await?;
            return stdout.flush().await;
        }
        let declaration = Declaration::inspect(&artifact::encode(&self)?, Facility::Sidecar).map_err(std::io::Error::other)?;
        let contracts = declaration.contracts.provides.clone();
        for (operation, contract) in self.operations.iter_mut().zip(&contracts) {
            // The Existing Inert Resolver Owns Canonical Definition Indexing.
            Arc::get_mut(operation).expect("unserved operation has one owner").contract = contract.clone();
        }
        let registry = Arc::new(Registry {
            operations: self.operations,
            streams: self.streams,
        });
        let capacity = self.capacity;
        let opening = Opening {
            serving: None,
            prepare: Box::new(move |assigned_owner| {
                let (owner, table) = Owner::<RuntimePlugin>::new(assigned_owner, contracts.clone(), capacity).map_err(Refusal::of)?;
                table.lock().map_err(Refusal::of)?.provide_resource(Arc::clone(&registry)).map_err(Refusal::of)?;
                harmos_operations::lifecycle::bind(owner.clone(), &table).map_err(Refusal::of)?;
                let interrupted = Arc::clone(&owner);
                Ok(Serving {
                    owner: assigned_owner.to_owned(),
                    operations: table,
                    retirement: Box::new(move || {
                        let owner = Arc::clone(&owner);
                        Box::pin(async move { owner.retire().await.map_err(Refusal::of) })
                    }),
                    quiescence: Box::new(move || {
                        let owner = Arc::clone(&interrupted);
                        Box::pin(async move { owner.quiesce().await.map_err(Refusal::of) })
                    }),
                })
            }),
            catalog: crate::PluginCatalog {
                registrations: Vec::new(),
                operations: declaration.operations,
                observers: Vec::new(),
            },
        };
        #[cfg(any(unix, windows))]
        if let Some(control) = crate::attach::control::inherited()? {
            let opening = Arc::new(tokio::sync::Mutex::new(opening));
            let data = (Box::new(tokio::io::stdin()) as _, Box::new(tokio::io::stdout()) as _);
            return super::connection::run_binary(opening, data, control, std::future::pending()).await;
        }
        Err(std::io::Error::other("serve requires the Harmos host control channel"))
    }
}

impl lifecycle::Policy for RuntimePlugin {
    const INITIALIZATION_POLICY: OperationPolicy = lifecycle::POLICY;
    const SHUTDOWN_POLICY: OperationPolicy = lifecycle::POLICY;
}

impl harmos_sdk::Lifecycle for RuntimePlugin {
    type Config = ();
    type Error = std::io::Error;
    type Context = Supervisor;

    async fn prepare(_: (), setup: &mut harmos_sdk::Setup<Self>) -> std::io::Result<Self> {
        let registry = setup.settings().context().resource::<Registry>().map_err(std::io::Error::other)?;
        Ok(Self(registry))
    }
}

impl Plugin for RuntimePlugin {
    const CATALOGS: &'static [harmos_sdk::operations::Catalog] = &[];
    fn attachment(&self) -> Option<Arc<dyn Attachment<Supervisor>>> {
        Some(self.0.clone())
    }
}

impl Attachment<Supervisor> for Registry {
    fn bind(self: Arc<Self>, table: &mut WireTable) -> Result<(), ServingError> {
        for operation in &self.operations {
            let operation = Arc::clone(operation);
            let registry = Arc::clone(&self);
            table.encoded(operation.contract.clone(), move |payload| {
                let request = operation.request.decode(payload).map_err(ServingError::Codec)?;
                Ok(Box::new(Invocation {
                    operation: Arc::clone(&operation),
                    registry: Arc::clone(&registry),
                    request,
                    worker: None,
                    cancellation: None,
                }))
            })?;
        }
        Ok(())
    }
    fn observers(&self) -> Vec<harmos_domain::inventory::observers::Declaration> {
        Vec::new()
    }
    fn quiesce(&self) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ServingError>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }
}

impl NativeInstance for Invocation {
    fn classify(&self, _: &(dyn std::error::Error + Send + Sync + 'static)) -> FailureKind {
        FailureKind::Permanent
    }
    fn initialize(&mut self, _: ExecutionContext) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn run(&mut self, execution: ExecutionContext) -> ServiceFuture<'_, ErasedResponse> {
        Box::pin(async move {
            if self.worker.is_some() {
                return Err(ServingError::Unsettled);
            }
            let host = execution.resource::<Supervisor>().map_err(|error| ServingError::Native(Arc::new(error)))?;
            self.cancellation = Some(execution.cancellation.clone());
            let context = Context {
                execution,
                host: (*host).clone(),
                registry: Arc::clone(&self.registry),
            };
            let request = self.request.clone();
            let operation = Arc::clone(&self.operation);
            self.worker = Some(tokio::task::spawn_blocking(move || {
                let response = (operation.run)(context, request).map_err(ServingError::Handler)?;
                operation.response.encode(&response).map_err(ServingError::Codec)
            }));
            let result = self.worker.as_mut().expect("owned callback worker").await;
            self.worker = None;
            let result = result.map_err(|_| ServingError::Panicked)?;
            Ok(Response::new(result?).into_erased())
        })
    }
    fn stop(&mut self) -> ServiceFuture<'_, ()> {
        Box::pin(async move {
            if let Some(cancellation) = &self.cancellation {
                cancellation.request();
            }
            if let Some(worker) = self.worker.as_mut() {
                // Dropping An Attempt Future Does Not Detach Its Physical Callback.
                let result = worker.await;
                self.worker = None;
                let _ = result.map_err(|_| ServingError::Panicked)?;
            }
            Ok(())
        })
    }
    fn cleanup(&mut self) -> ServiceFuture<'_, ()> {
        Box::pin(async { Ok(()) })
    }
    fn transferred(&self) -> bool {
        false
    }
    fn progress(&self) -> Vec<tokio::sync::watch::Receiver<Option<tokio::time::Instant>>> {
        Vec::new()
    }
}

impl Context {
    pub fn cancelled(&self) -> bool {
        self.execution.cancellation.is_requested()
    }

    pub fn wait_cancelled(&self) {
        tokio::runtime::Handle::current().block_on(self.execution.cancellation.cancelled());
    }

    pub fn operation(&self, id: &str, request: &Value) -> Result<Value, Failure> {
        let operation = self
            .registry
            .operations
            .iter()
            .find(|operation| operation.contract.operation().id.as_str() == id)
            .ok_or_else(|| Failure::new(FailureKind::Permanent, "operation is not declared"))?;
        let payload = operation.request.encode(request).map_err(permanent)?;
        let bytes = tokio::runtime::Handle::current().block_on(self.host.encoded_call(&operation.contract, payload)).map_err(permanent)?;
        operation.response.decode(&bytes).map_err(permanent)
    }

    #[cfg(feature = "arrow")]
    pub fn publish_arrow(&self, name: &str, batch: arrow_array::RecordBatch) -> Result<(), Failure> {
        let stream = self
            .registry
            .streams
            .iter()
            .find(|stream| stream.id.rsplit_once('.').is_some_and(|(_, local)| local == name))
            .ok_or_else(|| Failure::new(FailureKind::Permanent, "stream is not declared"))?;
        let mut bytes = Vec::new();
        {
            let mut writer = arrow_ipc::writer::StreamWriter::try_new(&mut bytes, batch.schema().as_ref()).map_err(permanent)?;
            writer.write(&batch).map_err(permanent)?;
            writer.finish().map_err(permanent)?;
        }
        let rows = [ArrowIpc::new(bytes).encode()];
        let stamps = [self.host.now()];
        let identity = super::host::StreamIdentity { id: &stream.id, window: stream.window };
        tokio::runtime::Handle::current().block_on(self.host.publication_with(identity, &rows, &stamps, None)).map_err(permanent)
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn permanent(error: impl std::fmt::Display) -> Failure {
    Failure::new(FailureKind::Permanent, error.to_string())
}

/* === Tests === */
