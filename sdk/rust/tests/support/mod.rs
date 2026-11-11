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
    events: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Process {
    pub async fn start(cleanup: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let path = binary("harmos-sdk-diagnostics-plugin");
        let mut child = Command::new(path)
            .arg(cleanup)
            .env("HARMOS_ADDRESS", "127.0.0.1:0")
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let stdout = child.stdout.take().ok_or("Missing Plugin Standard Output")?;
        let mut events = BufReader::new(stdout).lines();
        let line = events.next_line().await?.ok_or("Missing Readiness Line")?;
        let address = line.strip_prefix("harmos listening ").ok_or("Invalid Readiness Line")?;
        let client = PluginClient::connect(format!("http://{address}")).await?;
        Ok(Self { client, child, events })
    }

    pub async fn event(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        self.events.next_line().await?.ok_or_else(|| "Plugin Closed Before Event".into())
    }

    pub async fn close(mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.child.kill().await?;
        self.child.wait().await?;
        Ok(())
    }
}

/* === Public Functions === */

pub fn plugin() -> PathBuf {
    binary("harmos-sdk-conformance-plugin")
}

pub fn binary(name: &str) -> PathBuf {
    let extension = std::env::consts::EXE_SUFFIX;
    if let Ok(profile) = std::env::var("STKR_TEST_PROFILE") {
        let target = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let path = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_else(|| env!("CARGO_MANIFEST_DIR").into()))
            .join("../../target/build")
            .join(profile)
            .join(target)
            .join("fixtures/bin")
            .join(format!("{name}{extension}"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&path).expect("transferred fixture executable").permissions();
            permissions.set_mode(permissions.mode() | 0o100);
            std::fs::set_permissions(&path, permissions).unwrap();
        }
        return path;
    }
    std::env::var_os(format!("NEXTEST_BIN_EXE_{}", name.replace('-', "_")))
        .or_else(|| std::env::var_os(format!("CARGO_BIN_EXE_{name}")))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_else(|| env!("CARGO_MANIFEST_DIR").into())).join("../../target/debug").join(format!("{name}{extension}")))
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

pub async fn publish(
    client: &mut PluginClient<Channel>, id: &str,
) -> Result<(mpsc::Sender<v1::PublishFrame>, Streaming<v1::Samples>), Box<dyn std::error::Error>> {
    let (send, receive) = mpsc::channel(1);
    send.send(v1::PublishFrame {
        frame: Some(v1::publish_frame::Frame::Open(v1::PublishRequest { stream: id.into(), scope: "scope".into() })),
    })
    .await?;
    let input = stream::unfold(receive, |mut receive| async { receive.recv().await.map(|frame| (frame, receive)) });
    let output = client.publish(input).await?.into_inner();
    Ok((send, output))
}

/* === Tests === */
