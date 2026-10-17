/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
mod aggregates;
#[path = "../../../../tests/support/scenarios.rs"]
mod scenario_runtime;

#[allow(dead_code)]
mod probe {
    use harmos_storage::signal::{Band, Decibel, Edge, Scale, Series, Spectrum, Summary, Taper};

    #[cfg(feature = "fft")]
    use harmos_storage::signal::{Estimate, Welch};

    type Share = fn(&[f64], &[f64], f64, f64, f64) -> Option<f64>;
    type Occupancy = fn(&[f64], &[f64], f64, f64, f64) -> Option<(f64, f64, f64)>;
    type Resampling = fn(&[f64], &[f64], &[f64]) -> Vec<f64>;

    struct Types {
        band: Band,
        decibel: Decibel,
        edge: Edge,
        #[cfg(feature = "fft")]
        estimate: Estimate,
        scale: Scale,
        series: Series<'static>,
        spectrum: Spectrum,
        summary: Summary,
        taper: Taper,
        #[cfg(feature = "fft")]
        welch: Welch,
    }

    // The frequency domain: the transform, what is read off it, and the way back.
    #[cfg(feature = "fft")]
    const TRANSFORM: fn(&[f64], f64, Taper) -> Spectrum = harmos_storage::signal::transform;
    #[cfg(feature = "fft")]
    const RECONSTRUCT: fn(&Spectrum, Option<Band>) -> Vec<f64> = harmos_storage::signal::reconstruct;
    const MAGNITUDE: fn(&[f64], &[f64]) -> Vec<f64> = harmos_storage::signal::magnitude;
    const PHASE: fn(&[f64], &[f64]) -> Vec<f64> = harmos_storage::signal::phase;
    const BINS: fn(f64, usize) -> Vec<f64> = harmos_storage::signal::bins;
    const PEAKS: fn(&[f64]) -> Vec<usize> = harmos_storage::signal::peaks;
    const HARMONICS: fn(&[f64], f64) -> Vec<usize> = harmos_storage::signal::harmonics;
    const FUNDAMENTAL: fn(&[f64], &[f64]) -> Option<usize> = harmos_storage::signal::fundamental;
    const THD: fn(&[f64], &[f64]) -> Option<f64> = harmos_storage::signal::thd;
    const BAND_ENERGY: fn(&[f64], &[f64], f64, f64) -> f64 = harmos_storage::signal::band_energy;
    const BAND_RMS: fn(&[f64], &[f64], f64, f64) -> f64 = harmos_storage::signal::band_rms;
    const CENTROID: fn(&[f64], &[f64], f64, f64) -> Option<f64> = harmos_storage::signal::centroid;
    const PAPR: fn(&[f64], &[f64], f64, f64) -> Option<f64> = harmos_storage::signal::papr;
    const ROLLOFF: Share = harmos_storage::signal::rolloff;
    const OCCUPIED: Occupancy = harmos_storage::signal::occupied_bandwidth;

    // The time domain: what to keep, how to read between, what it comes to.
    const LTTB: fn(&[f64], &[f64], usize) -> Vec<usize> = harmos_storage::signal::lttb;
    const LTTB_LOG: fn(&[f64], &[f64], usize) -> Vec<usize> = harmos_storage::signal::lttb_log;
    const DECIMATE: fn(usize, usize) -> Vec<usize> = harmos_storage::signal::decimate;
    const EXTREMA: fn(&[f64], usize) -> Vec<usize> = harmos_storage::signal::extrema;
    const ABOVE_FLOOR: fn(&[f64], f64) -> Vec<usize> = harmos_storage::signal::above_floor;
    const VALUE_AT: fn(&[f64], &[f64], f64) -> f64 = harmos_storage::signal::value_at;
    const RESAMPLE: Resampling = harmos_storage::signal::resample;
    const RESAMPLE_UNIFORM: fn(&[f64], &[f64], f64) -> Vec<f64> = harmos_storage::signal::resample_uniform;
    const RESAMPLE_COUNT: fn(&[f64], usize) -> Vec<f64> = harmos_storage::signal::resample_count;
    const NEAREST: fn(&[f64], f64) -> Option<usize> = harmos_storage::signal::nearest;
    const DERIVATIVE: fn(&[f64], &[f64]) -> Vec<f64> = harmos_storage::signal::derivative;
    const INTEGRAL: fn(&[f64], &[f64]) -> Vec<f64> = harmos_storage::signal::integral;
    const EDGES: fn(&[f64], &[f64], f64, f64) -> Vec<Edge> = harmos_storage::signal::edges;
    const CROSSING: fn(&[f64], &[f64], f64) -> Option<f64> = harmos_storage::signal::crossing;

    // The inherent surface each named type carries.
    const TAPER_COEFFICIENT: fn(&Taper, usize, usize) -> f64 = Taper::coefficient;
    const TAPER_COEFFICIENTS: fn(&Taper, usize) -> Vec<f64> = Taper::coefficients;
    const BAND_COEFFICIENT: fn(&Band, f64) -> f64 = Band::coefficient;
    const BAND_BOUNDS: fn(&Band) -> (f64, f64) = Band::bounds;
    const SPECTRUM_RATE: fn(&Spectrum) -> f64 = Spectrum::rate;
    #[cfg(feature = "fft")]
    const WELCH_ESTIMATE: fn(&Welch, &[f64], f64) -> Estimate = Welch::estimate;
    const DECIBEL_TO_DB: fn(&Decibel, f64) -> f64 = Decibel::to_db;
    const DECIBEL_TO_LINEAR: fn(&Decibel, f64) -> f64 = Decibel::to_linear;
    const DECIBEL_OFFSET: fn(&Decibel, &Decibel) -> Option<f64> = Decibel::offset_from;
    const SCALE_SUM: fn(&Scale, &Scale) -> Option<Scale> = Scale::sum;
    const SCALE_DIFFERENCE: fn(&Scale, &Scale) -> Option<Scale> = Scale::difference;
    const SUMMARY_OF: fn(&[f64]) -> Summary = Summary::of;
    const SUMMARY_PUSH: fn(&mut Summary, f64) = Summary::push;
    const SUMMARY_MERGE: fn(&Summary, &Summary) -> Summary = Summary::merge;
    const SUMMARY_VARIANCE: fn(&Summary) -> Option<f64> = Summary::variance;
    const SUMMARY_STD: fn(&Summary) -> Option<f64> = Summary::std;
    const SERIES_NEW: fn(&'static [f64]) -> Series<'static> = Series::new;
    const SERIES_SUMMARY: fn(&Series<'static>) -> Summary = Series::summary;
    const SERIES_MEDIAN: fn(&Series<'static>) -> Option<f64> = Series::median;
    const SERIES_PERCENTILE: fn(&Series<'static>, f64) -> Option<f64> = Series::percentile;
    const SERIES_ALL_BELOW: fn(&Series<'static>, f64) -> bool = Series::all_below;
    const SERIES_ALL_WITHIN: fn(&Series<'static>, f64, f64) -> bool = Series::all_within;
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
// Third Party Imports

// Local Imports

/* === Constants === */
const CENSUS: [&str; 42] = [
    "Error",
    "Result",
    "Band",
    "Decibel",
    "Edge",
    "Estimate",
    "Scale",
    "Series",
    "Spectrum",
    "Summary",
    "Taper",
    "Welch",
    "above_floor",
    "band_energy",
    "band_rms",
    "bins",
    "centroid",
    "crossing",
    "decimate",
    "derivative",
    "edges",
    "extrema",
    "fft_in_place",
    "fundamental",
    "harmonics",
    "integral",
    "lttb",
    "lttb_log",
    "magnitude",
    "nearest",
    "occupied_bandwidth",
    "papr",
    "peaks",
    "phase",
    "reconstruct",
    "resample",
    "resample_count",
    "resample_uniform",
    "rolloff",
    "thd",
    "transform",
    "value_at",
];

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */
fn crate_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn source() -> PathBuf {
    crate_root().join("src")
}

fn sources() -> Vec<(String, String)> {
    let mut found = Vec::new();
    collect(&source(), &mut found);
    found.sort();
    found
}

fn collect(directory: &Path, found: &mut Vec<(String, String)>) {
    let listing = fs::read_dir(directory).expect("the source directory is readable");

    for entry in listing.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, found);
        } else if path.extension().is_some_and(|suffix| suffix == "rs") {
            found.push((path.to_string_lossy().replace('\\', "/"), read(&path)));
        }
    }
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        .replace("\r\n", "\n")
}

fn code(text: &str) -> String {
    text.lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

fn reexported(text: &str) -> BTreeSet<String> {
    let mut handed_out = BTreeSet::new();

    for statement in text.split("pub use ").skip(1) {
        let statement = statement.split(';').next().unwrap_or_default();
        let leaves = match statement.split_once('{') {
            Some((_, group)) => group.to_owned(),
            None => statement.rsplit("::").next().unwrap_or_default().to_owned(),
        };
        handed_out.extend(
            leaves
                .split(',')
                .map(|name| name.trim().trim_end_matches('}').trim().to_owned())
                .filter(|name| !name.is_empty()),
        );
    }
    handed_out
}

fn dependencies(manifest: &str) -> BTreeSet<String> {
    manifest
        .split("[dependencies]")
        .nth(1)
        .unwrap_or_default()
        .split("\n[")
        .next()
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(name, _)| name.trim())
        .map(|name| name.strip_suffix(".workspace").unwrap_or(name).to_owned())
        .filter(|name| !name.is_empty() && !name.starts_with('#'))
        .collect()
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("crate_root_hands", || {
            let handed_out = reexported(&read(&source().join("lib.rs")));
            let accepted: BTreeSet<String> = CENSUS.iter().map(|name| (*name).to_owned()).collect();

            assert_eq!(handed_out, accepted, "the signal surface drifted");
        });
    }
    {
        scenario_runtime::check("manifest_names_approved", || {
            let manifest = read(&crate_root().join("Cargo.toml"));

            assert_eq!(
                dependencies(&manifest),
                BTreeSet::from([
                    "rustfft".to_owned(),
                    "thiserror".to_owned(),
                    "tracing".to_owned()
                ]),
                "the dependency budget is a planned FFT and shared error/tracing contracts"
            );
            assert!(
                !manifest.contains("[dev-dependencies]"),
                "a development dependency enters the lockfile too, and harmos's \
         allowance audit counts it against this crate"
            );
        });
    }
    {
        scenario_runtime::check("nothing_here_names", || {
            let found: Vec<String> = sources()
                .into_iter()
                .filter(|(_, text)| code(text).contains("harmos"))
                .map(|(path, _)| path)
                .collect();

            assert!(
                found.is_empty(),
                "this crate must lift into a repository of its own unchanged: {found:?}"
            );
        });
    }
    scenario_runtime::check("empty_input", aggregates::empty_input);
    scenario_runtime::check("single_value", aggregates::single_value);
    scenario_runtime::check("known_run", aggregates::known_run);
    scenario_runtime::check("odd_even_medians", aggregates::odd_even_medians);
    scenario_runtime::check(
        "interpolated_percentiles",
        aggregates::interpolated_percentiles,
    );
    scenario_runtime::check("sorted_once", aggregates::sorted_once);
    scenario_runtime::check("finite_extremes", aggregates::finite_extremes);
    scenario_runtime::check("non_finite_skipped", aggregates::non_finite_skipped);
    scenario_runtime::check("value_predicates", aggregates::value_predicates);
    scenario_runtime::check("summaries_agree", aggregates::summaries_agree);
    {
        scenario_runtime::check("anchor_reached_root", || {
            let root = code(&read(&source().join("lib.rs")));

            // Private modules, public names: a caller writes `harmos_storage::signal::lttb` and
            // never a module path, so the census above is the whole reachable surface.
            assert!(root.contains("mod spectrum;") && !root.contains("pub mod spectrum;"));
            assert!(root.contains("mod waveform;") && !root.contains("pub mod waveform;"));
        });
    }
}
