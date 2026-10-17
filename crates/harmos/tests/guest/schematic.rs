/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Mirrors `examples/schematic/contract`, Which Lives Outside This Workspace

/* === Modules === */
pub mod model {
    #[harmos::message]
    #[derive(Clone, Debug, PartialEq)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }

    #[harmos::message]
    #[derive(Clone, Debug, PartialEq)]
    pub struct Connection {
        pub from: u64,
        pub to: u64,
    }

    #[harmos::message]
    #[derive(Clone, Debug, PartialEq)]
    pub struct Snapshot {
        pub symbols: Vec<u64>,
        pub connections: Vec<Connection>,
    }
}

pub mod operations {
    pub const POLICY: harmos::operations::OperationPolicy = harmos::operations::OperationPolicy {
        timeout: std::time::Duration::from_secs(10),
        attempt_timeout: std::time::Duration::from_secs(10),
        retries: harmos::operations::RetryPolicy::Never,
        restart: harmos::operations::RestartPolicy::Never,
    };

    #[harmos::catalog]
    pub enum Operations {
        Validate(validate::ValidateSchematic),
        Route(propose::ProposeRoute),
        Analyze(analyze::AnalyzeSchematic),
    }

    pub mod analyze {
        #[harmos::message]
        #[derive(Clone, Debug, PartialEq)]
        pub struct Finding {
            pub text: String,
        }

        #[harmos::message]
        #[derive(Clone, Debug, PartialEq)]
        pub struct Request {
            pub schematic: super::super::model::Snapshot,
        }

        #[harmos::message]
        #[derive(Clone, Debug, PartialEq)]
        pub struct Response {
            pub checked: u32,
        }

        #[harmos::operation(
            id = "schematic.analyze",
            request = Request,
            response = Response,
            output = Finding,
            policy = super::POLICY,
            visibility = harmos::operations::Visibility::Shared,
            replay = harmos::operations::ReplaySafety::Idempotent,
            version = 1,
            grpc(package = "schematic.v1", service = "Schematic", method = "Analyze"),
        )]
        pub struct AnalyzeSchematic;
    }

    pub mod propose {
        #[harmos::message]
        #[derive(Clone, Debug, PartialEq)]
        pub struct Request {
            pub from: super::super::model::Point,
            pub to: super::super::model::Point,
        }

        #[harmos::message]
        #[derive(Clone, Debug, PartialEq)]
        pub struct Response {
            pub points: Vec<super::super::model::Point>,
        }

        #[harmos::operation(
            id = "schematic.route",
            request = Request,
            response = Response,
            policy = super::POLICY,
            visibility = harmos::operations::Visibility::Shared,
            replay = harmos::operations::ReplaySafety::Idempotent,
            version = 1,
            grpc(package = "schematic.v1", service = "Schematic", method = "ProposeRoute"),
        )]
        pub struct ProposeRoute;
    }

    pub mod validate {
        #[harmos::message]
        #[derive(Clone, Debug, PartialEq)]
        pub struct Request {
            pub schematic: super::super::model::Snapshot,
        }

        #[harmos::message]
        #[derive(Clone, Debug, PartialEq)]
        pub struct Response {
            pub diagnostics: Vec<String>,
        }

        #[harmos::operation(
            id = "schematic.validate",
            request = Request,
            response = Response,
            policy = super::POLICY,
            visibility = harmos::operations::Visibility::Shared,
            replay = harmos::operations::ReplaySafety::Idempotent,
            version = 1,
            grpc(package = "schematic.v1", service = "Schematic", method = "Validate"),
        )]
        pub struct ValidateSchematic;
    }
}

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

/* === Private Functions === */

/* === Tests === */
