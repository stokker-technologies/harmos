/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Plugin Owner Admission And Authority

use super::*;
use harmos_runtime::operations::{OperationDeclaration, Registration, WireDeclaration};

type PublicBackends = PluginBackend<PublicActivation, PublicActivation, PublicActivation>;

struct PublicHost {
    operations: OperationManager<Lanes>,
}

#[derive(Default)]
struct PublicActivation {
    endpoint: Option<PluginEndpoint>,
    registration: Option<PluginRegistration>,
}

#[harmos_runtime::operation(id = "owners.public", visibility = Public, policy = harmos_runtime::builtins::host::POLICY,
    grant = |request: &RootPublic| Some(harmos_domain::plugin::Grant::of("owners.public", if request.restricted { "restricted" } else { "allowed" })))]
struct RootPublic {
    restricted: bool,
}

#[harmos_runtime::operation(id = "owners.host_only", visibility = Host, policy = harmos_runtime::builtins::host::POLICY)]
struct RootHost;

#[harmos_runtime::operation(id = "owners.private", policy = harmos_runtime::builtins::host::POLICY)]
struct RootPrivate;

#[harmos_runtime::operation(id = "owners.call", operations = [RootPublic], policy = harmos_runtime::builtins::host::POLICY)]
struct PublicCaller {
    restricted: bool,
}

#[harmos_runtime::operation(id = "owners.restricted", operations = [RootHost], policy = harmos_runtime::builtins::host::POLICY)]
struct RestrictedCaller;

#[harmos_runtime::operation(id = "owners.private_call", operations = [RootPrivate], policy = harmos_runtime::builtins::host::POLICY)]
struct PrivateCaller;

impl PluginLifecycle for PublicHost {
    type Guest = PublicActivation;
    type Sidecar = PublicActivation;
    type Embedded = PublicActivation;

    fn preflight(&mut self, candidate: &PluginDefinitions) -> Result<()> {
        for (_, required) in candidate.host_requirements() {
            assert_eq!(required.contract, WireDeclaration::of::<RootPublic>().contract().unwrap());
            assert_eq!(required.grants, [harmos_domain::plugin::Grant::of("owners.public", "allowed")]);
        }
        self.operations.validate_requirements(candidate)
    }

    fn prepare(&mut self, _: &PluginDefinition) -> Result<PublicBackends> {
        Ok(PluginBackend::Embedded(PublicActivation::default()))
    }

    async fn fence(&mut self, _: PluginTransition, _: &[PluginDefinition]) -> Result<()> {
        Ok(())
    }

    async fn start(
        &mut self, definition: &PluginDefinition, router: &Router, _: &PluginLifecycleAuthority<'_>, backend: &mut PublicBackends,
    ) -> Result<()> {
        let PluginBackend::Embedded(owned) = backend else { unreachable!() };
        owned.endpoint = Some(router.enter(&definition.id).unwrap());
        let scope = owned.endpoint.as_ref().unwrap().scope().unwrap();
        owned.registration =
            Some(self.operations.stage_plugin(scope, |bindings| bindings.operations::<Operations<PublicCaller>>(Lanes::Open))?);
        Ok(())
    }

    async fn stop(_: &PluginDefinition, _: &PluginLifecycleAuthority<'_>, backend: &mut PublicBackends) -> Result<()> {
        let PluginBackend::Embedded(owned) = backend else { unreachable!() };
        if let Some(registration) = &mut owned.registration {
            registration.shutdown().await?;
        }
        owned.registration.take();
        owned.endpoint.take();
        Ok(())
    }
}

impl harmos_runtime::Operation for RootPublic {
    type Error = std::convert::Infallible;
    async fn run(self, _: harmos_runtime::Context<'_, Self>) -> std::result::Result<harmos_runtime::Response<()>, Self::Error> {
        Ok(().into())
    }
}

impl harmos_runtime::Operation for RootHost {
    type Error = std::convert::Infallible;
    async fn run(self, _: harmos_runtime::Context<'_, Self>) -> std::result::Result<harmos_runtime::Response<()>, Self::Error> {
        Ok(().into())
    }
}

impl harmos_runtime::Operation for RootPrivate {
    type Error = std::convert::Infallible;
    async fn run(self, _: harmos_runtime::Context<'_, Self>) -> std::result::Result<harmos_runtime::Response<()>, Self::Error> {
        Ok(().into())
    }
}

impl harmos_runtime::Operation for PublicCaller {
    type Error = harmos_runtime::operations::InvocationError;
    async fn run(self, ctx: harmos_runtime::Context<'_, Self>) -> std::result::Result<harmos_runtime::Response<()>, Self::Error> {
        ctx.operation(RootPublic { restricted: self.restricted }).await
    }
}

impl harmos_runtime::Operation for RestrictedCaller {
    type Error = std::convert::Infallible;
    async fn run(self, _: harmos_runtime::Context<'_, Self>) -> std::result::Result<harmos_runtime::Response<()>, Self::Error> {
        Ok(().into())
    }
}

impl harmos_runtime::Operation for PrivateCaller {
    type Error = std::convert::Infallible;
    async fn run(self, _: harmos_runtime::Context<'_, Self>) -> std::result::Result<harmos_runtime::Response<()>, Self::Error> {
        Ok(().into())
    }
}

#[tokio::test]
async fn host_owner_admission() {
    use harmos_domain::inventory::contracts::Required;

    for owner in ["host", "ordinary"] {
        let runtime = harmos_runtime::Runtime::<Fixture>::launch_with((), (), |setup| {
            setup.settings().operation_settings(harmos_runtime::operations::Settings {
                preparation_bytes: std::num::NonZeroUsize::new(4096),
                ..Default::default()
            });
        })
        .await
        .unwrap();
        let root = Registration::new(Lanes::Open).operation::<RootPublic>(RootPublic::ID).unwrap();
        runtime.register_batch(root).await.unwrap();
        let mut plugin = plan().ordered().iter().find(|definition| definition.id == "provider").unwrap().clone();
        plugin.id = owner.into();
        plugin.contributions = PluginContributions::default();
        plugin.contributions.required_contracts = Some(vec![Required {
            contract: WireDeclaration::of::<RootPublic>().contract().unwrap(),
            grants: vec![harmos_domain::plugin::Grant::of("owners.public", "allowed")],
        }]);
        plugin.contributions.operations = vec![PublicCaller::definition()];
        plugin.contributions.wire_contracts = vec![WireDeclaration::of::<PublicCaller>().contract().unwrap()];
        let candidate = PluginDefinitions::new([plugin.clone()], []).unwrap();
        let mut manager = PluginManager::new(candidate).unwrap();
        let mut host = PublicHost { operations: runtime.operations.clone() };
        manager.initialize(&mut host).await.unwrap();

        let target = format!("{owner}.{}", PublicCaller::ID);
        let request = harmos_runtime::Message::encode(&PublicCaller { restricted: false });
        let bytes = runtime.operations.call_encoded(&target, request).await.unwrap();
        assert!(<() as harmos_runtime::Message>::decode(&bytes).is_ok());
        let request = harmos_runtime::Message::encode(&PublicCaller { restricted: true });
        assert!(runtime.operations.call_encoded(&target, request).await.is_err());
        assert_eq!(manager.status(owner).unwrap().state, PluginState::Ready);
        assert!(PluginDefinitions::new([plugin.clone(), plugin], []).is_err());
        manager.shutdown(&mut host).await.unwrap();
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn owner_name_permissions() {
    for owner in ["host", "ordinary"] {
        let runtime = harmos_runtime::Runtime::<Fixture>::launch((), ()).await.unwrap();
        let root =
            Registration::new(Lanes::Open).operation::<RootHost>(RootHost::ID).unwrap().operation::<RootPrivate>(RootPrivate::ID).unwrap();
        runtime.register_batch(root).await.unwrap();
        let mut plugin = plan().ordered().iter().find(|definition| definition.id == "provider").unwrap().clone();
        plugin.id = owner.into();
        plugin.contributions = PluginContributions::default();
        runtime.plugin.register_embedded::<Operations<RestrictedCaller>, _>(plugin.clone(), Default::default(), Lanes::Open).await.unwrap();
        let duplicate = runtime.plugin.register_embedded::<Operations<RestrictedCaller>, _>(plugin, Default::default(), Lanes::Open).await;
        assert!(matches!(duplicate, Err(Error::AlreadyRegistered { .. })), "{owner}: {duplicate:?}");

        assert!(runtime.plugin.initialize().await.is_err(), "{owner}");
        assert_eq!(runtime.plugin.status(owner).unwrap().state, PluginState::Failed);
        assert!(runtime.operations.definition(RestrictedCaller::ID).is_err());
        runtime.operation(RootHost).run().await.unwrap();
        runtime.shutdown().await.unwrap();
    }
}

#[tokio::test]
async fn private_owner_permissions() {
    for owner in ["host", "ordinary"] {
        let runtime = harmos_runtime::Runtime::<Fixture>::launch((), ()).await.unwrap();
        let root = Registration::new(Lanes::Open).operation::<RootPrivate>(RootPrivate::ID).unwrap();
        runtime.register_batch(root).await.unwrap();
        let mut plugin = plan().ordered().iter().find(|definition| definition.id == "provider").unwrap().clone();
        plugin.id = owner.into();
        plugin.contributions = PluginContributions::default();
        runtime.plugin.register_embedded::<Operations<PrivateCaller>, _>(plugin, Default::default(), Lanes::Open).await.unwrap();

        assert!(runtime.plugin.initialize().await.is_err(), "{owner}");
        assert_eq!(runtime.plugin.status(owner).unwrap().state, PluginState::Failed);
        assert!(runtime.operations.definition(PrivateCaller::ID).is_err());
        runtime.operation(RootPrivate).run().await.unwrap();
        runtime.shutdown().await.unwrap();
    }
}
