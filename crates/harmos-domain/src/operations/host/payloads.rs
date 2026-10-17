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

// Third Party Imports

// Local Imports
use crate::operations::common::payload::payload;
#[cfg(feature = "grpc")]
use crate::operations::common::payload::repeated;

/* === Constants === */

/* === Macros === */
crate::with_protobuf! {
    repeated!(Header);
    repeated!(Remembered);
    repeated!(Measured);
}

payload! { OperationsRequest,
    after: Option<String> => (1, Kind::String, Optional),
    rows: Option<u32> => (2, Kind::U32, Optional),
    bytes: Option<u32> => (3, Kind::U32, Optional),
}

// Catalog Is A UTF-8 JSON Array Of Canonical DiscoveredOperation Values.
payload! { OperationsResponse,
    catalog: Vec<u8> => (1, Kind::Bytes, Required),
    next: Option<String> => (2, Kind::String, Optional),
}

payload! { RecordRequest,
    name: String => (1, Kind::String, Required),
    version: u32 => (2, Kind::U32, Required),
    payload: Vec<u8> => (3, Kind::Bytes, Required),
}

payload! { Receipt,
    position: u64 => (1, Kind::U64, Required),
    correlation: u64 => (2, Kind::U64, Required),
}

payload! { RecallRequest,
    name: String => (1, Kind::String, Required),
    after: u64 => (2, Kind::U64, Required),
    limit: u32 => (3, Kind::U32, Required),
}

payload! { Remembered,
    position: u64 => (1, Kind::U64, Required),
    name: String => (2, Kind::String, Required),
    version: u32 => (3, Kind::U32, Required),
    payload: Vec<u8> => (4, Kind::Bytes, Required),
}

payload! { RecallResponse,
    rows: Vec<Remembered> => (1, Kind::Message { definition: Remembered::DEFINITION }, Repeated),
}

payload! { InvokeRequest,
    grant: String => (1, Kind::String, Required),
    key: String => (2, Kind::String, Required),
    payload: Vec<u8> => (3, Kind::Bytes, Required),
}

payload! { Header,
    name: String => (1, Kind::String, Required),
    value: String => (2, Kind::String, Required),
}

payload! { EgressRequest,
    name: String => (1, Kind::String, Required),
    method: String => (2, Kind::String, Required),
    path: String => (3, Kind::String, Required),
    headers: Vec<Header> => (4, Kind::Message { definition: Header::DEFINITION }, Repeated),
    body: Vec<u8> => (5, Kind::Bytes, Required),
}

payload! { Fetched,
    status: u32 => (1, Kind::U32, Required),
    headers: Vec<Header> => (2, Kind::Message { definition: Header::DEFINITION }, Repeated),
    body: Vec<u8> => (3, Kind::Bytes, Required),
}

payload! { SurveyRequest,
    stream: String => (1, Kind::String, Required),
    first: u64 => (2, Kind::U64, Required),
    last: u64 => (3, Kind::U64, Required),
    aggregate: String => (4, Kind::String, Required),
}

payload! { Measured,
    field: String => (1, Kind::String, Required),
    value: f64 => (2, Kind::F64, Required),
}

payload! { SurveyResponse,
    rows: Vec<Measured> => (1, Kind::Message { definition: Measured::DEFINITION }, Repeated),
}

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
