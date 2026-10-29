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
use std::collections::BTreeMap;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
// Third Party Imports
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, watch};
// Local Imports
use super::credit::{MINIMUM, WINDOW};
use super::{Consumption, Credit, Packet, ReceiveCredit, Reservation, Role, read, read_data, write, write_with};
use crate::Frame;

/* === Constants === */

const OUTGOING_DATA: usize = 8;
const CONTROL: usize = 4;
const EARLY_CANCELLATIONS: usize = 256;
const INCOMING_DATA: usize = WINDOW / MINIMUM + 1;

/* === Macros === */

/* === Type Aliases === */

pub type Io = (Box<dyn AsyncRead + Send + Unpin>, Box<dyn AsyncWrite + Send + Unpin>);
type Outcome = Result<(), Arc<io::Error>>;
type IoReader = Box<dyn AsyncRead + Send + Unpin>;
type IoWriter = Box<dyn AsyncWrite + Send + Unpin>;
type DriverFuture = Pin<Box<dyn Future<Output = Outcome> + Send>>;

/* === Structs === */

pub struct Connection {
    pub sender: Sender,
    pub receiver: Receiver,
    pub driver: Driver,
}

#[derive(Clone)]
pub struct Sender {
    data: mpsc::Sender<Outgoing>,
    control: mpsc::Sender<Packet>,
    state: Arc<State>,
}

pub struct Receiver {
    delivered: u64,
    cancellations: BTreeMap<u64, u64>,
    data: mpsc::Receiver<Received>,
    control: mpsc::Receiver<Received>,
    state: Arc<State>,
}

pub struct Received {
    pub cancelled: bool,
    preceding: Option<u64>,
    pub frame: Frame,
    pub consumption: Option<Consumption>,
}

pub struct Driver {
    work: DriverFuture,
    result: Option<Outcome>,
    state: Arc<State>,
}

struct Outgoing {
    packet: Packet,
    reservation: Reservation,
}

struct State {
    queued: Mutex<u64>,
    written: watch::Sender<u64>,
    sending: Arc<Credit>,
    receiving: Arc<ReceiveCredit>,
    stopped: watch::Sender<bool>,
    failure: Mutex<Option<Arc<io::Error>>>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Connection {
    pub fn new(data: Io, control: Io) -> Self {
        let state = Arc::new(State {
            queued: Mutex::new(0),
            written: watch::channel(0).0,
            sending: Credit::new(),
            receiving: ReceiveCredit::new(),
            stopped: watch::channel(false).0,
            failure: Mutex::new(None),
        });
        let (data_tx, data_rx) = mpsc::channel(OUTGOING_DATA);
        let (control_tx, control_rx) = mpsc::channel(CONTROL);
        let (incoming_tx, incoming_rx) = mpsc::channel(INCOMING_DATA);
        let (incoming_control_tx, incoming_control_rx) = mpsc::channel(CONTROL);
        let owner = state.clone();
        let work = Box::pin(async move {
            let result = tokio::select! {
                biased;
                _ = stopped(owner.stopped.subscribe()) => owner.outcome().map_err(io::Error::other),
                _ = stopped(owner.receiving.closed()) => Err(broken("received frame abandoned")),
                result = receive_data(data.0, incoming_tx, owner.receiving.clone()) => result,
                result = receive_control(control.0, incoming_control_tx, owner.sending.clone()) => result,
                result = send_data(data.1, data_rx, owner.written.clone()) => result,
                result = send_control(control.1, control_rx, owner.receiving.released()) => result,
            };
            owner.stop(result.err());
            owner.outcome()
        });
        Self {
            sender: Sender {
                data: data_tx,
                control: control_tx,
                state: state.clone(),
            },
            receiver: Receiver {
                delivered: 0,
                cancellations: BTreeMap::new(),
                data: incoming_rx,
                control: incoming_control_rx,
                state: state.clone(),
            },
            driver: Driver { work, result: None, state },
        }
    }
}

impl Sender {
    pub async fn send(&self, frame: Frame) -> io::Result<()> {
        let packet = Packet::Frame(frame);
        tokio::select! {
            biased;
            _ = stopped(self.state.stopped.subscribe()) => Err(self.state.error()),
            result = async {
                if let Packet::Frame(Frame::Cancel { id }) = packet {
                    let preceding = *self.state.queued.lock().expect("transport queue order lock");
                    return self.control.send(Packet::Cancellation { id, preceding }).await.map_err(|_| self.state.error());
                }
                let reservation = self.state.sending.reserve(&packet).await?;
                let slot = self.data.reserve().await.map_err(|_| self.state.error())?;
                let mut queued = self.state.queued.lock().expect("transport queue order lock");
                *queued = queued.checked_add(1).ok_or_else(|| super::invalid("transport queue counter overflow"))?;
                slot.send(Outgoing { packet, reservation });
                Ok(())
            } => result,
        }
    }

    pub async fn drain(&self) -> io::Result<()> {
        let through = *self.state.queued.lock().expect("transport queue order lock");
        let mut written = self.state.written.subscribe();
        loop {
            if *written.borrow_and_update() >= through {
                return Ok(());
            }
            tokio::select! {
                biased;
                _ = stopped(self.state.stopped.subscribe()) => return Err(self.state.error()),
                result = written.changed() => result.map_err(|_| self.state.error())?,
            }
        }
    }

    pub fn close(&self) {
        self.state.stop(None);
    }
}

impl Receiver {
    pub async fn receive(&mut self) -> io::Result<Option<Received>> {
        let mut received = tokio::select! {
            biased;
            // Complete Buffered Frames Precede The Terminal Transport Outcome.
            Some(frame) = self.control.recv() => frame,
            Some(frame) = self.data.recv() => frame,
            _ = stopped(self.state.stopped.subscribe()) => return self.state.outcome().map(|()| None).map_err(io::Error::other),
        };
        if let Some(preceding) = received.preceding {
            if preceding > self.delivered {
                let Frame::Cancel { id } = received.frame else {
                    unreachable!("only cancellation has a data watermark");
                };
                if !self.cancellations.contains_key(&id) && self.cancellations.len() >= EARLY_CANCELLATIONS {
                    self.state.stop(Some(super::invalid("early cancellation capacity exhausted")));
                    return Err(self.state.error());
                }
                self.cancellations.entry(id).and_modify(|through| *through = (*through).max(preceding)).or_insert(preceding);
            }
        } else {
            self.delivered = self.delivered.checked_add(1).ok_or_else(|| super::invalid("received data counter overflow"))?;
            if let Frame::Call { id, .. } = &received.frame {
                received.cancelled = self.cancellations.remove(id).is_some();
            }
            self.cancellations.retain(|_, preceding| *preceding > self.delivered);
        }
        Ok(Some(received))
    }
}

impl Drop for Receiver {
    fn drop(&mut self) {
        self.state.stop(None);
    }
}

impl Driver {
    pub async fn run(&mut self) -> io::Result<()> {
        if self.result.is_none() {
            self.result = Some(self.work.as_mut().await);
        }
        self.result.as_ref().expect("driver completed").clone().map_err(io::Error::other)
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        // Dropping The Retained Future Closes All Four Physical Halves Together.
        self.state.stop(None);
    }
}

impl State {
    fn stop(&self, error: Option<io::Error>) {
        let mut failure = self.failure.lock().expect("transport failure lock");
        if *self.stopped.borrow() {
            return;
        }
        *failure = error.map(Arc::new);
        self.sending.close();
        // Buffered Frames Retain Their Consumption Receipts After Transport Closure.
        self.stopped.send_replace(true);
    }

    fn outcome(&self) -> Outcome {
        match &*self.failure.lock().expect("transport failure lock") {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    fn error(&self) -> io::Error {
        self.outcome().err().map(io::Error::other).unwrap_or_else(|| broken("sidecar connection closed"))
    }
}

/* === Public Functions === */

/* === Private Functions === */

async fn stopped(mut receiver: watch::Receiver<bool>) {
    while !*receiver.borrow_and_update() {
        if receiver.changed().await.is_err() {
            break;
        }
    }
}

async fn receive_data(mut reader: IoReader, incoming: mpsc::Sender<Received>, credit: Arc<ReceiveCredit>) -> io::Result<()> {
    loop {
        let Some((Packet::Frame(frame), consumption)) = read_data(&mut reader, &credit).await? else {
            return Err(broken("sidecar data channel ended"));
        };
        incoming
            .try_send(Received {
                frame,
                cancelled: false,
                preceding: None,
                consumption: Some(consumption),
            })
            .map_err(|_| broken("bounded sidecar data receiver unavailable"))?;
    }
}

async fn receive_control(mut reader: IoReader, incoming: mpsc::Sender<Received>, credit: Arc<Credit>) -> io::Result<()> {
    loop {
        match read(&mut reader, Role::Control).await? {
            Some(Packet::Credit(consumed)) => credit.returned(consumed)?,
            // A Full Queue Backpressures The Control Pipe Without Losing Its Packet.
            Some(Packet::Cancellation { id, preceding }) => incoming
                .send(Received {
                    frame: Frame::Cancel { id },
                    consumption: None,
                    cancelled: false,
                    preceding: Some(preceding),
                })
                .await
                .map_err(|_| broken("sidecar control receiver closed"))?,
            Some(Packet::Frame(_)) => {
                return Err(super::invalid("cancellation requires a data watermark"));
            }
            None => return Err(broken("sidecar control channel ended")),
        }
    }
}

async fn send_data(mut writer: IoWriter, mut outgoing: mpsc::Receiver<Outgoing>, written: watch::Sender<u64>) -> io::Result<()> {
    // Wire Credits Bound In-Flight Packets; This Separate Driver-Owned Scratch
    // Holds At Most One Validated Frame, Released When The Driver Exits.
    // Scalar Small Frames Reuse It; Large Payloads Remain In Their Own Backing.
    let mut bytes = Vec::new();
    while let Some(Outgoing { packet, reservation }) = outgoing.recv().await {
        reservation.issued()?;
        write_with(&mut writer, &packet, &mut bytes).await?;
        let span = tracing::trace_span!(target: "harmos::carrier", "sidecar.frame.flush");
        tracing::Instrument::instrument(writer.flush(), span).await?;
        let next = (*written.borrow()).checked_add(1).ok_or_else(|| super::invalid("written data counter overflow"))?;
        written.send_replace(next);
    }
    Err(broken("sidecar data sender closed"))
}

async fn send_control(mut writer: IoWriter, mut outgoing: mpsc::Receiver<Packet>, mut released: watch::Receiver<u64>) -> io::Result<()> {
    let initial = *released.borrow_and_update();
    if initial != 0 {
        write(&mut writer, &Packet::Credit(initial)).await?;
        writer.flush().await?;
    }
    loop {
        let packet = tokio::select! {
            packet = outgoing.recv() => packet.ok_or_else(|| broken("sidecar control sender closed"))?,
            changed = released.changed() => {
                changed.map_err(|_| broken("sidecar consumption owner closed"))?;
                Packet::Credit(*released.borrow_and_update())
            }
        };
        write(&mut writer, &packet).await?;
        writer.flush().await?;
    }
}

fn broken(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, message)
}

/* === Tests === */
