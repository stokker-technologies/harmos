/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Render Prepared Build Results And Development Outcomes.

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

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */
pub fn build(build: &harmos_packaging::Build, json: bool) -> Result<(), harmos_packaging::Error> {
    if json {
        println!("{}", serde_json::to_string_pretty(build)?);
    } else {
        for artifact in &build.artifacts {
            println!("{}: {}", artifact.producer, artifact.path.display());
        }
        for schema in &build.schemas {
            println!(
                "{}/{}: {}",
                schema.producer,
                schema.catalog,
                schema.directory.display()
            );
        }

    }
    Ok(())
}

pub fn dev(event: harmos_packaging::DevEvent, json: bool) -> Result<(), harmos_packaging::Error> {
    use harmos_packaging::DevEvent;
    let record = match event {
        DevEvent::Started { attempt } => {
            if !json {
                eprintln!("Build attempt {attempt}");
            }
            serde_json::json!({"event":"started", "attempt":attempt})
        }
        DevEvent::Finished {
            attempt,
            result: Ok(result),
        } => {
            if !json {
                build(&result, false)?;
            }
            serde_json::json!({"event":"built", "attempt":attempt, "build":result})
        }
        DevEvent::Finished {
            attempt,
            result: Err(error),
        } => {
            eprintln!("Build attempt {attempt} failed: {error}");
            serde_json::json!({"event":"failed", "attempt":attempt, "error":error.to_string()})
        }
    };
    if json {
        println!("{}", serde_json::to_string(&record)?);
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */
