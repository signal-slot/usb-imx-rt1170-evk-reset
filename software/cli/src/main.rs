//! Host-side CLI for the XIAO RP2040 "USB iMX RT1170 EVK Reset" firmware.
//!
//! Talks to the firmware over its USB CDC ACM port using the same
//! line-delimited ASCII protocol described in the repo-root `README.md` §8
//! (and implemented in `../../firmware/src/main.rs`):
//!
//!   PING            -> PONG
//!   RESET [<ms>]    -> OK RESET <ms>
//!   OFF             -> OK OFF
//!   BOOTSEL         -> OK BOOTSEL  (then chip reboots into RPI-RP2)
//!
//! The serial port is opened in raw 8N1 mode by the `serialport` crate, so
//! the kernel TTY layer's default ECHO / canonical mode does not create
//! the feedback loop that bare `cat > /dev/ttyACM*` style tools suffer from.

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serialport::SerialPort;
use std::time::{Duration, Instant};

const VID: u16 = 0x1209;
const PID: u16 = 0x0001;

/// Control the XIAO RP2040 USB iMX RT1170 EVK reset firmware.
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Serial device path. Default: auto-detect by USB VID:PID = 1209:0001.
    #[arg(short, long, global = true)]
    port: Option<String>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Liveness check (expects PONG).
    Ping,
    /// Pulse RESET HIGH for <ms> milliseconds, then return to LOW.
    /// Firmware clamps to 10..=1000.
    Reset {
        #[arg(default_value_t = 100)]
        ms: u32,
    },
    /// Drive RESET LOW (deassert).
    Off,
    /// Drive RESET HIGH (hold the photo-relay asserted indefinitely until `off`).
    On,
    /// Reboot the XIAO into BOOTSEL / RPI-RP2 mode for re-flashing.
    Bootsel,
}

fn find_port() -> Result<String> {
    for p in serialport::available_ports().context("enumerate serial ports")? {
        if let serialport::SerialPortType::UsbPort(info) = &p.port_type {
            if info.vid == VID && info.pid == PID {
                return Ok(p.port_name);
            }
        }
    }
    bail!(
        "no USB CDC device matching VID:PID {:04x}:{:04x} found (pass --port to override)",
        VID,
        PID
    );
}

fn open_port(path: &str) -> Result<Box<dyn SerialPort>> {
    // Baud rate is ignored by USB CDC, but the crate requires one.
    serialport::new(path, 115_200)
        .timeout(Duration::from_millis(200))
        .open()
        .with_context(|| format!("open {}", path))
}

/// Send `command\n`, then read until we see a newline or `total` elapses.
/// Returns the reply with trailing CR/LF stripped.
fn exchange(port: &mut dyn SerialPort, command: &str, total: Duration) -> Result<String> {
    // Drop anything already buffered (stale replies, echo loops, etc).
    let _ = port.clear(serialport::ClearBuffer::Input);

    let mut tx = command.as_bytes().to_vec();
    tx.push(b'\n');
    port.write_all(&tx).context("write")?;
    port.flush().ok();

    let deadline = Instant::now() + total;
    let mut out = Vec::<u8>::new();
    let mut buf = [0u8; 256];

    while Instant::now() < deadline && !out.contains(&b'\n') {
        match port.read(&mut buf) {
            Ok(0) => std::thread::sleep(Duration::from_millis(5)),
            Ok(n) => out.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e) => return Err(e).context("read"),
        }
    }

    Ok(String::from_utf8_lossy(&out)
        .replace("\r\n", "\n")
        .trim_end_matches('\n')
        .to_string())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = match cli.port.clone() {
        Some(p) => p,
        None => find_port()?,
    };
    let mut port = open_port(&path)?;

    let (request, wait, expected_prefix) = match &cli.cmd {
        Cmd::Ping => ("PING".to_string(), Duration::from_millis(500), "PONG"),
        Cmd::Reset { ms } => (
            format!("RESET {}", ms),
            // Pulse blocks the firmware for up to `ms` ms before replying;
            // give it that plus a fixed slack.
            Duration::from_millis(u64::from((*ms).min(1000)) + 400),
            "OK RESET",
        ),
        Cmd::Off => ("OFF".to_string(), Duration::from_millis(500), "OK OFF"),
        Cmd::On => ("ON".to_string(), Duration::from_millis(500), "OK ON"),
        Cmd::Bootsel => (
            "BOOTSEL".to_string(),
            Duration::from_millis(300),
            "OK BOOTSEL",
        ),
    };

    let reply = exchange(&mut *port, &request, wait).unwrap_or_default();
    println!("{}", reply);

    if !reply.starts_with(expected_prefix) {
        // BOOTSEL is best-effort: the device may unmount before we read.
        if !matches!(cli.cmd, Cmd::Bootsel) {
            std::process::exit(1);
        }
    }
    Ok(())
}
