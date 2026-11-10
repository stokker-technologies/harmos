/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Run Every Rule Through the Managed Executable Interface.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
use tokio::process::Command;

// Local Imports
use super::support;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[tokio::test]
async fn managed_plugin_passes_every_rule() -> Result<(), Box<dyn std::error::Error>> {
    // Locate the Reference Host and Hand-Written Plugin.
    let host = support::binary("harmos-conformance", env!("CARGO_BIN_EXE_harmos-conformance"));
    let plugin = support::binary("harmos-conformance-plugin", env!("CARGO_BIN_EXE_harmos-conformance-plugin"));

    // Run the Complete Managed Plugin Journey.
    let output = Command::new(&host).arg("--plugin").arg(&plugin).env_remove("HARMOS_ADDRESS").output().await?;

    // Inspect Every Rule and the Managed Process Shutdown.
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;
    assert!(output.status.success(), "{stdout}\n{stderr}");
    assert!(!stdout.contains("FAIL"), "{stdout}");
    assert_eq!(stdout.lines().filter(|line| line.ends_with("| PASS |")).count(), harmos_conformance::RULE_COUNT, "{stdout}");

    // Run the Same Cases Against an Independently Managed Remote Endpoint.
    let remote = support::PluginProcess::start(&plugin)?;
    let output = Command::new(&host).arg("--address").arg(&remote.address).output().await?;
    let stdout = String::from_utf8(output.stdout)?;
    assert!(output.status.success(), "{stdout}\n{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(stdout.lines().filter(|line| line.ends_with("| PASS |")).count(), harmos_conformance::RULE_COUNT, "{stdout}");

    // Diagnose a Broken Catalog and Continue Running Every Other Case.
    let fault = support::CatalogFault::start(&remote.address).await?;
    let output = Command::new(&host).arg("--address").arg(&fault.address).output().await?;
    let stdout = String::from_utf8(output.stdout)?;
    assert_eq!(output.status.code(), Some(1), "{stdout}\n{}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout.lines().any(|line| line.starts_with("| 01 |") && line.contains("FAIL:")), "{stdout}");
    assert_eq!(stdout.lines().filter(|line| line.ends_with("| PASS |")).count(), harmos_conformance::RULE_COUNT - 1, "{stdout}");

    // Release the Adapter and Its Owned Remote Process.
    fault.close().await?;
    drop(remote);
    Ok(())
}
