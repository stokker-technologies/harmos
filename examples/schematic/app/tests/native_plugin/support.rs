/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Native Schematic Story Mechanics

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::{num::NonZeroUsize, path::Path};

// Third Party Imports

// Local Imports
use schematic_app::{app::Runtime, document::Document};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

pub async fn host(directory: &Path) -> Runtime {
    let (clock, _) = harmos::clock::Clock::virtual_at(std::time::SystemTime::UNIX_EPOCH);
    Runtime::launch_with(Document::new(directory.join("schematic.json")).into_origin(), None, |setup| {
        setup.settings().clock(clock);
    })
    .await
    .unwrap()
}

pub fn binary() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("SCHEMATIC_PLUGIN") {
        return path.into();
    }
    let current = std::env::current_exe().unwrap();
    current.parent().unwrap().parent().unwrap().join(format!("schematic-sidecar{}", std::env::consts::EXE_SUFFIX))
}

pub fn package(binary: std::path::PathBuf) -> harmos_packaging::bundle::Bundle {
    let packages = tempfile::tempdir().unwrap();
    let build = harmos_packaging::Build {
        backend: harmos_packaging::config::BuildBackend::Sidecar,
        profile: harmos_packaging::config::BuildProfile::Dev,
        target: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        artifacts: vec![harmos_packaging::Artifact::read("schematic-sidecar".into(), "schematic-sidecar".into(), binary).unwrap()],
        schemas: Vec::new(),
    };
    let packaged = harmos_packaging::package(&build, packages.path()).unwrap();
    let bytes = std::fs::read(&packaged[0].path).unwrap();
    harmos_packaging::bundle::Bundle::inspect(&bytes, Default::default()).unwrap()
}

pub fn snapshot() -> schematic_app::domain::model::Snapshot {
    schematic_app::domain::model::Snapshot {
        symbols: vec![1, 2],
        connections: vec![schematic_app::domain::model::Connection { from: 1, to: 2 }],
    }
}

pub fn settings() -> harmos::operations::StreamSettings {
    harmos::operations::StreamSettings {
        items: NonZeroUsize::new(4).unwrap(),
        bytes: NonZeroUsize::new(4096).unwrap(),
    }
}

/* === Private Functions === */

/* === Tests === */
