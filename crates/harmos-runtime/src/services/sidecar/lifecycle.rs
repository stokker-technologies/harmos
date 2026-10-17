/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use super::*;

/* === Implementations === */
impl<A, S, H> Resident<A> for Supervised<A, S, H>
where
    A: Application,
    S: Attach,
    H: Fn(&Service<A>, Notice) + Send + Sync + 'static,
{
    type Error = String;

    async fn run(&mut self, context: Service<A>) -> Result<(), Fault<String>> {
        let result = self.attempt(context).await;
        // An application fault is terminal: a caller awaiting staging learns the attempt was refused.
        if let Err(Fault::Application(_)) = &result {
            self.calls.refuse();
        }
        result
    }

    async fn stop(&mut self) -> Result<(), String> {
        self.calls.close();
        if let Some(cleanup) = self.cleanup.get_mut().as_mut() {
            cleanup.finish().await?;
        }
        Ok(())
    }

    async fn cleanup(&mut self) -> Result<(), String> {
        self.cleanup.get_mut().take();
        Ok(())
    }
}

impl<A, S, H> Supervised<A, S, H>
where
    A: Application,
    S: Attach,
    H: Fn(&Service<A>, Notice) + Send + Sync + 'static,
{
    // One physical attempt; its Attach guard closes the call surface when it ends.
    async fn attempt(&mut self, mut context: Service<A>) -> Result<(), Fault<String>> {
        self.calls.begin();
        let _attempt = Attempt(&self.calls);
        if context.is_stopping() {
            return Ok(());
        }
        if self.cleanup.get_mut().is_some() {
            return Err(Fault::Application("native physical cleanup is still pending".into()));
        }
        let instance = self
            .routing
            .as_ref()
            .map(|router| router.enter(self.attach.owner()))
            .transpose()
            .map_err(|error| Fault::Application(error.to_string()))?;
        let declarations = self.attach.publications();
        let observers = self.attach.observers();
        let operations = self.attach.operations().map_err(|error| Fault::Application(error.to_string()))?;
        harmos_sidecar::inventory::operations::validate(&operations).map_err(|error| Fault::Application(error.to_string()))?;
        let contracts = self.attach.operation_contracts().map_err(|error| Fault::Application(error.to_string()))?;
        if let Some(contracts) = &contracts {
            let mut ids = std::collections::BTreeSet::new();
            if contracts.len() != operations.len()
                || contracts.iter().any(|contract| {
                    !ids.insert(contract.operation().id.clone()) || !operations.iter().any(|operation| operation == contract.operation())
                })
            {
                return Err(Fault::Application("native canonical contracts differ from the admitted operations".into()));
            }
        }
        let contracts = contracts.unwrap_or_default();
        let sdk = self.attach.sdk_lifecycle().map_err(|error| Fault::Application(error.to_string()))?;
        let scope = instance.as_ref().and_then(|endpoint| endpoint.scope());
        if sdk.is_none() || scope.is_none() {
            return Err(Fault::Application("native SDK transport requires the original manager-issued scope".to_owned()));
        }
        if sdk.as_ref() != scope.and_then(|scope| scope.lifecycle()) {
            return Err(Fault::Application("native SDK pair differs from the original admitted generation".to_owned()));
        }
        let control = self.attach.observe_contract().map_err(|error| Fault::Application(error.to_string()))?;
        let mut required = self.attach.required_contracts().map_err(|error| Fault::Application(error.to_string()))?;
        if let Some(required) = &mut required {
            required.sort_by(|a, b| a.contract.operation().id.cmp(&b.contract.operation().id));
        }
        if required.as_deref() != scope.and_then(|scope| scope.required_contracts()) {
            return Err(Fault::Application("native required contracts differ from the original admitted generation".to_owned()));
        }
        if control.as_ref() != scope.and_then(|scope| scope.observe_contract())
            || scope.is_some_and(|scope| normalized_observers(scope.observers().to_vec()) != normalized_observers(observers.clone()))
        {
            return Err(Fault::Application("native observer roles or inventory differ from the original admitted generation".to_owned()));
        }
        harmos_sidecar::inventory::observers::validate(&observers, &declarations).map_err(|error| Fault::Application(error.to_string()))?;
        let plugin = instance.as_ref().and_then(|endpoint| endpoint.scope()).cloned();
        if let Some(scope) = &plugin {
            scope.validate_operations(operations.iter()).map_err(|error| Fault::Application(error.to_string()))?;
        }
        *self.cleanup.get_mut() = Some(Cleanup {
            instance,
            registration: None,
            admitted: BTreeMap::new(),
            session: None,
            readers: Vec::new(),
            failed_readers: Vec::new(),
            routed_tasks: BTreeMap::new(),
            child_scopes: BTreeMap::new(),
            endpoint: None,
            logical_failure: None,
            disposition: None,
            ended: None,
            failure: None,
        });
        let cleanup = self.cleanup.get_mut().as_mut().expect("the attempt owns its cleanup");
        let admitted: BTreeMap<String, Arc<crate::streams::PublicationLease>> = executor::timeout(self.handshake, async {
            let scope = plugin.as_ref().expect("SDK transport retains its original admitted scope");
            let registration =
                context.streams().stage_plugin(scope, declarations.clone()).await.map_err(|error| Fault::Application(error.to_string()))?;
            let leases = registration.leases().to_vec();
            cleanup.registration = Some(registration);
            Ok::<_, Fault<String>>(declarations.iter().zip(leases).map(|(declaration, lease)| (declaration.id.clone(), lease)).collect())
        })
        .await
        .map_err(|_| Fault::Application("stream admission exceeded the handshake deadline".into()))??;

        // Retain the reservation through physical attempt settlement.
        cleanup.admitted = admitted.clone();
        let attached =
            self.attach.attach().await.map_err(|error| Fault::Environmental(format!("the sidecar could not be started: {error}")))?;

        let result = converse(
            attached,
            &mut context,
            &*self.notice,
            &self.ingress,
            Settling {
                admitted: &admitted,
                demand_keys: &self.demand_keys,
                handshake: self.handshake,
                manager_owned_stop: self.manager_owned_stop,
                expected: self.expected_registrations.as_deref().map(Vec::as_slice),
                publications: &declarations,
                observers: &observers,
                operations: &operations,
                contracts: &contracts,
            },
            &self.calls,
            cleanup,
        )
        .await;
        cleanup.disposition = Some(result);
        let settled = cleanup.finish().await;
        let disposition = cleanup.disposition.take().expect("the attempt retained its disposition");
        disposition?;
        settled.map_err(Fault::Application)?;
        if let Some(failure) = &cleanup.logical_failure {
            return Err(Fault::Environmental(failure.clone()));
        }
        Ok(())
    }
}
