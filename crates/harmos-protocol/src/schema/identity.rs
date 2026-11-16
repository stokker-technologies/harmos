/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

//! Wire Operation Identities and Executor Admission.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use super::Error;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Executor {
    Plugin,
    Host,
}

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/// Validate a Wire Operation Identity and Resolve Its Executor.
pub fn operation_identity(id: &str) -> Result<Executor, Error> {
    let invalid = || Error(format!("Declare {id}: Use a Valid Operation Identity with a Host or Plugin Executor"));
    let Some((service, method)) = id.split_once('/') else {
        let local =
            id.split('.').all(|segment| !segment.is_empty() && segment.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'));
        if local {
            return Ok(Executor::Plugin);
        }
        return Err(invalid());
    };
    let (package, service) = service.rsplit_once('.').ok_or_else(invalid)?;
    let identifier = |value: &str| {
        let mut bytes = value.bytes();
        let valid_start = bytes.next().is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_');
        let valid_rest = bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        valid_start && valid_rest
    };
    let valid_package = package.split('.').all(identifier);
    let valid_service = identifier(service);
    let valid_method = identifier(method);
    if !valid_package || !valid_service || !valid_method {
        return Err(invalid());
    }
    let segments: Vec<_> = package.split('.').collect();
    let mut executor = None;
    for (index, segment) in segments.iter().enumerate().skip(1) {
        let version = segment.strip_prefix('v').is_some_and(|value| !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()));
        if !version {
            continue;
        }
        let selected = match segments[index - 1] {
            "host" => Executor::Host,
            "plugin" => Executor::Plugin,
            _ => continue,
        };
        if executor.replace(selected).is_some() {
            return Err(invalid());
        }
    }
    executor.ok_or_else(invalid)
}

/// Admit Only Plugin Executor Identities Outside the Library Namespace.
pub fn plugin_operation_identity(id: &str) -> Result<(), Error> {
    if operation_identity(id)? != Executor::Plugin {
        return Err(Error(format!("Register {id}: Serve Operations on the Plugin Executor")));
    }
    if id.starts_with("harmos.") || id.starts_with("harmos_") {
        return Err(Error(format!("Register {id}: Use an Application Namespace; the Host Namespace Is Reserved")));
    }
    Ok(())
}

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_identities_resolve_executors_and_reject_missing_executor() {
        for (id, executor) in [
            ("example.step.0", Executor::Plugin),
            ("example.plugin.v1.Lifecycle/Initialize", Executor::Plugin),
            ("example.plugin.v2.steps.Progress/Get", Executor::Plugin),
            ("example.host.v1.Prompts/Create", Executor::Host),
            ("example.host.v1.jobs.Progress/Watch", Executor::Host),
            ("example.host.v1.jobs.v2.Progress/Watch", Executor::Host),
        ] {
            assert_eq!(operation_identity(id).unwrap(), executor);
            assert_eq!(plugin_operation_identity(id).is_ok(), executor == Executor::Plugin);
        }
        for id in [
            "",
            ".example",
            "example..step",
            "Steps/Execute",
            "example.values.Steps/Get",
            "example.model.v1.Steps/Get",
            "example.host.v1.Steps/",
            "example.host.v1.Steps/Get/Again",
            "example.plugin.v1.host.v1.Steps/Get",
        ] {
            assert!(operation_identity(id).is_err(), "{id}");
        }
        assert!(plugin_operation_identity("harmos.plugin.v1.Instance/Invoke").is_err());
    }
}
