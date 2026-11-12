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
use std::collections::{BTreeSet, VecDeque};

// Third Party Imports
use harmos_operations::execution::operations::ServingError;
use harmos_protocol::{
    status::Event,
    v1::{History as WireHistory, PageRequest, Position},
};
use prost::Message;

// Local Imports
use super::failure;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct History {
    identity: WireHistory,
    sequence: u64,
    floor: u64,
    bytes: usize,
    events: VecDeque<Event>,
    issued: BTreeSet<u64>,
    tokens: VecDeque<String>,
    maximum_items: usize,
    maximum_bytes: usize,
    read_leases: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl History {
    pub fn new(store: &str, epoch: &str, items: usize, bytes: usize, read_leases: usize) -> Self {
        Self {
            identity: WireHistory { store: store.into(), epoch: epoch.into() },
            sequence: 0,
            floor: 0,
            bytes: 0,
            events: VecDeque::new(),
            issued: BTreeSet::new(),
            tokens: VecDeque::new(),
            maximum_items: items,
            maximum_bytes: bytes,
            read_leases,
        }
    }
    pub fn append(&mut self, mut event: Event) -> Result<(), ServingError> {
        let sequence = self.sequence.checked_add(1).ok_or_else(|| failure("Status.Append", "history exhausted"))?;
        event.sequence = sequence;
        let bytes = event.encoded_len();
        if bytes > self.maximum_bytes {
            return Err(failure("Status.Append", "event exceeds history byte budget"));
        }
        self.sequence = sequence;
        self.bytes += bytes;
        self.events.push_back(event);
        while self.events.len() > self.maximum_items || self.bytes > self.maximum_bytes {
            let event = self.events.pop_front().expect("history has an event");
            self.bytes -= event.encoded_len();
            self.floor = event.sequence;
        }
        self.issued.retain(|sequence| *sequence >= self.floor);
        Ok(())
    }
    pub fn resolve(&mut self, at: Option<&Position>, action: &'static str) -> Result<Position, ServingError> {
        let sequence = match at {
            None => self.sequence,
            Some(at) if at.history.as_ref() == Some(&self.identity) && self.issued.contains(&at.sequence) && at.sequence >= self.floor => {
                at.sequence
            }
            Some(_) => return Err(failure(action, "position is foreign, expired or was not issued")),
        };
        Ok(self.issue(sequence))
    }
    pub fn page(
        &mut self, at: Option<&Position>, page: &PageRequest, action: &'static str,
    ) -> Result<(Vec<Event>, Position, String), ServingError> {
        if page.page_size == 0 || page.max_bytes == 0 {
            return Err(failure(action, "positive page_size and max_bytes are required"));
        }
        let (boundary, after) = if page.page_token.is_empty() {
            (self.resolve(at, action)?, self.floor)
        } else {
            if !self.tokens.contains(&page.page_token) {
                return Err(failure(action, "page token is expired or mismatched"));
            }
            let (_, token) = page.page_token.split_once('/').ok_or_else(|| failure(action, "invalid page token"))?;
            let (boundary, after) = token.split_once(':').ok_or_else(|| failure(action, "invalid page token"))?;
            let boundary = boundary.parse().map_err(|_| failure(action, "invalid page token"))?;
            let after = after.parse().map_err(|_| failure(action, "invalid page token"))?;
            let position = Position {
                history: Some(self.identity.clone()),
                sequence: boundary,
            };
            if at.is_some_and(|at| at != &position) {
                return Err(failure(action, "page token does not match position"));
            }
            (self.resolve(Some(&position), action)?, after)
        };
        let mut events = self.range(after, boundary.sequence, page.page_size as usize, page.max_bytes as usize, action)?;
        let token = loop {
            let last = events.last().map_or(after, |event| event.sequence);
            let more = self.events.iter().any(|event| event.sequence > last && event.sequence <= boundary.sequence);
            let token = if more { format!("{}/{}:{last}", self.identity.store, boundary.sequence) } else { String::new() };
            let response = harmos_protocol::status::LogsListResponse {
                events: events.clone(),
                resolved_at: Some(boundary.clone()),
                next_page_token: token.clone(),
            };
            if response.encoded_len() as u64 <= page.max_bytes {
                break token;
            }
            events.pop();
            if events.is_empty() {
                return Err(failure(action, "next event and continuation exceed response byte bound"));
            }
        };
        if !token.is_empty() && !self.tokens.contains(&token) {
            self.tokens.push_back(token.clone());
        }
        while self.tokens.len() > self.read_leases {
            self.tokens.pop_front();
        }
        Ok((events, boundary, token))
    }
    pub fn range(
        &self, after: u64, boundary: u64, maximum_items: usize, maximum_bytes: usize, action: &'static str,
    ) -> Result<Vec<Event>, ServingError> {
        if after < self.floor {
            return Err(failure(action, "history gap: resynchronize with List"));
        }
        if maximum_items == 0 || maximum_bytes == 0 {
            return Err(failure(action, "positive event and byte bounds are required"));
        }
        let mut bytes = harmos_protocol::status::LogsWatchResponse {
            events: Vec::new(),
            resolved_at: Some(Position {
                history: Some(self.identity.clone()),
                sequence: boundary,
            }),
        }
        .encoded_len();
        if bytes > maximum_bytes {
            return Err(failure(action, "selection metadata exceeds response byte bound"));
        }
        let mut events = Vec::new();
        for event in self.events.iter().filter(|event| event.sequence > after && event.sequence <= boundary) {
            let size = harmos_protocol::status::LogsWatchResponse {
                events: vec![event.clone()],
                resolved_at: None,
            }
            .encoded_len();
            if events.len() == maximum_items || bytes.saturating_add(size) > maximum_bytes {
                if events.is_empty() {
                    return Err(failure(action, "next event exceeds response byte bound"));
                }
                break;
            }
            bytes += size;
            events.push(event.clone());
        }
        Ok(events)
    }
    pub fn issue(&mut self, sequence: u64) -> Position {
        self.issued.insert(sequence);
        while self.issued.len() > self.read_leases {
            let expired = self.issued.iter().copied().find(|issued| *issued != sequence).expect("another issued position exists");
            self.issued.remove(&expired);
        }
        Position {
            history: Some(self.identity.clone()),
            sequence,
        }
    }
    pub fn latest(&mut self) -> Position {
        self.resolve(None, "Status.Watch").expect("latest history is retained")
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
