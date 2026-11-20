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
use std::error::Error as StdError;
use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    process::Stdio,
};

// Third Party Imports
use futures_util::stream;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::TcpListener,
    process::{Child, Command},
    sync::{mpsc, oneshot},
};
use tonic::{
    Streaming,
    transport::{Channel, Server},
};

// Local Imports
use harmos_protocol::plugin::ChildRequest;
use harmos_protocol::plugin::ChildResponse;
use harmos_protocol::plugin::Request;
use harmos_protocol::plugin::Response;
use harmos_protocol::plugin::SamplesSelection;
use harmos_protocol::plugin::child_response;
use harmos_protocol::plugin::instance_samples_request;
use harmos_protocol::plugin::launches_server::{self, LaunchesServer};
use harmos_protocol::plugin::{LaunchesAnnounceRequest, LaunchesAnnounceResponse};
use harmos_protocol::plugin::response;
use harmos_protocol::plugin::{
    InstanceGetRequest, InstanceGetResponse, InstanceInvokeRequest, InstanceInvokeResponse, InstanceSamplesRequest, InstanceSamplesResponse,
};
use prost::Message as ProstMessage;
use tonic::Status;

use harmos_protocol::plugin::{instance_client::InstanceClient, instance_invoke_request, instance_invoke_response};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Process {
    pub client: InstanceClient<Channel>,
    child: Child,
    events: tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    _bootstrap: Bootstrap,
}

/// A Minimal Host Bootstrap Listener: Serves `Launches/Announce` on Loopback and Records Each Call.
pub struct Bootstrap {
    endpoint: SocketAddr,
    announces: mpsc::UnboundedReceiver<LaunchesAnnounceRequest>,
    _shutdown: oneshot::Sender<()>,
}

struct Launches {
    announces: mpsc::UnboundedSender<LaunchesAnnounceRequest>,
    refusal: Option<&'static str>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Process {
    pub async fn start(cleanup: &Path) -> Result<Self, Box<dyn StdError>> {
        let mut bootstrap = Bootstrap::start(None).await?;
        let mut child = bootstrap
            .command("harmos-sdk-diagnostics-plugin")
            .arg(cleanup)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()?;
        let stdout = child.stdout.take().ok_or("Missing Plugin Standard Output")?;
        let events = BufReader::new(stdout).lines();
        let announce = bootstrap.announced().await?;
        let client = InstanceClient::connect(format!("http://{}", announce.address)).await?;
        Ok(Self {
            client,
            child,
            events,
            _bootstrap: bootstrap,
        })
    }

    pub async fn event(&mut self) -> Result<String, Box<dyn StdError>> {
        self.events.next_line().await?.ok_or_else(|| "Plugin Closed Before Event".into())
    }

    pub async fn close(mut self) -> Result<(), Box<dyn StdError>> {
        self.child.kill().await?;
        self.child.wait().await?;
        Ok(())
    }
}

impl Bootstrap {
    pub const TOKEN: &str = "story-launch-token";

    /// Serve Announce on a Loopback Port; a Refusal Answers Every Call With `UNAUTHENTICATED`.
    pub async fn start(refusal: Option<&'static str>) -> Result<Self, Box<dyn StdError>> {
        let socket = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = socket.local_addr()?;
        let (record, announces) = mpsc::unbounded_channel();
        let (shutdown, stopped) = oneshot::channel::<()>();
        let incoming = stream::unfold(socket, |socket| async {
            let accepted = socket.accept().await.map(|(connection, _)| connection);
            Some((accepted, socket))
        });
        let service = LaunchesServer::new(Launches { announces: record, refusal });
        tokio::spawn(Server::builder().add_service(service).serve_with_incoming_shutdown(incoming, async {
            let _ = stopped.await;
        }));
        Ok(Self {
            endpoint,
            announces,
            _shutdown: shutdown,
        })
    }

    /// A Plugin Command Carrying Both Launch Variables.
    pub fn command(&self, name: &str) -> Command {
        let mut command = Command::new(binary(name));
        command.env("HARMOS_HOST_ENDPOINT", self.endpoint.to_string()).env("HARMOS_LAUNCH_TOKEN", Self::TOKEN);
        command
    }

    pub async fn announced(&mut self) -> Result<LaunchesAnnounceRequest, Box<dyn StdError>> {
        self.announces.recv().await.ok_or_else(|| "Bootstrap Closed Before Announce".into())
    }

    /// Announces Received So Far Without Waiting.
    pub fn pending(&mut self) -> usize {
        let mut count = 0;
        while self.announces.try_recv().is_ok() {
            count += 1;
        }
        count
    }
}

#[tonic::async_trait]
impl launches_server::Launches for Launches {
    async fn announce(
        &self, request: tonic::Request<LaunchesAnnounceRequest>,
    ) -> Result<tonic::Response<LaunchesAnnounceResponse>, Status> {
        let _ = self.announces.send(request.into_inner());
        match self.refusal {
            Some(message) => Err(Status::unauthenticated(message)),
            None => Ok(tonic::Response::new(LaunchesAnnounceResponse {})),
        }
    }
}

/* === Public Functions === */

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
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap_or_else(|| env!("CARGO_MANIFEST_DIR").into()))
                .join("../../target/debug")
                .join(format!("{name}{extension}"))
        })
}

pub async fn describe(client: &mut InstanceClient<Channel>) -> Result<InstanceGetResponse, Status> {
    Ok(client.get(InstanceGetRequest { revision: 1, ..Default::default() }).await?.into_inner())
}

pub async fn invoke(
    client: &mut InstanceClient<Channel>, operation: &str,
) -> Result<(mpsc::Sender<InstanceInvokeRequest>, Streaming<InstanceInvokeResponse>), Box<dyn StdError>> {
    let (send, receive) = mpsc::channel(1);
    send.send(InstanceInvokeRequest {
        frame: Some(instance_invoke_request::Frame::Request(Request {
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

pub async fn child(input: &mut Streaming<InstanceInvokeResponse>) -> Result<ChildRequest, Box<dyn StdError>> {
    match input.message().await?.and_then(|frame| frame.frame) {
        Some(instance_invoke_response::Frame::Child(child)) => Ok(child),
        _ => Err("Expected Host Child Call".into()),
    }
}

pub async fn reply<T: ProstMessage>(send: &mpsc::Sender<InstanceInvokeRequest>, call: u64, reply: T) -> Result<(), Box<dyn StdError>> {
    send.send(InstanceInvokeRequest {
        frame: Some(instance_invoke_request::Frame::Child(ChildResponse {
            call,
            result: Some(child_response::Result::Payload(reply.encode_to_vec())),
        })),
    })
    .await?;
    Ok(())
}

pub async fn terminal<T: ProstMessage + Default>(input: &mut Streaming<InstanceInvokeResponse>) -> Result<T, Box<dyn StdError>> {
    match input.message().await?.and_then(|frame| frame.frame) {
        Some(instance_invoke_response::Frame::Response(Response {
            result: Some(response::Result::Payload(payload)),
        })) => Ok(T::decode(payload.as_slice())?),
        _ => Err("Expected Successful Terminal Response".into()),
    }
}

/* === Private Functions === */

pub async fn publish(
    client: &mut InstanceClient<Channel>, id: &str,
) -> Result<(mpsc::Sender<InstanceSamplesRequest>, Streaming<InstanceSamplesResponse>), Box<dyn StdError>> {
    let (send, receive) = mpsc::channel(1);
    send.send(InstanceSamplesRequest {
        frame: Some(instance_samples_request::Frame::Open(SamplesSelection { stream: id.into(), scope: "scope".into() })),
    })
    .await?;
    let input = stream::unfold(receive, |mut receive| async { receive.recv().await.map(|frame| (frame, receive)) });
    let output = client.samples(input).await?.into_inner();
    Ok((send, output))
}

/* === Tests === */
