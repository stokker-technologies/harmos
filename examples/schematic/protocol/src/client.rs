// @generated from schematic protocol descriptors; do not edit.
use harmos::client::{CallOptions, ClientTransport, action_error};
use prost_reflect::ReflectMessage;
use tonic::{Status, codegen::http::uri::PathAndQuery};
#[derive(Clone)]
pub struct Client {
    pub schematic: Schematic,
}
impl Client {
    pub fn new(transport: ClientTransport) -> Self {
        Self {
            schematic: Schematic { transport },
        }
    }
}
#[derive(Clone)]
pub struct Schematic {
    transport: ClientTransport,
}
impl Schematic {
    pub async fn validate(&self, schematic: Snapshot) -> Result<Vec<String>, Status> {
        let request = CallOptions::default().request(SchematicValidateRequest {
            schematic: Some(schematic),
        });
        let response = self
            .transport
            .unary::<SchematicValidateRequest, SchematicValidateResponse>(
                PathAndQuery::from_static("/schematic.host.v1.Schematic/Validate"),
                request,
                SchematicValidateResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("schematic.validate", error))?;
        Ok(response.into_inner().diagnostics)
    }
}
impl Schematic {
    pub async fn propose_route(&self, from: Point, to: Point) -> Result<Vec<Point>, Status> {
        let request = CallOptions::default().request(SchematicProposeRouteRequest {
            from: Some(from),
            to: Some(to),
        });
        let response = self
            .transport
            .unary::<SchematicProposeRouteRequest, SchematicProposeRouteResponse>(
                PathAndQuery::from_static("/schematic.host.v1.Schematic/ProposeRoute"),
                request,
                SchematicProposeRouteResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("schematic.propose_route", error))?;
        Ok(response.into_inner().points)
    }
}
impl Schematic {
    pub async fn list_directories(&self) -> Result<Vec<String>, Status> {
        let request = CallOptions::default().request(SchematicListDirectoriesRequest {});
        let response = self
            .transport
            .unary::<SchematicListDirectoriesRequest, SchematicListDirectoriesResponse>(
                PathAndQuery::from_static("/schematic.host.v1.Schematic/ListDirectories"),
                request,
                SchematicListDirectoriesResponse::default().descriptor(),
            )
            .await
            .map_err(|error| action_error("schematic.list_directories", error))?;
        Ok(response.into_inner().directories)
    }
}
use crate::{
    Point, SchematicListDirectoriesRequest, SchematicListDirectoriesResponse,
    SchematicProposeRouteRequest, SchematicProposeRouteResponse, SchematicValidateRequest,
    SchematicValidateResponse, Snapshot,
};
