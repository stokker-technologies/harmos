/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Generated Client Methods

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{marker::PhantomData, sync::Arc};

// Third Party Imports
use prost::Message;
use prost_reflect::ReflectMessage;
use tonic::{Status, codegen::http::uri::PathAndQuery};

// Local Imports
use harmos_protocol::{schema, v1 as host};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Signature<Q, R, I, O> = fn(Q, I) -> (R, O);

/* === Structs === */

/// One Generated Application Contract, With Typed Request, Result And Stream Items.
pub struct Method<Q, R, I = NoItems, O = NoItems> {
    pub(super) contract: Arc<host::DiscoveredContract>,
    pub(super) path: PathAndQuery,
    pub(super) input: bool,
    pub(super) output: bool,
    marker: PhantomData<Signature<Q, R, I, O>>,
}

/// A Method Direction With No Item Stream.
#[derive(Clone, PartialEq, prost::Message)]
pub struct NoItems {}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl<Q, R, I, O> Clone for Method<Q, R, I, O> {
    fn clone(&self) -> Self {
        Self {
            contract: self.contract.clone(),
            path: self.path.clone(),
            input: self.input,
            output: self.output,
            marker: PhantomData,
        }
    }
}

impl<Q: ReflectMessage + Default, R: ReflectMessage + Default> Method<Q, R> {
    /// Bind Generated Types To An Authoritative Discovered Or Application Contract.
    pub fn new(contract: host::DiscoveredContract) -> Result<Self, Status> {
        if contract.visibility != host::Visibility::Public as i32 {
            return Err(Status::permission_denied("operation is private"));
        }
        let protobuf = contract.protobuf.as_ref().ok_or_else(|| Status::invalid_argument("operation has no generated contract"))?;
        verify::<Q>(&protobuf.request, protobuf)?;
        verify::<R>(&protobuf.response, protobuf)?;
        let fingerprint = schema::host_fingerprint(&contract.id, protobuf).map_err(invalid)?;
        if fingerprint != contract.fingerprint {
            return Err(Status::invalid_argument("operation fingerprint differs from descriptors"));
        }
        let grpc = contract.grpc.as_ref().ok_or_else(|| Status::invalid_argument("operation has no gRPC mapping"))?;
        if !grpc.package.split('.').all(identifier) || !identifier(&grpc.service) || !identifier(&grpc.method) {
            return Err(Status::invalid_argument("invalid gRPC mapping"));
        }
        let path = format!("/{}.{}/{}", grpc.package, grpc.service, grpc.method).parse().map_err(invalid)?;
        Ok(Self {
            contract: Arc::new(contract),
            path,
            input: false,
            output: false,
            marker: PhantomData,
        })
    }
}

impl<Q, R, I, O> Method<Q, R, I, O> {
    pub fn contract(&self) -> &host::DiscoveredContract {
        &self.contract
    }

    pub fn with_input<T: ReflectMessage + Default>(self) -> Result<Method<Q, R, T, O>, Status> {
        let protobuf = self.contract.protobuf.as_ref().expect("validated contract");
        let name = protobuf.input.as_deref().ok_or_else(|| Status::invalid_argument("operation has no input stream"))?;
        verify::<T>(name, protobuf)?;
        Ok(Method {
            contract: self.contract,
            path: self.path,
            input: true,
            output: self.output,
            marker: PhantomData,
        })
    }

    pub fn with_output<T: ReflectMessage + Default>(self) -> Result<Method<Q, R, I, T>, Status> {
        let protobuf = self.contract.protobuf.as_ref().expect("validated contract");
        let name = protobuf.output.as_deref().ok_or_else(|| Status::invalid_argument("operation has no output stream"))?;
        verify::<T>(name, protobuf)?;
        Ok(Method {
            contract: self.contract,
            path: self.path,
            input: self.input,
            output: true,
            marker: PhantomData,
        })
    }

    pub(super) fn validate(&self) -> Result<(), Status> {
        let protobuf = self.contract.protobuf.as_ref().expect("validated contract");
        if self.input != protobuf.input.is_some() || self.output != protobuf.output.is_some() {
            return Err(Status::invalid_argument("bind every declared input and output stream before calling"));
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn verify<T: ReflectMessage + Default>(name: &str, contract: &host::Protobuf) -> Result<(), Status> {
    let descriptor = T::default().descriptor();
    if descriptor.full_name() != name {
        return Err(Status::invalid_argument("generated message name differs from operation contract"));
    }
    let expected = prost_types::FileDescriptorSet::decode(contract.descriptors.as_slice()).map_err(invalid)?;
    let actual = prost_types::FileDescriptorSet {
        file: descriptor.parent_pool().files().map(|file| file.file_descriptor_proto().clone()).collect(),
    };
    let expected = schema::message_types(&expected, name).map_err(invalid)?;
    let actual = schema::message_types(&actual, name).map_err(invalid)?;
    if expected != actual {
        return Err(Status::invalid_argument("generated message schema differs from operation contract"));
    }
    Ok(())
}

fn identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn invalid(error: impl std::fmt::Display) -> Status {
    Status::invalid_argument(error.to_string())
}

/* === Tests === */
