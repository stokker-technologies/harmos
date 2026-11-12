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
use std::sync::Arc;

// Third Party Imports
use harmos_operations::execution::operations::ServingError;
use harmos_protocol::{
    access as wire,
    v1::{History, Position},
};
use prost::Message;
use sha2::{Digest, Sha256};

// Local Imports
use super::{Access, Pairing, failure, identifier};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Access {
    pub(super) async fn create_pairing(
        self: &Arc<Self>, request: wire::PairingsCreateRequest,
    ) -> Result<wire::PairingsCreateResponse, ServingError> {
        let action = "Access.Pairings.Create";
        let mut state = self.state.lock().await;
        self.loaded(&state, action).await?;
        if state.attached.is_none() {
            return Err(failure(action, "enable the listener before creating a pairing"));
        }
        if request.label.is_empty() || request.label.len() > self.settings.label_bytes.get() {
            return Err(failure(action, "label is empty or exceeds configured bound"));
        }
        if state.value.pairings.len() >= self.settings.pairings.get() {
            return Err(failure(action, "pairing capacity exceeded"));
        }
        let credential = identifier(action)?;
        let id = identifier(action)?;
        let mut candidate = state.value.clone();
        candidate.pairings.push(Pairing {
            id: id.clone(),
            label: request.label.clone(),
            credential_sha256: Sha256::digest(credential.as_bytes()).to_vec(),
        });
        self.persist(&state, &mut candidate, action).await?;
        self.publish(&mut state, candidate);
        Ok(wire::PairingsCreateResponse {
            pairing: Some(wire::Pairing { id, label: request.label }),
            endpoint: Self::endpoint(&state),
            pinned_certificate_pem: state.value.certificate_pem.clone(),
            credential,
        })
    }
    pub(super) async fn list_pairings(&self, request: wire::PairingsListRequest) -> Result<wire::PairingsListResponse, ServingError> {
        let action = "Access.Pairings.List";
        let mut state = self.state.lock().await;
        self.loaded(&state, action).await?;
        let page = request.page.ok_or_else(|| failure(action, "page bounds are required"))?;
        if page.page_size == 0 || page.max_bytes == 0 {
            return Err(failure(action, "positive page_size and max_bytes are required"));
        }
        let position = Position {
            history: Some(History {
                store: "access.pairings".into(),
                epoch: state.epoch.clone(),
            }),
            sequence: state.value.revision,
        };
        if request.at.as_ref().is_some_and(|at| at != &position || state.issued != Some(at.sequence)) {
            return Err(failure(action, "pairing position is foreign, expired or not issued"));
        }
        let start = if page.page_token.is_empty() {
            0
        } else {
            if !state.tokens.contains(&page.page_token) {
                return Err(failure(action, "pairing page token is expired or mismatched"));
            }
            page.page_token
                .rsplit_once(':')
                .and_then(|(_, n)| n.parse::<usize>().ok())
                .ok_or_else(|| failure(action, "invalid page token"))?
        };
        let mut bytes = wire::PairingsListResponse {
            resolved_at: Some(position.clone()),
            ..Default::default()
        }
        .encoded_len();
        if bytes as u64 > page.max_bytes {
            return Err(failure(action, "selection metadata exceeds response byte bound"));
        }
        let mut pairings = Vec::new();
        for pairing in state.value.pairings.iter().skip(start) {
            let pairing = wire::Pairing {
                id: pairing.id.clone(),
                label: pairing.label.clone(),
            };
            let size = wire::PairingsListResponse {
                pairings: vec![pairing.clone()],
                ..Default::default()
            }
            .encoded_len();
            if pairings.len() == page.page_size as usize || bytes.saturating_add(size) > page.max_bytes as usize {
                if pairings.is_empty() {
                    return Err(failure(action, "next pairing exceeds response byte bound"));
                }
                break;
            }
            bytes += size;
            pairings.push(pairing);
        }
        let token = loop {
            let next = start + pairings.len();
            let token = if next < state.value.pairings.len() { format!("{}:{next}", state.value.revision) } else { String::new() };
            let response = wire::PairingsListResponse {
                pairings: pairings.clone(),
                resolved_at: Some(position.clone()),
                next_page_token: token.clone(),
            };
            if response.encoded_len() as u64 <= page.max_bytes {
                break token;
            }
            pairings.pop();
            if pairings.is_empty() {
                return Err(failure(action, "next pairing and continuation exceed response byte bound"));
            }
        };
        if !token.is_empty() {
            state.tokens.insert(token.clone());
            while state.tokens.len() > self.settings.read_leases.get() {
                state.tokens.pop_first();
            }
        }
        state.issued = Some(position.sequence);
        Ok(wire::PairingsListResponse {
            pairings,
            resolved_at: Some(position),
            next_page_token: token,
        })
    }
    pub(super) async fn delete_pairing(&self, request: wire::PairingsDeleteRequest) -> Result<wire::PairingsDeleteResponse, ServingError> {
        let action = "Access.Pairings.Delete";
        let mut state = self.state.lock().await;
        self.loaded(&state, action).await?;
        let mut candidate = state.value.clone();
        let original = candidate.pairings.len();
        candidate.pairings.retain(|p| p.id != request.name);
        if candidate.pairings.len() == original {
            return Err(failure(action, "pairing does not exist"));
        }
        self.persist(&state, &mut candidate, action).await?;
        self.publish(&mut state, candidate);
        Ok(wire::PairingsDeleteResponse {})
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
