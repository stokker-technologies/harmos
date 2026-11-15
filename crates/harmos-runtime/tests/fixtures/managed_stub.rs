/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Managed Process Whose Lifetime the Story Controls

/* === Imports === */

// Standard Library Imports
use std::{
    env,
    io::{Read, Write},
    net::TcpStream,
    process::ExitCode,
};

/* === Public Functions === */

/// Announce Another Server's Address, Then Exit When the Story Closes Its Control Socket.
fn main() -> ExitCode {
    let mut arguments = env::args().skip(1);
    let control = arguments.next().expect("control address");
    let announced = arguments.next().expect("announced plugin address");
    let mut control = TcpStream::connect(control).expect("control connection");
    let mut stdout = std::io::stdout();
    writeln!(stdout, "harmos listening {announced}").expect("readiness line");
    stdout.flush().expect("readiness flush");
    let mut byte = [0];
    while control.read(&mut byte).is_ok_and(|read| read > 0) {}
    ExitCode::from(3)
}
