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
use harmos_protocol::v1::{History, Position, access as wire};
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
        let pairings = state
            .value
            .pairings
            .iter()
            .map(|pairing| wire::Pairing {
                id: pairing.id.clone(),
                label: pairing.label.clone(),
            })
            .collect();
        state.issued = Some(position.sequence);
        Ok(wire::PairingsListResponse { pairings, resolved_at: Some(position) })
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
