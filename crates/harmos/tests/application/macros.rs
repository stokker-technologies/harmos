/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

mod sheet {
    use harmos::{state::Invert, state::Transaction};
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Sheet {
        pub title: String,
        pub applications: u64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
    #[error("the title must not be empty")]
    pub struct Untitled;

    #[harmos::transaction(version = 1)]
    #[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct RenameV1 {
        pub to: String,
    }

    impl Transaction<Sheet> for RenameV1 {
        type Error = Untitled;

        fn check(&self, _sheet: &Sheet) -> Result<(), Self::Error> {
            Ok(())
        }

        fn apply(&self, sheet: &mut Sheet) {
            sheet.title.clone_from(&self.to);
            sheet.applications += 1;
        }
    }

    impl From<RenameV1> for Rename {
        fn from(old: RenameV1) -> Self {
            Self { to: old.to }
        }
    }

    #[harmos::transaction(id = "sheet.rename", version = 2, inverse = Self, history(RenameV1))]
    #[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct Rename {
        pub to: String,
    }

    impl Transaction<Sheet> for Rename {
        type Error = Untitled;

        fn check(&self, _sheet: &Sheet) -> Result<(), Self::Error> {
            (!self.to.is_empty()).then_some(()).ok_or(Untitled)
        }

        fn apply(&self, sheet: &mut Sheet) {
            sheet.title.clone_from(&self.to);
            sheet.applications += 1;
        }
    }

    impl Invert<Sheet> for Rename {
        fn invert(&self, before: &Sheet) -> Self {
            Self {
                to: before.title.clone(),
            }
        }
    }

    #[harmos::transaction(id = "sheet.erase", version = 1, irreversible)]
    #[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct Erase;

    impl Transaction<Sheet> for Erase {
        type Error = Untitled;

        fn check(&self, _sheet: &Sheet) -> Result<(), Self::Error> {
            Ok(())
        }

        fn apply(&self, sheet: &mut Sheet) {
            sheet.title.clear();
            sheet.applications += 1;
        }
    }

    #[harmos::record(id = "sheet.noted", version = 1)]
    #[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct Noted(pub String);

    #[harmos::transactions(state = Sheet, error = Untitled)]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Change {
        Rename(Rename),
        Erase(Erase),
    }

    #[harmos::records]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Fact {
        Noted(Noted),
    }

    #[derive(Clone, Copy, Debug)]
    pub struct Studio;

    impl harmos::Application for Studio {
        type State = Sheet;
        type Transaction = Change;
        type Record = Fact;
        type Streams = ();
        type Resources = ();
    }

    pub fn rename(title: &str) -> Rename {
        Rename {
            to: title.to_owned(),
        }
    }
}

mod counter {
    use harmos::{state::Invert, state::Transaction};
    use serde::{Deserialize, Serialize};

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Tally {
        pub total: i64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
    #[error("the step must be positive")]
    pub struct NotPositive;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
    #[error("the tally would go negative")]
    pub struct Underflow;

    #[harmos::transaction(id = "tally.up", version = 1, inverse = Down)]
    #[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct Up(pub i64);

    impl Transaction<Tally> for Up {
        type Error = NotPositive;

        fn check(&self, _tally: &Tally) -> Result<(), Self::Error> {
            (self.0 > 0).then_some(()).ok_or(NotPositive)
        }

        fn apply(&self, tally: &mut Tally) {
            tally.total += self.0;
        }
    }

    impl Invert<Tally> for Up {
        fn invert(&self, _before: &Tally) -> Down {
            Down(self.0)
        }
    }

    #[harmos::transaction(id = "tally.down", version = 1, inverse = Up)]
    #[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct Down(pub i64);

    impl Transaction<Tally> for Down {
        type Error = Underflow;

        fn check(&self, tally: &Tally) -> Result<(), Self::Error> {
            (tally.total >= self.0).then_some(()).ok_or(Underflow)
        }

        fn apply(&self, tally: &mut Tally) {
            tally.total -= self.0;
        }
    }

    impl Invert<Tally> for Down {
        fn invert(&self, _before: &Tally) -> Up {
            Up(self.0)
        }
    }

    #[harmos::transactions(state = Tally)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Step {
        Up(Up),
        Down(Down),
    }

    #[derive(Clone, Copy, Debug)]
    pub struct Counter;

    impl harmos::Application for Counter {
        type State = Tally;
        type Transaction = Step;
        type Record = ();
        type Streams = ();
        type Resources = ();
    }
}

#[cfg(all(feature = "plugin-sidecar", feature = "plugin-guest"))]
mod unified_plugins {
    use std::convert::Infallible;
    use std::time::Duration;

    use harmos::__private::{PluginDeclared, PluginInventory};
    use harmos::operations::{
        Contracts, OperationDeclaration, OperationPolicy, RestartPolicy, RetryPolicy, ServingError,
        WireOperation, WireTable, lifecycle,
    };

    struct Echo;
    impl OperationDeclaration for Echo {
        type Request = ();
        type Response = ();
        const ID: &'static str = "fixture.unified.echo";
        const POLICY: OperationPolicy = POLICY;
    }
    impl WireOperation for Echo {
        const VERSION: u32 = 3;
    }

    #[harmos::catalog]
    enum Leaves {
        Echo(Echo),
    }
    #[harmos::catalog]
    enum Selected {
        #[nested]
        Leaves(Leaves),
    }

    #[harmos::stream(id = "unified_rows", window = 4)]
    #[harmos::message]
    #[derive(Clone, PartialEq, serde::Deserialize, serde::Serialize)]
    struct Row {
        value: u32,
    }
    #[harmos::stream(id = "unified_extra", window = 2)]
    #[harmos::message]
    #[derive(Clone, PartialEq, serde::Deserialize, serde::Serialize)]
    struct Extra {
        value: u64,
    }

    const PROJECTED: &[PluginInventory::publications::Entry] =
        &[PluginInventory::publications::Entry::of::<Extra>()];
    const POLICY: OperationPolicy = OperationPolicy {
        timeout: Duration::from_secs(15),
        attempt_timeout: Duration::from_secs(5),
        retries: RetryPolicy::Never,
        restart: RestartPolicy::Never,
    };

    struct Native;
    impl lifecycle::Policy for Native {
        const INITIALIZATION_POLICY: OperationPolicy = POLICY;
        const SHUTDOWN_POLICY: OperationPolicy = POLICY;
    }

    #[harmos::plugin(description = "Native Authored Plugin",
        id = "unified-native",
        provides = Selected,
        requires = Selected,
        streams(Row),
        stream_entries = PROJECTED
    )]
    impl harmos::Plugin for Native {
        type Config = ();
        type Error = Infallible;
        type Context = harmos::sidecar::Supervisor;
        type Resources = ();
        fn resources(&self) -> std::sync::Arc<()> {
            std::sync::Arc::new(())
        }

        fn prepare(_: ()) -> Result<Self, Self::Error> {
            Ok(Self)
        }
        async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
            Ok(())
        }
        fn bind_operations(&self, _: &mut WireTable<Self::Context>) -> Result<(), ServingError> {
            Ok(())
        }
        async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn cleanup(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct ObserverObserve;
    impl OperationDeclaration for ObserverObserve {
        type Request = harmos::operations::observers::Control;
        type Response = ();
        const ID: &'static str = "fixture.unified.observer-observe";
        const POLICY: OperationPolicy = harmos::operations::observers::POLICY;
        const VISIBILITY: harmos::operations::Visibility = harmos::operations::Visibility::Host;
    }
    impl WireOperation for ObserverObserve {
        const VERSION: u32 = 2;
    }
    #[harmos::catalog]
    enum ObserverOperations {
        #[nested]
        Selected(Selected),
        Observe(ObserverObserve),
    }

    struct CustomHost;
    impl harmos::sidecar::ObserverHost<()> for CustomHost {
        fn bind(_: std::sync::Arc<()>, _: harmos::sidecar::Supervisor) -> Self {
            Self
        }
    }
    struct InertObserver;
    #[harmos::observer(id = "fixture.unified.inert-observer", interval_ms = 1000)]
    impl InertObserver {
        async fn initialize(_: &()) -> Result<Self, Infallible> {
            panic!("metadata inspection must not initialize callback resources")
        }
        async fn observe(
            &mut self,
            _: &harmos::sidecar::ObserverContext<'_, (), CustomHost>,
        ) -> Result<(), Infallible> {
            panic!("metadata inspection must not start a callback")
        }
    }
    #[harmos::catalog]
    enum ObserverLeaves {
        Inert(InertObserver),
    }
    #[harmos::catalog]
    enum ObserverCatalog {
        #[nested]
        Leaves(ObserverLeaves),
    }

    struct SdkAuthored;
    trait ProductPlugin {
        type Settings;
        fn prepare(settings: Self::Settings) -> Self;
    }

    impl lifecycle::Policy for SdkAuthored {
        const INITIALIZATION_POLICY: OperationPolicy = POLICY;
        const SHUTDOWN_POLICY: OperationPolicy = POLICY;
    }

    #[harmos::plugin(
        id = "unified-sdk-projection",
        description = "Inert Product SDK Declaration",
        provides = ObserverOperations,
        requires(
            harmos::operations::streams::Publish,
            harmos::operations::lifecycle::Status: scope("reading", "power")
        ),
        observers = ObserverCatalog,
        observer_control = ObserverObserve,
        context = harmos::sidecar::Supervisor,
        config = SdkInitialization,
        restart = "never"
    )]
    impl ProductPlugin for SdkAuthored {
        type Settings = String;
        fn prepare(_: String) -> Self {
            panic!("inert inspection must not prepare the product plugin")
        }
    }

    #[derive(Default, serde::Deserialize)]
    struct SdkInitialization {
        settings: String,
        snapshot: Vec<u8>,
    }

    #[test]
    fn contract_scenario() {
        {
            super::scenario_runtime::check("metadata_never_require", || {
                let _prepare: fn(String) -> SdkAuthored = <SdkAuthored as ProductPlugin>::prepare;
                let initialization = SdkInitialization::default();
                assert!(initialization.settings.is_empty());
                assert!(initialization.snapshot.is_empty());
                let markers = SdkAuthored::markers().unwrap();
                let declaration = harmos::sidecar::inventory::Declaration::inspect(
                    &markers.concat(),
                    harmos::sidecar::inventory::Facility::Sidecar,
                )
                .unwrap();
                assert_eq!(declaration.id, SdkAuthored::ID);
                let required = &declaration.contracts.requires;
                assert_eq!(required.len(), 2);
                assert_eq!(
            required[0].contract.operation().id.as_str(),
            <harmos::operations::streams::Publish as harmos::operations::OperationDeclaration>::ID
        );
                assert!(required[0].grants.is_empty());
                assert_eq!(
                    required[1].grants,
                    [
                        harmos_domain::plugin::Grant::of("host.operations.status", "reading"),
                        harmos_domain::plugin::Grant::of("host.operations.status", "power"),
                    ]
                );
                // A grant names the operation it permits and, for a scoped
                // operation, the stream; the retired kinded shape is refused.
                assert_eq!(
                    serde_json::to_string(&required[1].grants[0]).unwrap(),
                    r#"{"operation":"host.operations.status","scope":"reading"}"#
                );
                assert!(
                    serde_json::from_str::<harmos_domain::plugin::Grant>(r#"{"survey":"reading"}"#)
                        .is_err()
                );
                assert_eq!(declaration.config, "SdkInitialization");
                let provides = Contracts::of::<ObserverOperations>().unwrap();
                assert_eq!(declaration.contracts.provides, provides);
                assert_eq!(
                    declaration.restart().unwrap(),
                    harmos::sidecar::inventory::RestartPolicy {
                        credit: 0,
                        sustained_health_ms: 30_000,
                        backoff_base_ms: 250,
                        backoff_cap_ms: 10_000,
                    }
                );
                assert_eq!(declaration.observers.len(), 1);
                assert_eq!(
                    declaration.observers[0].id,
                    "fixture.unified.inert-observer"
                );
                let inert = <InertObserver as harmos_domain::inventory::observers::Declared>::ENTRY;
                let physical = <harmos::sidecar::__private::ObserverInventory<CustomHost> as harmos::catalog::Member<InertObserver>>::ENTRY;
                assert_eq!(inert.id, physical.id);
                assert_eq!(inert.interval, physical.interval);
                assert_eq!(inert.streams.len(), physical.streams.len());
                let control = declaration.observer_control().unwrap().clone();
                assert_eq!(control, provides[1]);
                assert_eq!(control.version(), 2);
            });
        }
        {
            super::scenario_runtime::check("contributions_carry_inert", || {
                use harmos_domain::inventory::{Declaration, Facility};
                let declaration = Declaration::inspect(
                    &Contributing::markers().unwrap().concat(),
                    Facility::Sidecar,
                )
                .unwrap();
                let namespaces = declaration
                    .sections
                    .iter()
                    .map(|section| section.namespace.as_ref())
                    .collect::<Vec<_>>();
                assert_eq!(namespaces, ["dokime.catalog", "studio.panels"]);
                let catalog = declaration.section("dokime.catalog").unwrap();
                assert_eq!(
                    catalog.decode::<serde_json::Value>().unwrap()["kinds"][0],
                    "bench"
                );
                // A Host That Does Not Own A Namespace Simply Never Asks For It
                assert!(declaration.section("unknown.host").is_none());
                let mut registrar = harmos_domain::Registrar::new();
                <Contributing as harmos::Plugin>::register(&mut registrar).unwrap();
                let registered = registrar
                    .finish()
                    .into_iter()
                    .map(|registration| (registration.namespace, registration.schema))
                    .collect::<Vec<_>>();
                assert_eq!(
                    registered,
                    [
                        ("dokime.catalog".to_owned(), 1),
                        ("studio.panels".to_owned(), 2)
                    ]
                );
                harmos::sidecar::PluginNative::<Contributing>::new(
                    std::num::NonZeroUsize::new(1).unwrap(),
                )
                .expect("the native owner accepts the inspected sections");
            });
        }
        {
            super::scenario_runtime::check("inventory_rejects_changed", || {
                let mut parts: Vec<serde_json::Value> = Listening::markers()
                    .unwrap()
                    .into_iter()
                    .map(|marker| {
                        serde_json::from_slice(
                            &marker[harmos_domain::PREFIX.len()..marker.len() - 1],
                        )
                        .unwrap()
                    })
                    .collect();
                let bytes = |parts: &[serde_json::Value]| {
                    let mut bytes = Vec::new();
                    for part in parts {
                        bytes.extend_from_slice(harmos_domain::PREFIX);
                        bytes.extend_from_slice(&serde_json::to_vec(part).unwrap());
                        bytes.push(0);
                    }
                    bytes
                };
                parts
                    .iter_mut()
                    .find(|part| part["kind"] == "listeners")
                    .unwrap()["listeners"][0]["callback"]["version"] = 5.into();
                assert!(
                    harmos::sidecar::inventory::Declaration::inspect(
                        &bytes(&parts),
                        harmos::sidecar::inventory::Facility::Sidecar
                    )
                    .is_err()
                );
                parts
                    .iter_mut()
                    .find(|part| part["kind"] == "listeners")
                    .unwrap()["listeners"][0]["callback"]["version"] = 4.into();
                parts
                    .iter_mut()
                    .find(|part| part["kind"] == "contracts")
                    .unwrap()["provides"] = serde_json::json!([]);
                assert!(
                    harmos::sidecar::inventory::Declaration::inspect(
                        &bytes(&parts),
                        harmos::sidecar::inventory::Facility::Sidecar
                    )
                    .is_err()
                );
            });
        }
        {
            super::scenario_runtime::check("listener_inventory_inert", || {
                let bytes: Vec<_> = Listening::markers()
                    .unwrap()
                    .into_iter()
                    .flatten()
                    .copied()
                    .collect();
                let declaration = harmos::sidecar::inventory::Declaration::inspect(
                    &bytes,
                    harmos::sidecar::inventory::Facility::Sidecar,
                )
                .unwrap();
                let selected = declaration.listeners;
                assert_eq!(
                    selected,
                    harmos::listeners::Declarations::of::<Audience>().unwrap()
                );
                assert_eq!(selected[0].callback(), &declaration.contracts.provides[0]);
                assert_eq!(selected[0].callback().version(), 4);
                let guest_markers: Vec<_> = ListeningGuest::markers()
                    .unwrap()
                    .into_iter()
                    .flatten()
                    .copied()
                    .collect();
                let guest = harmos::guest::inventory::Declaration::inspect(
                    &guest_markers,
                    harmos::guest::inventory::Facility::Guest,
                )
                .unwrap();
                assert_eq!(&guest.listeners, &selected);
                assert_eq!(guest.listeners[0].callback(), &guest.contracts.provides[0]);

                let guest_markers: Vec<_> = Guest::markers()
                    .unwrap()
                    .into_iter()
                    .flatten()
                    .copied()
                    .collect();
                assert_eq!(
                    harmos::guest::inventory::Declaration::inspect(
                        &guest_markers,
                        harmos::guest::inventory::Facility::Guest
                    )
                    .unwrap()
                    .listeners,
                    Vec::new()
                );
            });
        }
        {
            super::scenario_runtime::check("context_selects_native", || {
                inspect::<Native>("sidecar", 2, 1);
                let native = harmos::sidecar::PluginNative::<Native>::new(
                    std::num::NonZeroUsize::new(1).unwrap(),
                )
                .unwrap();
                assert_eq!(
                    harmos::sidecar::Attach::operation_contracts(&native)
                        .unwrap()
                        .unwrap(),
                    Contracts::of::<Selected>().unwrap()
                );
                let required = harmos::sidecar::Attach::required_contracts(&native)
                    .unwrap()
                    .unwrap()
                    .into_iter()
                    .map(|required| required.contract)
                    .collect::<Vec<_>>();
                assert_eq!(required, Contracts::of::<Selected>().unwrap());
                assert_eq!(Native::STREAMS[1].id, "unified_extra");
                assert_eq!(Native::DESCRIPTION, "Native Authored Plugin");
            });
        }
        {
            super::scenario_runtime::check("context_selects_guest", || {
                inspect::<Guest>("guest", 1, 0);
                assert_eq!(Guest::DESCRIPTION, "Guest Authored Plugin");
            });
        }
    }

    struct Guest;
    impl lifecycle::Policy for Guest {
        const INITIALIZATION_POLICY: OperationPolicy = POLICY;
        const SHUTDOWN_POLICY: OperationPolicy = POLICY;
    }

    #[harmos::plugin(
        id = "unified-guest",
        description = "Guest Authored Plugin",
        provides = Selected,
        requires = (),
        streams(Row)
    )]
    impl harmos::Plugin for Guest {
        type Config = ();
        type Error = Infallible;
        type Context = harmos::guest::Context;
        type Resources = ();
        fn resources(&self) -> std::sync::Arc<()> {
            std::sync::Arc::new(())
        }

        fn prepare(_: ()) -> Result<Self, Self::Error> {
            Ok(Self)
        }
        async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
            Ok(())
        }
        fn bind_operations(&self, _: &mut WireTable<Self::Context>) -> Result<(), ServingError> {
            Ok(())
        }
        async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn cleanup(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct Catalog;
    impl harmos_domain::plugin::Contribution for Catalog {
        const NAMESPACE: &'static str = "dokime.catalog";
        const SECTION: harmos_domain::inventory::Section =
            harmos_domain::inventory::Section::new(Self::NAMESPACE, r#"{"kinds":["bench"]}"#);
        fn register(
            registrar: &mut harmos_domain::Registrar,
        ) -> Result<(), harmos_domain::RegistrationError> {
            registrar.register(Self::NAMESPACE, 1, &["bench"])
        }
    }

    struct Panels;
    impl harmos_domain::plugin::Contribution for Panels {
        const NAMESPACE: &'static str = "studio.panels";
        const SECTION: harmos_domain::inventory::Section =
            harmos_domain::inventory::Section::new(Self::NAMESPACE, "[]");
        fn register(
            registrar: &mut harmos_domain::Registrar,
        ) -> Result<(), harmos_domain::RegistrationError> {
            registrar.register(Self::NAMESPACE, 2, &Vec::<String>::new())
        }
    }

    struct Contributing;
    impl lifecycle::Policy for Contributing {
        const INITIALIZATION_POLICY: OperationPolicy = POLICY;
        const SHUTDOWN_POLICY: OperationPolicy = POLICY;
    }

    #[harmos::plugin(
        id = "unified-contributing",
        provides = (),
        requires = (),
        contributions(Catalog, Panels)
    )]
    impl harmos::Plugin for Contributing {
        type Config = ();
        type Error = Infallible;
        type Context = harmos::sidecar::Supervisor;
        type Resources = ();
        fn resources(&self) -> std::sync::Arc<()> {
            std::sync::Arc::new(())
        }
        fn prepare(_: ()) -> Result<Self, Self::Error> {
            Ok(Self)
        }
        async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
            Ok(())
        }
        fn bind_operations(&self, _: &mut WireTable<Self::Context>) -> Result<(), ServingError> {
            Ok(())
        }
        async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn cleanup(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct Heard;
    impl OperationDeclaration for Heard {
        type Request = harmos::listeners::Delivery<Row>;
        type Response = ();
        const ID: &'static str = "fixture.unified.heard";
        const POLICY: OperationPolicy = POLICY;
        const VISIBILITY: harmos::operations::Visibility = harmos::operations::Visibility::Shared;
    }
    impl WireOperation for Heard {
        const VERSION: u32 = 4;
    }
    #[harmos::listener(id = "fixture.row-listener", stream = Row, callback = Heard)]
    struct RowListener;
    #[harmos::catalog]
    enum Audience {
        Row(RowListener),
    }
    #[harmos::catalog]
    enum Callbacks {
        Heard(Heard),
    }

    struct Listening;
    impl lifecycle::Policy for Listening {
        const INITIALIZATION_POLICY: OperationPolicy = POLICY;
        const SHUTDOWN_POLICY: OperationPolicy = POLICY;
    }
    #[harmos::plugin(id = "unified-listening", provides = Callbacks, requires = (), listeners = Audience)]
    impl harmos::Plugin for Listening {
        type Config = ();
        type Error = Infallible;
        type Context = harmos::sidecar::Supervisor;
        type Resources = ();
        fn resources(&self) -> std::sync::Arc<()> {
            std::sync::Arc::new(())
        }
        fn prepare(_: ()) -> Result<Self, Self::Error> {
            Ok(Self)
        }
        async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
            Ok(())
        }
        fn bind_operations(&self, _: &mut WireTable<Self::Context>) -> Result<(), ServingError> {
            Ok(())
        }
        async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn cleanup(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    struct ListeningGuest;
    impl lifecycle::Policy for ListeningGuest {
        const INITIALIZATION_POLICY: OperationPolicy = POLICY;
        const SHUTDOWN_POLICY: OperationPolicy = POLICY;
    }
    #[harmos::plugin(id = "unified-listening-guest", provides = Callbacks, requires = (), listeners = Audience)]
    impl harmos::Plugin for ListeningGuest {
        type Config = ();
        type Error = Infallible;
        type Context = harmos::guest::Context;
        type Resources = ();
        fn resources(&self) -> std::sync::Arc<()> {
            std::sync::Arc::new(())
        }
        fn prepare(_: ()) -> Result<Self, Self::Error> {
            Ok(Self)
        }
        async fn initialize(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
            Ok(())
        }
        fn bind_operations(&self, _: &mut WireTable<Self::Context>) -> Result<(), ServingError> {
            Ok(())
        }
        async fn stop(&mut self, _: &Self::Context) -> Result<(), Self::Error> {
            Ok(())
        }
        async fn cleanup(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    fn inspect<P: PluginDeclared>(facility: &str, streams: usize, requires: usize) {
        assert_eq!(harmos::catalog::count(P::PROVIDES), 1);
        assert_eq!(harmos::catalog::count(P::REQUIRES), requires);
        assert_eq!(P::STREAMS.len(), streams);
        assert_eq!(P::STREAMS[0].id, "unified_rows");
        let mut bytes = Vec::new();
        for marker in P::markers().unwrap() {
            assert!(marker.starts_with(harmos_domain::PREFIX));
            bytes.extend_from_slice(marker);
        }
        let declaration = PluginInventory::Declaration::inspect(
            &bytes,
            match facility {
                "sidecar" => PluginInventory::Facility::Sidecar,
                "guest" => PluginInventory::Facility::Guest,
                _ => unreachable!("fixture selects one supported carrier"),
            },
        )
        .unwrap();
        assert_eq!(declaration.id, P::ID);
        assert_eq!(declaration.version.to_string(), env!("CARGO_PKG_VERSION"));
        assert_eq!(declaration.description, P::DESCRIPTION);
        assert!(!declaration.config.is_empty());
        let contracts = declaration.contracts;
        assert_eq!(contracts.provides, Contracts::of::<Selected>().unwrap());
        assert_eq!(contracts.requires.len(), requires);
        if requires != 0 {
            let required = contracts
                .requires
                .iter()
                .map(|required| required.contract.clone())
                .collect::<Vec<_>>();
            assert_eq!(required, contracts.provides);
        }
        assert_eq!(
            contracts.lifecycle[0].action(),
            lifecycle::Action::Initialize
        );
        assert_eq!(contracts.lifecycle[1].action(), lifecycle::Action::Shutdown);
        assert_eq!(contracts.lifecycle[0].contract().operation().policy, POLICY);
        assert_eq!(contracts.lifecycle[1].contract().operation().policy, POLICY);
        assert_eq!(declaration.streams.len(), streams);

        let inspected = match facility {
            "sidecar" => {
                let declaration = harmos::sidecar::inventory::Declaration::inspect(
                    &bytes,
                    harmos::sidecar::inventory::Facility::Sidecar,
                )
                .unwrap();
                assert_eq!(declaration.restart(), None);
                assert_eq!(declaration.operations.len(), contracts.provides.len());
                assert_eq!(declaration.streams.len(), streams);
                declaration.contracts
            }
            "guest" => {
                let declaration = harmos::guest::inventory::Declaration::inspect(
                    &bytes,
                    harmos::guest::inventory::Facility::Guest,
                )
                .unwrap();
                assert_eq!(declaration.operations.len(), contracts.provides.len());
                assert_eq!(declaration.streams.len(), streams);
                declaration.contracts
            }
            _ => unreachable!("only actual SDK contexts are authored"),
        };
        assert_eq!(inspected, contracts);
    }
}

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
use std::collections::BTreeMap;
// Third Party Imports

// Local Imports
use self::sheet::{Change, Erase, Fact, Noted, Rename, Sheet, Studio, Untitled};
use crate::support;
use harmos::{
    Error, Runtime, runtime::Registration, state::Focus, state::Kind, state::Position,
    state::Transaction,
};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("product_commits_refuses", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = studio().await;
                    let handle = runtime.state.as_principal(support::alice());

                    // A leaf-typed commit, as the target API spells it: the catalog aggregates
                    // into the application's own error, so the extraction is the reflexive one.
                    let first = handle
                        .commit(sheet::rename("draft"))
                        .await
                        .expect("accepted");
                    assert_eq!(title(&runtime).await, "draft");

                    let refused = handle.commit(Rename { to: String::new() }).await;
                    assert!(matches!(refused, Err(Error::Rejected(Untitled))));
                    assert_eq!(runtime.state.applied(), first.position);

                    // The table the registration carries makes the change that declared an
                    // inverse undoable and the one that declared itself irreversible a
                    // barrier, exactly as the hand-written function does.
                    handle
                        .undo(support::alice(), Focus::all())
                        .await
                        .expect("a rename comes back");
                    assert_eq!(title(&runtime).await, "origin");

                    handle.commit(Erase).await.expect("accepted");
                    assert!(
                        matches!(
                            handle.undo(support::alice(), Focus::all()).await,
                            Err(Error::NothingToUndo)
                        ),
                        "an erase declared itself irreversible, so it is a barrier"
                    );
                });
        });
    }
    {
        scenario_runtime::check("macro_authored_catalog", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let registered: BTreeMap<(String, u32), Kind> =
                        definitions_of(Change::catalog::<Studio>())
                            .into_iter()
                            .chain(definitions_of(Fact::catalog::<Studio>()))
                            .map(|(kind, id, version)| ((id.as_str().to_owned(), version), kind))
                            .collect();

                    assert_eq!(
                        registered,
                        BTreeMap::from([
                            (("sheet.erase".to_owned(), 1), Kind::Transaction),
                            (("sheet.noted".to_owned(), 1), Kind::Record),
                            (("sheet.rename".to_owned(), 1), Kind::Transaction),
                            (("sheet.rename".to_owned(), 2), Kind::Transaction),
                        ]),
                        "one variant per definition, plus every version its history declares"
                    );
                });
        });
    }
    {
        scenario_runtime::check("runtime_ac001_generated", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    // The stamped identity is the permanent wire name, not the Rust type name.
                    assert_eq!(Rename::DEFINITION, (Kind::Transaction, "sheet.rename", 2));
                    assert_eq!(Rename::HISTORY, [(Kind::Transaction, "sheet.rename", 1)]);
                    assert!(
                        Erase::HISTORY.is_empty(),
                        "a definition with no earlier version declares none"
                    );
                    assert_eq!(sheet::RenameV1::VERSION, 1);

                    // The generated conversions and dispatch are ordinary Rust the application
                    // could have written itself.
                    let change: Change = Change::from(sheet::rename("named"));
                    let mut state = Sheet::default();

                    assert_eq!(change, Change::Rename(sheet::rename("named")));
                    assert_eq!(Transaction::check(&change, &state), Ok(()));
                    Transaction::apply(&change, &mut state);
                    assert_eq!(state.title, "named");
                    assert_eq!(
                        harmos::state::Invert::invert(&sheet::rename("named"), &Sheet::default()),
                        Rename { to: String::new() },
                        "the declared body answers against the state before the change"
                    );

                    // The record catalog is the same shape minus behavior.
                    let fact: Fact = Noted("exported".to_owned()).into();
                    assert_eq!(fact, Fact::Noted(Noted("exported".to_owned())));
                    assert_eq!(Noted::DEFINITION, (Kind::Record, "sheet.noted", 1));
                });
        });
    }
    {
        scenario_runtime::check("runtime_ac001_macro", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    let runtime = studio().await;

                    let noted = runtime
                        .records
                        .publish(Noted("exported".to_owned()))
                        .await
                        .expect("a fact cannot be refused");
                    let facts: Vec<Fact> = runtime
                        .state
                        .query::<Fact>(Position::ORIGIN)
                        .await
                        .map(|entry| entry.expect("the tail holds it").value)
                        .collect();

                    assert_eq!(noted.position, Position(1));
                    assert_eq!(facts, vec![Fact::Noted(Noted("exported".to_owned()))]);
                });
        });
    }
    {
        scenario_runtime::check("derived_aggregate_writer", || {
            let mut scenario_executor = tokio::runtime::Builder::new_current_thread();
            scenario_executor
                .enable_all()
                .build()
                .expect("scenario runtime is valid")
                .block_on(async {
                    use counter::{Counter, Down, Step, StepError, Tally, Underflow, Up};

                    let runtime = Runtime::<Counter>::builder(Tally::default(), ())
                        .register(Step::catalog())
                        .runtime()
                        .await
                        .expect("a memory-only boot recovers nothing");

                    // Catalog-typed commits are the shape every application can always write:
                    // the aggregated error is the catalog's own, so the extraction is
                    // reflexive.
                    runtime
                        .state
                        .commit(Step::from(Up(10)))
                        .await
                        .expect("accepted");
                    let refused = runtime.state.commit(Step::from(Down(40))).await;

                    assert!(
                        matches!(refused, Err(Error::Rejected(StepError::Down(Underflow)))),
                        "the refusal arrives in the definition's own vocabulary, wrapped by the \
         variant it came from: {refused:?}"
                    );
                    assert_eq!(
                        refused.unwrap_err().to_string(),
                        "the transaction was refused: the tally would go negative",
                        "the generated aggregation displays its variant transparently"
                    );
                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|tally| tally.total)
                            .await
                            .expect("reads are open")
                            .value,
                        10
                    );

                    // `Up` and `Down` declare each other, so undoing one commits the other —
                    // a pair across two definition types, checked mutual where each is
                    // declared and derived through the table the registration carried.
                    let handle = runtime.state.as_principal(support::alice());
                    handle.commit(Step::from(Up(7))).await.expect("accepted");
                    handle
                        .undo(support::alice(), Focus::all())
                        .await
                        .expect("an up comes back as a down");

                    assert_eq!(
                        runtime
                            .state
                            .state()
                            .read(|tally| tally.total)
                            .await
                            .expect("reads are open")
                            .value,
                        10
                    );
                });
        });
    }
    {
        scenario_runtime::check("catalog_projects_borrowed", || {
            #[harmos::record(id = "projection.first", version = 1)]
            #[derive(serde::Deserialize, serde::Serialize)]
            struct First(String);

            #[harmos::record(id = "projection.second", version = 1)]
            #[derive(serde::Deserialize, serde::Serialize)]
            struct Second(String);

            #[harmos::records]
            enum Facts {
                First(First),
                Second(Second),
            }

            let first = Facts::First(First("first".to_owned()));
            let projected = harmos::state::RecordProjection::<First>::project(&first).unwrap();
            let Facts::First(original) = &first else {
                unreachable!("the selected record variant")
            };
            assert!(std::ptr::eq(projected, original));
            assert!(harmos::state::RecordProjection::<Second>::project(&first).is_none());

            let second = Facts::Second(Second("second".to_owned()));
            assert!(harmos::state::RecordProjection::<First>::project(&second).is_none());
            assert_eq!(
                harmos::state::RecordProjection::<Second>::project(&second)
                    .unwrap()
                    .0,
                "second",
            );

            let single = sheet::Fact::Noted(sheet::Noted("single".to_owned()));
            assert_eq!(
                harmos::state::RecordProjection::<sheet::Noted>::project(&single)
                    .unwrap()
                    .0,
                "single",
            );
        });
    }
}

fn definitions_of<A: harmos::Application>(
    catalog: impl Registration<A>,
) -> Vec<(Kind, harmos::DefinitionId, u32)> {
    match catalog.register() {
        harmos::runtime::Catalog::Transactions { definitions, .. } => definitions,
        harmos::runtime::Catalog::Streams(_) | harmos::runtime::Catalog::Lanes(_) => {
            unreachable!("a transaction or fact catalog always registers as `Transactions`")
        }
    }
}

async fn studio() -> Runtime<Studio> {
    // A sheet that already has a name, so undoing back to it is a change the
    // application's own `check` accepts.
    let origin = Sheet {
        title: "origin".to_owned(),
        applications: 0,
    };

    Runtime::<Studio>::builder(origin, ())
        .register(Change::catalog())
        .register(Fact::catalog())
        .runtime()
        .await
        .expect("a memory-only boot recovers nothing")
}

async fn title(runtime: &Runtime<Studio>) -> String {
    runtime
        .state
        .state()
        .read(|sheet| sheet.title.clone())
        .await
        .expect("reads are open")
        .value
}

/* === Tests === */
