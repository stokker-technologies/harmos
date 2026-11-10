/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Rust Plugin SDK

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};

// Third Party Imports
use futures_util::stream;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::mpsc,
};
use tonic::{Streaming, transport::Channel};

// Local Imports
use harmos_protocol::v1::{self as v1, child_response, host_frame, plugin_client::PluginClient, plugin_frame};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Process {
    pub client: PluginClient<Channel>,
    child: Child,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Process {
    pub async fn start(cleanup: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let path = binary("harmos-sdk-diagnostics-plugin", env!("CARGO_BIN_EXE_harmos-sdk-diagnostics-plugin"));
        let mut child = Command::new(path)
            .arg(cleanup)
            .env("HARMOS_ADDRESS", "127.0.0.1:0")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let stdout = child.stdout.take().ok_or("Missing Plugin Standard Output")?;
        let line = BufReader::new(stdout).lines().next_line().await?.ok_or("Missing Readiness Line")?;
        let address = line.strip_prefix("harmos listening ").ok_or("Invalid Readiness Line")?;
        let client = PluginClient::connect(format!("http://{address}")).await?;
        Ok(Self { client, child })
    }

    pub async fn close(mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.child.kill().await?;
        self.child.wait().await?;
        Ok(())
    }
}

/* === Public Functions === */

pub fn plugin() -> PathBuf {
    binary("harmos-sdk-conformance-plugin", env!("CARGO_BIN_EXE_harmos-sdk-conformance-plugin"))
}

pub fn binary(name: &str, fallback: &str) -> PathBuf {
    std::env::var_os(format!("NEXTEST_BIN_EXE_{name}"))
        .or_else(|| std::env::var_os(format!("CARGO_BIN_EXE_{name}")))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(fallback))
}

pub async fn describe(client: &mut PluginClient<Channel>) -> Result<v1::Description, tonic::Status> {
    Ok(client.describe(v1::DescribeRequest { revision: 1, ..Default::default() }).await?.into_inner())
}

pub async fn invoke(
    client: &mut PluginClient<Channel>, operation: &str,
) -> Result<(mpsc::Sender<v1::HostFrame>, Streaming<v1::PluginFrame>), Box<dyn std::error::Error>> {
    let (send, receive) = mpsc::channel(1);
    send.send(v1::HostFrame {
        frame: Some(host_frame::Frame::Request(v1::Request {
            operation: operation.into(),
            invocation: "execution-story".into(),
            attempt: 1,
            scope: "scope".into(),
            payload: Vec::new(),
        })),
    })
    .await?;
    let input = stream::unfold(receive, |mut receive| async { receive.recv().await.map(|frame| (frame, receive)) });
    let output = client.invoke(input).await?.into_inner();
    Ok((send, output))
}

pub async fn child(input: &mut Streaming<v1::PluginFrame>) -> Result<v1::ChildRequest, Box<dyn std::error::Error>> {
    match input.message().await?.and_then(|frame| frame.frame) {
        Some(plugin_frame::Frame::Child(child)) => Ok(child),
        _ => Err("Expected Host Child Call".into()),
    }
}

pub async fn reply<T: prost::Message>(send: &mpsc::Sender<v1::HostFrame>, call: u64, reply: T) -> Result<(), Box<dyn std::error::Error>> {
    send.send(v1::HostFrame {
        frame: Some(host_frame::Frame::Child(v1::ChildResponse {
            call,
            result: Some(child_response::Result::Payload(reply.encode_to_vec())),
        })),
    })
    .await?;
    Ok(())
}

pub async fn terminal<T: prost::Message + Default>(input: &mut Streaming<v1::PluginFrame>) -> Result<T, Box<dyn std::error::Error>> {
    match input.message().await?.and_then(|frame| frame.frame) {
        Some(plugin_frame::Frame::Response(v1::Response {
            result: Some(v1::response::Result::Payload(payload)),
        })) => Ok(T::decode(payload.as_slice())?),
        _ => Err("Expected Successful Terminal Response".into()),
    }
}

/* === Private Functions === */

/* === Tests === */
