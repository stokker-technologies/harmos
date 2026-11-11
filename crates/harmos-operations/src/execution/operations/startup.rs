/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Typed Startup Configuration Errors

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fmt;

// Third Party Imports

// Local Imports
use super::{FailureKind, ServingError};
use harmos_domain::{Failure, failure::Origin};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StartupReport {
    pub roots: Vec<StartupIssue>,
    pub consequences: Vec<StartupIssue>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StartupIssue {
    pub failure: Failure,
    pub resource: Option<String>,
    pub operation: Option<String>,
}

struct Messages<'a>(&'a [StartupError]);

/* === Enums === */

#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("resource `{name}` is already configured")]
    DuplicateResource { name: &'static str },

    #[error("catalog `{name}` is already configured")]
    DuplicateCatalog { name: &'static str },

    #[error("operation `{operation}` requires missing resource `{resource}`")]
    MissingResource { operation: String, resource: &'static str },

    #[error("lane `{lane}` is not declared by the application")]
    UnknownLane { lane: &'static str },

    #[error("startup configuration failed: {}", Messages(errors))]
    Multiple { errors: Vec<StartupError> },

    #[error("operation binding failed: {source}")]
    Binding {
        #[source]
        source: Box<ServingError>,
    },

    #[error("runtime startup failed: {source}")]
    Runtime {
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/* === Traits === */

/* === Implementations === */

impl fmt::Display for Messages<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut report = StartupReport {
            roots: Vec::new(),
            consequences: Vec::new(),
        };
        let preparation_failed = self.0.iter().any(StartupError::preparation_failed);
        for error in self.0 {
            error.collect(&mut report, preparation_failed);
        }
        fmt::Display::fmt(&report, formatter)
    }
}

impl StartupError {
    pub fn aggregate(mut errors: Vec<Self>) -> Option<Self> {
        match errors.len() {
            0 => None,
            1 => errors.pop(),
            _ => Some(Self::Multiple { errors }),
        }
    }

    pub fn report(&self) -> StartupReport {
        let mut report = StartupReport {
            roots: Vec::new(),
            consequences: Vec::new(),
        };
        self.collect(&mut report, self.preparation_failed());
        report
    }

    fn preparation_failed(&self) -> bool {
        match self {
            Self::Runtime { .. } => true,
            Self::Multiple { errors } => errors.iter().any(Self::preparation_failed),
            _ => false,
        }
    }

    fn collect(&self, report: &mut StartupReport, preparation_failed: bool) {
        if let Self::Multiple { errors } = self {
            for error in errors {
                error.collect(report, preparation_failed);
            }
            return;
        }
        let (resource, operation) = match self {
            Self::MissingResource { resource, operation } => (Some((*resource).to_owned()), Some(operation.clone())),
            _ => (None, None),
        };
        let issue = StartupIssue {
            failure: Failure::originate(FailureKind::Permanent, self, Origin::Plugin),
            resource,
            operation,
        };
        // Preparation Is Opaque; Missing Resources Are Consequences Of The Failed Pass.
        if preparation_failed && matches!(self, Self::MissingResource { .. }) {
            report.consequences.push(issue);
        } else {
            report.roots.push(issue);
        }
    }

    // Carriers Preserve The Whole Startup Aggregate Instead Of Losing Its Sibling Sources.
    pub(crate) fn failure(error: &(dyn std::error::Error + 'static), kind: FailureKind) -> Failure {
        let startup = std::iter::successors(Some(error), |source| source.source()).find_map(|source| {
            source.downcast_ref::<Self>().or_else(|| match source.downcast_ref::<ServingError>() {
                Some(ServingError::Startup(startup)) => Some(startup.as_ref()),
                _ => None,
            })
        });
        match startup {
            Some(startup) => startup.report().into_failure(),
            None => Failure::originate(kind, error, Origin::Plugin),
        }
    }
}

impl StartupReport {
    pub fn from_failure(failure: &Failure) -> Option<Self> {
        serde_json::from_str(failure.detail("harmos.startup")?).ok()
    }

    pub fn into_failure(self) -> Failure {
        let failure =
            if self.roots.len() == 1 { self.roots[0].failure.clone() } else { Failure::new(FailureKind::Permanent, self.to_string()) };
        let details = serde_json::to_string(&self).expect("startup diagnostics serialize");
        failure.with_detail("harmos.startup", details)
    }
}

impl fmt::Display for StartupReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, issue) in self.roots.iter().enumerate() {
            if index > 0 {
                formatter.write_str("; ")?;
            }
            formatter.write_str(issue.failure.cause())?;
        }
        let count = self.consequences.len();
        if count > 0 {
            let noun = if count == 1 { "operation" } else { "operations" };
            write!(formatter, "; {count} {noun} unavailable because startup failed")?;
        }
        Ok(())
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_roots() {
        let original = Failure::originate(FailureKind::Permanent, &std::io::Error::other("token unavailable"), Origin::Plugin)
            .with_detail("dokime.secret", "api_token");
        let refusal = harmos_domain::Refusal::failed(10001, original);
        let original = refusal.failure().clone();
        let error = StartupError::aggregate(vec![
            StartupError::Runtime { source: Box::new(refusal) },
            StartupError::UnknownLane { lane: "independent" },
        ])
        .unwrap();
        let display = error.to_string();
        assert!(display.contains("token unavailable"));
        assert!(display.contains("lane `independent`"));
        assert!(!display.contains("Runtime {"));
        assert!(!display.contains("Failure(Record"));

        let serving = ServingError::Startup(Box::new(error));
        let failure = StartupError::failure(&serving, FailureKind::Permanent);
        let received = Failure::decode(&failure.encode()).unwrap();
        let report = StartupReport::from_failure(&received).unwrap();
        assert_eq!(report.roots.len(), 2);
        assert!(report.consequences.is_empty());
        assert_eq!(report.roots[0].failure, original);
        assert_eq!(report.roots[0].failure.detail("dokime.secret"), Some("api_token"));
        assert_eq!(report.roots[0].failure.refusal_code(), Some(10001));
    }

    #[test]
    fn single_secret_root() {
        let refusal = harmos_domain::Refusal::failed(
            10001,
            Failure::new(FailureKind::Permanent, "secret unavailable").with_detail("dokime.secret", "api_token"),
        );
        let original = refusal.failure().clone();
        let error = StartupError::Runtime { source: Box::new(refusal) };
        let received = StartupError::failure(&error, FailureKind::Permanent);
        assert_eq!(received.cause(), original.cause());
        assert_eq!(received.detail("dokime.secret"), Some("api_token"));
        assert_eq!(received.refusal_code(), Some(10001));
        assert_eq!(StartupReport::from_failure(&received).unwrap().roots[0].failure, original);
    }

    #[test]
    fn prepare_consequences() {
        for first in [false, true] {
            let mut errors = (0..20)
                .map(|index| StartupError::MissingResource {
                    operation: format!("suite.{index}.run"),
                    resource: "Workflow",
                })
                .collect::<Vec<_>>();
            let refusal = harmos_domain::Refusal::failed(
                10001,
                Failure::new(FailureKind::Permanent, "token unavailable").with_detail("dokime.secret", "api_token"),
            );
            let original = refusal.failure().clone();
            errors.insert(if first { 0 } else { errors.len() }, StartupError::Runtime { source: Box::new(refusal) });
            let error = StartupError::aggregate(errors).unwrap();
            let report = error.report();
            assert_eq!(report.roots.len(), 1);
            assert_eq!(report.roots[0].failure, original);
            assert_eq!(report.consequences.len(), 20);
            for (index, issue) in report.consequences.iter().enumerate() {
                assert_eq!(issue.operation.as_deref(), Some(format!("suite.{index}.run").as_str()));
                assert_eq!(issue.resource.as_deref(), Some("Workflow"));
            }
            let message = error.to_string();
            assert!(message.contains("20 operations unavailable because startup failed"));
            assert!(!message.contains("suite."));
            assert!(!message.contains("Runtime {"));
            assert!(!message.contains("MissingResource {"));
            let carrier = StartupError::failure(&error, FailureKind::Permanent);
            let decoded = Failure::decode(&carrier.encode()).unwrap();
            assert_eq!(StartupReport::from_failure(&decoded), Some(report));
        }
    }

    #[test]
    fn missing_resource_roots() {
        let error = StartupError::aggregate(vec![
            StartupError::MissingResource {
                operation: "suite.run".to_owned(),
                resource: "Workflow",
            },
            StartupError::MissingResource {
                operation: "probe.read".to_owned(),
                resource: "Probe",
            },
        ])
        .unwrap();
        let report = error.report();
        assert_eq!(report.roots.len(), 2);
        assert!(report.consequences.is_empty());
        assert!(error.to_string().contains("operation `probe.read` requires missing resource `Probe`"));
    }

    #[test]
    fn independent_prepare_roots() {
        let error = StartupError::aggregate(vec![
            StartupError::Runtime {
                source: Box::new(std::io::Error::other("equipment unavailable")),
            },
            StartupError::Multiple {
                errors: vec![
                    StartupError::Runtime {
                        source: Box::new(std::io::Error::other("database unavailable")),
                    },
                    StartupError::UnknownLane { lane: "independent" },
                    StartupError::MissingResource {
                        operation: "suite.run".to_owned(),
                        resource: "Workflow",
                    },
                ],
            },
        ])
        .unwrap();
        let report = error.report();
        assert_eq!(report.roots.len(), 3);
        assert_eq!(report.consequences.len(), 1);
        assert!(report.roots[0].failure.cause().contains("equipment unavailable"));
        assert!(report.roots[1].failure.cause().contains("database unavailable"));
        assert!(report.roots[2].failure.cause().contains("independent"));
        assert!(error.to_string().contains("1 operation unavailable because startup failed"));
    }
}
