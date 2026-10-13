/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Adapt Shared Cargo Flags Into Library-Owned Build Settings.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(clap::Args)]
pub struct BuildArguments {
    #[arg(long, default_value = "Cargo.toml")]
    manifest_path: std::path::PathBuf,
    #[arg(short = 'p', long = "package")]
    packages: Vec<String>,
    #[arg(long)]
    target: Option<String>,
    #[arg(long)]
    target_dir: Option<std::path::PathBuf>,
    #[arg(long)]
    release: bool,
    #[arg(long)]
    timings: bool,
    #[arg(long, value_delimiter = ',')]
    features: Vec<String>,
    #[arg(long)]
    no_default_features: bool,
    #[arg(long)]
    pub json: bool,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl BuildArguments {
    pub fn settings(self, backend: harmos::config::BuildBackend) -> harmos::config::BuildSettings {
        harmos::config::BuildSettings {
            backend,
            manifest: self.manifest_path,
            packages: self.packages,
            target: self.target,
            target_directory: self.target_dir,
            release: self.release,
            timings: self.timings,
            features: self.features,
            default_features: !self.no_default_features,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
