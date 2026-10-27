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
use std::process::Command;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

// Third Party Imports
use harmos_runtime::storage::{StorageManager, StorageSettings};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

// Local Imports

/* === Constants === */

const SAMPLE_INTERVAL: Duration = Duration::from_millis(1);
/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Default)]
pub struct Observed {
    pending_write_bytes: usize,
    rss_baseline_bytes: Option<usize>,
    sampled_peak_rss_bytes: Option<usize>,
    samples: u64,
}

pub struct Probe {
    stop: Arc<(Mutex<bool>, Condvar)>,
    thread: Option<JoinHandle<Observed>>,
}
/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Probe {
    pub fn start<C: 'static>(storage: StorageManager<C>) -> Self {
        Self::begin(move || {
            let status = storage.status();
            status.pending_bytes
        })
    }

    pub fn start_process() -> Self {
        Self::begin(|| 0)
    }

    fn begin(sample: impl Fn() -> usize + Send + 'static) -> Self {
        let stop = Arc::new((Mutex::new(false), Condvar::new()));
        let signal = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            let mut observed = Observed {
                rss_baseline_bytes: rss(),
                ..Observed::default()
            };
            loop {
                let write = sample();
                observed.pending_write_bytes = observed.pending_write_bytes.max(write);
                if let Some(bytes) = rss() {
                    observed.sampled_peak_rss_bytes = Some(observed.sampled_peak_rss_bytes.unwrap_or(0).max(bytes));
                }
                observed.samples += 1;
                let (lock, wake) = &*signal;
                let stopped = lock.lock().unwrap();
                if *stopped {
                    break;
                }
                let (stopped, _) = wake.wait_timeout(stopped, SAMPLE_INTERVAL).unwrap();
                if *stopped {
                    break;
                }
            }
            observed
        });
        Self { stop, thread: Some(thread) }
    }

    pub fn finish(mut self) -> Observed {
        self.stop();
        self.thread.take().unwrap().join().unwrap()
    }

    fn stop(&self) {
        let (lock, wake) = &*self.stop;
        *lock.lock().unwrap() = true;
        wake.notify_one();
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Observed {
    pub fn merge(&mut self, other: Self) {
        self.pending_write_bytes = self.pending_write_bytes.max(other.pending_write_bytes);
        if let Some(bytes) = other.rss_baseline_bytes {
            self.rss_baseline_bytes = Some(self.rss_baseline_bytes.unwrap_or(0).max(bytes));
        }
        if let Some(bytes) = other.sampled_peak_rss_bytes {
            self.sampled_peak_rss_bytes = Some(self.sampled_peak_rss_bytes.unwrap_or(0).max(bytes));
        }
        self.samples += other.samples;
    }

    pub fn report(&self, story: &str, dataset: usize, concurrency: usize, settings: Option<&StorageSettings>) {
        println!(
            "{}",
            json!({
                "kind": "storage_benchmark_resources", "story": story, "dataset_rows_or_bytes": dataset,
                "backend": if settings.is_some() {"shipped_json_storage"} else {"arrow_transform_kernel"}, "concurrency": concurrency,
                "pending_write_sampled_peak_bytes": self.pending_write_bytes,
                "process_rss_baseline_max_bytes": self.rss_baseline_bytes, "process_sampled_peak_rss_bytes": self.sampled_peak_rss_bytes,
                "rss_available": self.sampled_peak_rss_bytes.is_some(), "sample_interval_us": SAMPLE_INTERVAL.as_micros(), "samples": self.samples,
                "settings": settings.map(|settings| format!("{settings:?}")),
                "memory_note": "Periodic samples include the whole benchmark process and may miss shorter peaks; unavailable platforms report null. Sampling overhead applies to all instrumented timings."
            })
        );
    }
}
/* === Public Functions === */

pub fn provenance() -> &'static Value {
    static VALUE: OnceLock<Value> = OnceLock::new();
    VALUE.get_or_init(|| {
        json!({
            "revision": command("git", &["rev-parse", "HEAD"]),
            "worktree_status": command("git", &["status", "--porcelain"]),
            "rustc": command("rustc", &["--version", "--verbose"]),
            "lock_sha256": file_hash(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock")),
            "tracked_diff_sha256": command("git", &["diff", "HEAD", "--binary"]).map(|diff| digest(diff.as_bytes())),
            "package_version": env!("CARGO_PKG_VERSION"), "grpc_enabled": cfg!(feature = "grpc"),
            "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
            "available_parallelism": std::thread::available_parallelism().ok().map(|count| count.get()),
            "generator": "balanced-three-phase-sine-v1", "amplitude": super::waveforms::AMPLITUDE,
            "samples_per_period": super::waveforms::SAMPLES_PER_PERIOD,
        })
    })
}
/* === Private Functions === */

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

fn file_hash(path: std::path::PathBuf) -> Option<String> {
    std::fs::read(path).ok().map(|bytes| digest(&bytes))
}

fn command(binary: &str, args: &[&str]) -> Option<String> {
    let mut command = Command::new(binary);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let output = command.args(args).current_dir(env!("CARGO_MANIFEST_DIR")).output().ok()?;
    output.status.success().then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[cfg(target_os = "linux")]
fn rss() -> Option<usize> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let row = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    row.split_whitespace().nth(1)?.parse::<usize>().ok()?.checked_mul(1024)
}
#[cfg(not(target_os = "linux"))]
fn rss() -> Option<usize> {
    None
}
/* === Tests === */
