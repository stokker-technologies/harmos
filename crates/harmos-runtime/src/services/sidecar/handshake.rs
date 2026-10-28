/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

impl<A, H> Exchange<'_, A, H>
where
    A: Application,
    H: Fn(&ServiceContext<A>, Notice) + Send + Sync,
{
    pub(super) async fn open(&mut self, within: Duration) -> Result<(), String> {
        let owner = self.plugin.as_ref().ok_or_else(|| "sidecar has no assigned owner".to_owned())?.owner().to_owned();
        let encoded = self.open_call(Frame::REGISTER, owner.encode(), within).await?;
        let catalog = serde_json::from_slice::<harmos_sidecar::PluginCatalog>(&encoded)
            .map_err(|error| format!("the sidecar returned an invalid plugin catalog: {error}"))?;
        harmos_sidecar::inventory::observers::validate(&catalog.observers, self.publications).map_err(|error| error.to_string())?;
        if normalized_observers(catalog.observers) != normalized_observers(self.observers.to_vec()) {
            return Err("the sidecar observers differ from the admitted catalog".to_owned());
        }
        harmos_sidecar::inventory::operations::validate_match(self.operations, &catalog.operations)
            .map_err(|error| format!("the sidecar operations differ from the admitted catalog: {error}"))?;
        let mut registrations = catalog.registrations;
        if let Some(expected) = self.expected {
            registrations.sort_by(|left, right| left.namespace.cmp(&right.namespace));
            if registrations.windows(2).any(|pair| pair[0].namespace == pair[1].namespace) || registrations != expected {
                return Err("the sidecar registrations differ from the expected catalog".to_owned());
            }
        }
        self.registrations.send_replace(Some(registrations));
        Ok(())
    }

    pub(super) async fn open_call(
        &mut self, method: &str, params: Vec<u8>, within: Duration,
    ) -> Result<harmos_sidecar::FramePayload, String> {
        let id = self.mint();
        if let Err(error) = self
            .send(Frame::Call {
                id,
                method: method.to_owned(),
                params: params.into(),
            })
            .await
        {
            return Err(format!("the sidecar could not answer `{method}`: {error}"));
        }

        let opened = executor::timeout(within, async {
            loop {
                let Some(arrival) = self.arrival().await else {
                    return Err("the sidecar stopped speaking while opening".to_owned());
                };
                let frame = match &arrival {
                    Arrival::Frame(frame) => Some(frame),
                    Arrival::Binary(received) => Some(&received.frame),
                    _ => None,
                };
                let answered = match frame {
                    Some(Frame::Answer { id: at, result }) if *at == id => Some(result.clone()),
                    _ => None,
                };
                let refused = match frame {
                    Some(Frame::Refusal { id: at, refusal }) if *at == id => Some(refusal.message().to_owned()),
                    _ => None,
                };
                match self.absorb(arrival).await {
                    Absorbed::Ended => {
                        return Err("the sidecar exited while opening".to_owned());
                    }
                    Absorbed::Malformed(error) => return Err(error),
                    Absorbed::Carried => {}
                }
                if let Some(message) = refused {
                    return Err(format!("the sidecar refused `{method}`: {message}"));
                }
                if let Some(result) = answered {
                    return Ok(result);
                }
            }
        })
        .await;

        opened.unwrap_or_else(|_| Err("the sidecar did not open inside its handshake".to_owned()))
    }
}
