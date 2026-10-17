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
use heck::ToUpperCamelCase;
use prost_reflect::ServiceDescriptor;

// Local Imports
use super::{DescriptorError, descriptor};
use crate::operations::Contract;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
pub struct Method<'a> {
    pub name: &'a str,
    pub contract: &'a Contract,
    pub declaration: &'a str,
}

pub struct GeneratedService {
    pub descriptor: ServiceDescriptor,
    pub source: String,
}

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum GenerateError {
    #[error(transparent)]
    Descriptor(#[from] DescriptorError),
    #[error("expected a named Rust path, received `{0}`")]
    Path(String),
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn generate_server(
    package: &str,
    service: &str,
    harmos_path: &str,
    methods: &[Method<'_>],
) -> Result<GeneratedService, GenerateError> {
    path(harmos_path)?;
    for method in methods {
        path(method.declaration)?;
    }
    let declarations: Vec<_> = methods
        .iter()
        .map(|method| (method.name, method.contract))
        .collect();
    let descriptor = descriptor(package, service, &declarations)?;
    let name = format!("{}Server", service.to_upper_camel_case());
    let identity = format!("{package}.{service}");
    let mut routes = String::new();
    for method in methods {
        let handler = match (
            method.contract.input().is_some(),
            method.contract.output().is_some(),
        ) {
            (false, false) => "unary",
            (true, false) => "upload",
            (false, true) => "download",
            (true, true) => "exchange",
        };
        let uri = format!("/{identity}/{}", method.name);
        routes.push_str(&format!("{uri:?} => {harmos_path}::grpc::server::{handler}::<{}, {harmos_path}::grpc::Transcoded>(authority, settings, request).await,\n", method.declaration));
    }
    let source = format!(
        r#"
#[derive(Clone)]
pub struct {name} {{
    calls: {harmos_path}::grpc::server::Calls,
    authority: {harmos_path}::grpc::server::Authority,
    settings: {harmos_path}::grpc::server::Settings,
}}
impl {name} {{
    pub fn new(authority: {harmos_path}::grpc::server::Authority, settings: {harmos_path}::grpc::server::Settings) -> ::core::result::Result<Self, {harmos_path}::grpc::__private::tonic::Status> {{
        settings.validate()?;
        let calls = {harmos_path}::grpc::server::Calls::new(settings.calls)?;
        ::core::result::Result::Ok(Self {{ calls, authority, settings }})
    }}
}}
impl {harmos_path}::grpc::__private::tonic::server::NamedService for {name} {{
    const NAME: &'static str = {identity:?};
}}
impl {harmos_path}::grpc::__private::tonic::codegen::Service<
    {harmos_path}::grpc::__private::tonic::codegen::http::Request<{harmos_path}::grpc::__private::tonic::body::Body>
> for {name} {{
    type Response = {harmos_path}::grpc::__private::tonic::codegen::http::Response<{harmos_path}::grpc::__private::tonic::body::Body>;
    type Error = ::core::convert::Infallible;
    type Future = {harmos_path}::grpc::__private::tonic::codegen::BoxFuture<Self::Response, Self::Error>;
    fn poll_ready(&mut self, _: &mut ::core::task::Context<'_>) -> ::core::task::Poll<::core::result::Result<(), Self::Error>> {{
        ::core::task::Poll::Ready(::core::result::Result::Ok(()))
    }}
    fn call(&mut self, request: {harmos_path}::grpc::__private::tonic::codegen::http::Request<{harmos_path}::grpc::__private::tonic::body::Body>) -> Self::Future {{
        let authority = self.authority.clone();
        let settings = self.settings;
        let response = self.calls.run(::std::boxed::Box::pin(async move {{
            match request.uri().path() {{
                {routes}
                _ => {harmos_path}::grpc::__private::tonic::Status::unimplemented("unknown operation method").into_http(),
            }}
        }}));
        ::std::boxed::Box::pin(async move {{ ::core::result::Result::Ok(response.await) }})
    }}
}}
"#
    );
    Ok(GeneratedService { descriptor, source })
}

/* === Private Functions === */
fn path(value: &str) -> Result<(), GenerateError> {
    let segments = value.strip_prefix("::").unwrap_or(value);
    let valid = !segments.is_empty()
        && segments.split("::").all(|segment| {
            let segment = segment.strip_prefix("r#").unwrap_or(segment);
            let mut chars = segment.chars();
            segment != "_"
                && chars
                    .next()
                    .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
                && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
        });
    if valid {
        Ok(())
    } else {
        Err(GenerateError::Path(value.into()))
    }
}

/* === Tests === */
