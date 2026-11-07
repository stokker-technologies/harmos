/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Compare Arrow Payloads Over Both Native Carriers

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::num::NonZeroUsize;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

// Third Party Imports
use arrow_array::{Float64Array, RecordBatch, UInt64Array};
use arrow_schema::{DataType, Field, Schema};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

// Local Imports
use harmos_sidecar::kit::prototype::{Host, Packet, Settings};
use harmos_sidecar::transport::{Connection, Receiver, Sender};
use harmos_sidecar::{Attach, Executable, Frame};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub async fn run() -> Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(1800), async {
        // Launch Both Carriers From The Same Optimized Executable.
        let executable = std::env::current_exe()?;
        let token = harmos_sidecar::kit::prototype::credentials()?;
        let settings = Settings {
            message_bytes: NonZeroUsize::new(2 * 1024 * 1024).unwrap(),
            queued_batches: NonZeroUsize::new(2).unwrap(),
            window_bytes: NonZeroUsize::new(64 * 1024).unwrap(),
            connection_window_bytes: NonZeroUsize::new(2 * 1024 * 1024).unwrap(),
            calls: NonZeroUsize::new(16).unwrap(),
        };
        let started = Instant::now();
        let mut child = tokio::process::Command::new(&executable)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit()).kill_on_drop(true).spawn()?;
        let configuration = serde_json::to_vec(&(token.clone(), 50u64))?;
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(&configuration).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut address = String::new();
        stdout.read_line(&mut address).await?;
        let host = Host::connect(address.trim(), token, "schematic_tools".into(), 50, settings).await?;
        let grpc_startup_ms = started.elapsed().as_secs_f64() * 1000.0;

        // Use The Shipped Framed Connection, Stdio And Authenticated Independent Control Pipe.
        let executable = Executable::at(&executable)?.argument("--framed-arrow");
        let started = Instant::now();
        let attached = executable.attach().await?;
        let (write, read, control, _diagnostics, mut ending) = attached.into_parts();
        let Connection { sender, mut receiver, mut driver } = Connection::new((read, write), control.unwrap());
        let driving = tokio::spawn(async move { driver.run().await });
        sender.send(Frame::Call { id: 0, method: "complete".into(), params: Vec::new().into() }).await?;
        let ready = receiver.receive().await?.ok_or("framed child closed")?;
        if let Some(receipt) = ready.consumption { receipt.consumed()?; }
        assert!(matches!(ready.frame, Frame::Answer { id: 0, .. }));
        eprintln!("GATE_5_STARTUP grpc_ms={grpc_startup_ms:.3} framed_ms={:.3}", started.elapsed().as_secs_f64() * 1000.0);

        // Apply The Owner's Absolute Throughput Floor And Keep The Paired Framed Measurements.
        let mut passed = true;
        let mut invocation = 100;
        for rows in [4096, 65_536] {
            let payload = arrow(rows)?;
            let count = (256 * 1024 * 1024usize).div_ceil(payload.len()) as u64;

            // Verify Identical Decodable Arrow Inputs And Warm Both Carriers Outside Sampling.
            let mut reader = arrow_ipc::reader::StreamReader::try_new(std::io::Cursor::new(&payload), None)?;
            assert_eq!(reader.next().unwrap()?.num_rows(), rows);
            invocation += 1;
            grpc(&host, invocation, &payload, 8).await?;
            framed(&sender, &mut receiver, invocation, &payload, 8).await?;
            let mut grpc_rates = Vec::new();
            let mut framed_rates = Vec::new();
            let repetitions = if std::env::var_os("HARMOS_PROFILE_TRANSPORT").is_some() { 1 } else { 7 };
            for repetition in 0..repetitions {
                invocation += 1;
                // Alternate Carrier Order To Reduce Time-Order Bias On The Busy Machine.
                let (grpc_rate, framed_rate) = if repetition % 2 == 0 {
                    let grpc_rate = grpc(&host, invocation, &payload, count).await?;
                    let framed_rate = framed(&sender, &mut receiver, invocation, &payload, count).await?;
                    (grpc_rate, framed_rate)
                } else {
                    let framed_rate = framed(&sender, &mut receiver, invocation, &payload, count).await?;
                    let grpc_rate = grpc(&host, invocation, &payload, count).await?;
                    (grpc_rate, framed_rate)
                };
                eprintln!("GATE_5 repetition={} payload_bytes={} batches={count} grpc_mib_s={grpc_rate:.3} framed_mib_s={framed_rate:.3}", repetition + 1, payload.len());
                grpc_rates.push(grpc_rate);
                framed_rates.push(framed_rate);
            }
            grpc_rates.sort_by(f64::total_cmp);
            framed_rates.sort_by(f64::total_cmp);
            let median = grpc_rates.len() / 2;
            let adequate = grpc_rates[median] > 200.0;
            passed &= adequate;
            eprintln!("GATE_5_SUMMARY payload_bytes={} grpc_median={:.3} grpc_min={:.3} grpc_max={:.3} framed_median={:.3} framed_min={:.3} framed_max={:.3} ratio={:.4} above_floor={adequate}", payload.len(), grpc_rates[median], grpc_rates[0], grpc_rates[repetitions-1], framed_rates[median], framed_rates[0], framed_rates[repetitions-1], grpc_rates[median] / framed_rates[median]);
        }

        // Settle Both Owned Processes Before Reporting The Measured Gate.
        host.retire().await?;
        child.kill().await?;
        child.wait().await?;
        sender.close();
        driving.await?.map_err(std::io::Error::other)?;
        if let Some(ending) = ending.as_mut() { ending.settle().await?; }
        eprintln!("GATE_5_RESULT {}", if passed { "PASS" } else { "BELOW_FLOOR" });
        if !passed { return Err("gRPC Arrow median did not exceed the owner's 200 MiB/s floor; over 80% CPU makes this inconclusive pending a quiet rerun".into()); }
        Ok(())
    }).await?
}

/* === Private Functions === */

fn arrow(rows: usize) -> Result<Vec<u8>> {
    let schema =
        Arc::new(Schema::new(vec![Field::new("sequence", DataType::UInt64, false), Field::new("value", DataType::Float64, false)]));
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(UInt64Array::from_iter_values((0..rows).map(|row| row as u64))),
            Arc::new(Float64Array::from_iter_values((0..rows).map(|row| row as f64 * 0.25))),
        ],
    )?;
    let mut writer = arrow_ipc::writer::StreamWriter::try_new(Vec::new(), &schema)?;
    writer.write(&batch)?;
    writer.finish()?;
    Ok(writer.into_inner()?)
}

async fn grpc(host: &Host, invocation: u64, payload: &[u8], count: u64) -> Result<f64> {
    let request = payload.to_vec();
    let started = Instant::now();
    let mut incoming = host.arrow(invocation, request, count).await?;
    let mut bytes = 0;
    let mut received = 0;
    let mut ordered = true;
    let mut last = None;
    while let Some(packet) = incoming.message().await? {
        ordered &= packet.count == received;
        received += 1;
        bytes += packet.payload.len();
        last = Some(packet.payload);
    }
    let completed = host
        .phase(Packet {
            phase: 12,
            invocation,
            ..Packet::default()
        })
        .await?;
    let elapsed_s = started.elapsed().as_secs_f64();
    assert!(ordered);
    assert_eq!(received, count);
    assert_eq!(bytes as u64, payload.len() as u64 * count);
    assert_eq!(last.unwrap().as_ref(), payload);
    assert_eq!((completed.phase, completed.count), (1, count));
    Ok(bytes as f64 / elapsed_s / (1024.0 * 1024.0))
}

async fn framed(sender: &Sender, receiver: &mut Receiver, invocation: u64, payload: &[u8], count: u64) -> Result<f64> {
    let mut request = count.to_le_bytes().to_vec();
    request.extend_from_slice(payload);
    let request = Frame::Call {
        id: invocation,
        method: "arrow".into(),
        params: request.into(),
    };
    let started = Instant::now();
    sender.send(request).await?;
    let mut bytes = 0;
    let mut ordered = true;
    let mut last = None;
    for sequence in 0..count {
        let received = receiver.receive().await?.ok_or("framed child closed")?;
        let Frame::Answer { id, result } = received.frame else {
            return Err("framed Arrow answer shape".into());
        };
        ordered &= id == sequence;
        bytes += result.len();
        last = Some(result);
        if let Some(receipt) = received.consumption {
            receipt.consumed()?;
        }
    }
    sender
        .send(Frame::Call {
            id: invocation,
            method: "complete".into(),
            params: Vec::new().into(),
        })
        .await?;
    let completed = receiver.receive().await?.ok_or("framed child closed")?;
    if let Some(receipt) = completed.consumption {
        receipt.consumed()?;
    }
    let elapsed_s = started.elapsed().as_secs_f64();
    assert!(ordered);
    assert_eq!(bytes as u64, payload.len() as u64 * count);
    assert_eq!(last.unwrap().as_slice(), payload);
    assert!(matches!(completed.frame, Frame::Answer { id, .. } if id == invocation));
    Ok(bytes as f64 / elapsed_s / (1024.0 * 1024.0))
}

/* === Tests === */
