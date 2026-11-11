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
use std::collections::BTreeMap;

// Third Party Imports

// Local Imports
use self::sheet::{Change, Erase, Fact, Noted, Rename, Sheet, Studio, Untitled};
use crate::application::support;
use harmos_runtime::{Error, Runtime, runtime::Registration, state::Focus, state::Kind, state::Position, state::Transaction};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

#[tokio::test]
async fn product_commits_refuses() {
    let runtime = studio().await;
    let handle = runtime.state.as_principal(support::alice());

    // A leaf-typed commit, as the target API spells it: the catalog aggregates
    // into the application's own error, so the extraction is the reflexive one.
    let first = handle.commit(sheet::rename("draft")).await.expect("accepted");
    assert_eq!(title(&runtime).await, "draft");

    let refused = handle.commit(Rename { to: String::new() }).await;
    assert!(matches!(refused, Err(Error::Rejected(Untitled))));
    assert_eq!(runtime.state.applied(), first.position);

    // The Catalog Infers Reversibility From The Invertible Implementation.
    handle.undo(support::alice(), Focus::all()).await.expect("a rename comes back");
    assert_eq!(title(&runtime).await, "origin");

    handle.commit(Erase).await.expect("accepted");
    assert!(
        matches!(handle.undo(support::alice(), Focus::all()).await, Err(Error::NothingToUndo)),
        "an erase has no Invertible implementation, so it is a barrier"
    );
}

#[tokio::test]
async fn catalog_declares_records() {
    {
        // The stamped identity is the permanent wire name, not the Rust type name.

        // The generated conversions and dispatch are ordinary Rust the application
        // could have written itself.
        let change: Change = Change::from(sheet::rename("named"));
        let mut state = Sheet::default();

        assert_eq!(change, Change::Rename(sheet::rename("named")));
        assert_eq!(Transaction::check(&change, &state), Ok(()));
        Transaction::apply(&change, &mut state);
        assert_eq!(state.title, "named");
        assert_eq!(
            harmos_runtime::state::Invertible::inverse(&sheet::rename("named"), &Sheet::default()),
            Rename { to: String::new() },
            "the declared body answers against the state before the change"
        );

        // The record catalog is the same shape minus behavior.
        let fact: Fact = Noted("exported".to_owned()).into();
        assert_eq!(fact, Fact::Noted(Noted("exported".to_owned())));
    }
    {
        let registered: BTreeMap<(String, u32), Kind> = definitions_of::<Studio>(Change::catalog())
            .into_iter()
            .chain(definitions_of::<Studio>(Fact::catalog()))
            .map(|(kind, id, version)| ((id.as_str().to_owned(), version), kind))
            .collect();

        assert_eq!(
            registered,
            BTreeMap::from([
                (("sheet.erase".to_owned(), <sheet::Erase as harmos_runtime::Identified>::definition().1), Kind::Transaction),
                (("sheet.noted".to_owned(), <Noted as harmos_runtime::Identified>::definition().1), Kind::Record),
                (("sheet.rename".to_owned(), <Rename as harmos_runtime::Identified>::definition().1), Kind::Transaction),
            ]),
            "one variant per current definition"
        );
    }
    {
        let runtime = studio().await;

        let noted = runtime.records.publish(Noted("exported".to_owned())).await.expect("a fact cannot be refused");
        let facts: Vec<Fact> =
            runtime.state.query::<Fact>(Position::ORIGIN).await.map(|entry| entry.expect("the tail holds it").value).collect();

        assert_eq!(noted.position, Position(1));
        assert_eq!(facts, vec![Fact::Noted(Noted("exported".to_owned()))]);
    }
}

#[tokio::test]
async fn derived_aggregate_writer() {
    use counter::{Counter, Down, Step, StepError, Tally, Underflow, Up};

    let runtime = Runtime::<Counter>::launch_with(Tally::default(), (), |setup| {
        setup.settings().register(Step::catalog());
    })
    .await
    .expect("a memory-only boot recovers nothing");

    // Catalog-typed commits are the shape every application can always write:
    // the aggregated error is the catalog's own, so the extraction is
    // reflexive.
    runtime.state.commit(Step::from(Up(10))).await.expect("accepted");
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
    assert_eq!(runtime.state.state().read(|tally| tally.total).await.expect("reads are open").value, 10);

    // `Up` and `Down` declare each other, so undoing one commits the other —
    // a pair across two definition types, checked mutual where each is
    // declared and derived through the table the registration carried.
    let handle = runtime.state.as_principal(support::alice());
    handle.commit(Step::from(Up(7))).await.expect("accepted");
    handle.undo(support::alice(), Focus::all()).await.expect("an up comes back as a down");

    assert_eq!(runtime.state.state().read(|tally| tally.total).await.expect("reads are open").value, 10);
}

fn definitions_of<A: harmos_runtime::Application>(catalog: impl Registration<A>) -> Vec<(Kind, harmos_runtime::DefinitionId, u32)> {
    match catalog.register() {
        harmos_runtime::runtime::Catalog::Transactions { definitions, .. } => definitions,
        harmos_runtime::runtime::Catalog::Streams(_) => {
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

    Runtime::<Studio>::launch_with(origin, (), |setup| {
        setup.settings().register(Change::catalog());
        setup.settings().register(Fact::catalog());
    })
    .await
    .expect("a memory-only boot recovers nothing")
}

async fn title(runtime: &Runtime<Studio>) -> String {
    runtime.state.state().read(|sheet| sheet.title.clone()).await.expect("reads are open").value
}

mod sheet {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports
    use serde::{Deserialize, Serialize};

    // Local Imports
    use harmos_runtime::{state::Invertible, state::Transaction};

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Sheet {
        pub title: String,
        pub applications: u64,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
    #[error("the title must not be empty")]
    pub struct Untitled;

    #[harmos_runtime::transaction(id = "sheet.rename")]
    #[derive(harmos_runtime::Described, Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct Rename {
        pub to: String,
    }

    #[harmos_runtime::transaction(id = "sheet.erase")]
    #[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct Erase;

    #[derive(harmos_runtime::Described, Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct Noted(pub String);

    #[derive(Clone, Copy, Debug)]
    pub struct Studio;

    /* === Enums === */

    #[harmos_runtime::transactions(state = Sheet, error = Untitled)]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Change {
        Rename(Rename),
        Erase(Erase),
    }

    #[harmos_runtime::records]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum Fact {
        Noted(Noted),
    }

    /* === Traits === */

    /* === Implementations === */

    impl harmos_runtime::state::Record for Noted {
        const ENTRY: harmos_runtime::state::RecordEntry<Self> = harmos_runtime::state::RecordEntry::new("sheet.noted");
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

    impl Invertible<Sheet> for Rename {
        type Inverse = Self;

        fn inverse(&self, before: &Sheet) -> Self::Inverse {
            Self { to: before.title.clone() }
        }
    }

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

    impl harmos_runtime::Lifecycle for Studio {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = harmos_runtime::Startup<Self>;

        async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
            Ok(Self)
        }
    }

    impl harmos_runtime::Application for Studio {
        type State = Sheet;
        type Transaction = Change;
        type Record = Fact;
    }

    /* === Public Functions === */

    pub fn rename(title: &str) -> Rename {
        Rename { to: title.to_owned() }
    }

    /* === Private Functions === */

    /* === Tests === */
}

mod counter {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports

    // Third Party Imports
    use serde::{Deserialize, Serialize};

    // Local Imports
    use harmos_runtime::{state::Invertible, state::Transaction};

    /* === Constants === */

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

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

    #[harmos_runtime::transaction(id = "tally.up",  inverse = Down)]
    #[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct Up(pub i64);

    #[harmos_runtime::transaction(id = "tally.down",  inverse = Up)]
    #[derive(harmos_runtime::Described, Clone, Copy, Debug, Deserialize, PartialEq, Eq, Serialize)]
    pub struct Down(pub i64);

    #[derive(Clone, Copy, Debug)]
    pub struct Counter;

    /* === Enums === */

    #[harmos_runtime::transactions(state = Tally)]
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Step {
        Up(Up),
        Down(Down),
    }

    /* === Traits === */

    /* === Implementations === */

    impl Transaction<Tally> for Up {
        type Error = NotPositive;

        fn check(&self, _tally: &Tally) -> Result<(), Self::Error> {
            (self.0 > 0).then_some(()).ok_or(NotPositive)
        }

        fn apply(&self, tally: &mut Tally) {
            tally.total += self.0;
        }
    }

    impl Invertible<Tally> for Up {
        type Inverse = Down;

        fn inverse(&self, _before: &Tally) -> Self::Inverse {
            Down(self.0)
        }
    }

    impl Transaction<Tally> for Down {
        type Error = Underflow;

        fn check(&self, tally: &Tally) -> Result<(), Self::Error> {
            (tally.total >= self.0).then_some(()).ok_or(Underflow)
        }

        fn apply(&self, tally: &mut Tally) {
            tally.total -= self.0;
        }
    }

    impl Invertible<Tally> for Down {
        type Inverse = Up;

        fn inverse(&self, _before: &Tally) -> Self::Inverse {
            Up(self.0)
        }
    }

    impl harmos_runtime::Lifecycle for Counter {
        type Config = ();
        type Error = std::convert::Infallible;
        type Context = harmos_runtime::Startup<Self>;

        async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
            Ok(Self)
        }
    }

    impl harmos_runtime::Application for Counter {
        type State = Tally;
        type Transaction = Step;
        type Record = ();
    }

    /* === Public Functions === */

    /* === Private Functions === */

    /* === Tests === */
}

#[cfg(feature = "plugin-guest")]
mod guest_plugins {
    /* === Modules === */

    /* === Re-Exports === */

    /* === Imports === */

    // Standard Library Imports
    use std::convert::Infallible;
    use std::time::Duration;

    // Third Party Imports

    // Local Imports
    use harmos_operations::lifecycle;
    use harmos_runtime::__private::{PluginDeclared, PluginInventory};
    use harmos_runtime::operations::{Contracts, OperationPolicy, RetryPolicy};

    /* === Constants === */

    const POLICY: OperationPolicy = harmos_runtime::OperationPolicy {
        timeout: Duration::from_secs(15),
        attempt_timeout: Duration::from_secs(5),
        cancellation_timeout: std::time::Duration::ZERO,
        retries: RetryPolicy::Never,
    };

    /* === Macros === */

    /* === Type Aliases === */

    /* === Structs === */

    #[harmos_runtime::operation(id = "fixture.unified.echo",  response = (), policy = POLICY)]
    struct Echo {}

    #[harmos_runtime::stream(id = "unified_rows", window = 4)]
    #[harmos_runtime::message]
    #[derive(Clone, PartialEq, serde::Deserialize, serde::Serialize)]
    struct Row {
        value: u32,
    }

    struct Guest;

    #[harmos_runtime::operation(codec = manual, resources = [], id = "fixture.unified.heard", response = (),
    policy = POLICY, visibility = Public)]
    #[derive(Clone)]
    struct RowCallback(harmos_runtime::listeners::Delivery<Row>);

    struct ListeningGuest;

    /* === Enums === */

    #[harmos_runtime::catalog(harmos_path = ::harmos_runtime)]
    enum Leaves {
        Echo(Echo),
    }

    #[harmos_runtime::catalog(harmos_path = ::harmos_runtime)]
    enum Selected {
        #[nested]
        Leaves(Leaves),
    }

    const AUDIENCE: &[harmos_runtime::catalog::Node<harmos_runtime::listeners::Entry>] =
        &[harmos_runtime::catalog::Node::Leaf(harmos_runtime::listeners::Entry::stream::<Row>(
            "fixture.row-listener",
            "unified_rows",
            harmos_runtime::operations::WireDeclaration::of::<RowCallback>(),
        ))];

    #[harmos_runtime::catalog(harmos_path = ::harmos_runtime)]
    enum Callbacks {
        Heard(RowCallback),
    }

    /* === Traits === */

    /* === Implementations === */

    impl harmos_runtime::operations::Operation for Echo {
        type Error = Infallible;
        async fn run(self, _: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Self::Error> {
            panic!("metadata inspection must not execute operations")
        }
    }

    impl lifecycle::Policy for Guest {
        const INITIALIZATION_POLICY: OperationPolicy = POLICY;
        const SHUTDOWN_POLICY: OperationPolicy = POLICY;
    }

    impl harmos_runtime::Lifecycle for Guest {
        type Config = ();
        type Error = Infallible;
        type Context = harmos_runtime::guest::kit::Context;

        async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> Result<Self, Self::Error> {
            Ok(Self)
        }
    }

    #[harmos_runtime::guest(
        id = "unified_guest",
        description = "Guest Authored Plugin",
        requires = (),
        streams(Row)
    )]
    impl harmos_runtime::guest::Plugin for Guest {
        const CATALOGS: &'static [harmos_runtime::Catalog] = &[Selected::CATALOG];
    }

    impl harmos_runtime::Message for RowCallback {
        const CODEC: &'static str = <harmos_runtime::listeners::Delivery<Row> as harmos_runtime::Message>::CODEC;
        fn encode(&self) -> Vec<u8> {
            harmos_runtime::Message::encode(&self.0)
        }
        fn encoded_size(&self) -> usize {
            harmos_runtime::Message::encoded_size(&self.0)
        }
        fn decoding_memory(bytes: &[u8]) -> Result<usize, harmos_runtime::MessageError> {
            <harmos_runtime::listeners::Delivery<Row> as harmos_runtime::Message>::decoding_memory(bytes)
        }
        fn decode(bytes: &[u8]) -> Result<Self, harmos_runtime::MessageError> {
            <harmos_runtime::listeners::Delivery<Row> as harmos_runtime::Message>::decode(bytes).map(Self)
        }
    }

    impl harmos_runtime::MemorySize for RowCallback {
        fn memory_size(&self) -> usize {
            harmos_runtime::MemorySize::memory_size(&self.0)
        }
    }

    impl harmos_runtime::schema::Described for RowCallback {
        const DEFINITION: harmos_runtime::schema::declaration::Definition =
            <harmos_runtime::listeners::Delivery<Row> as harmos_runtime::schema::Described>::DEFINITION;
        const NAME: &'static str = <harmos_runtime::listeners::Delivery<Row> as harmos_runtime::schema::Described>::NAME;
        const FIELDS: &'static [harmos_runtime::schema::Field] =
            <harmos_runtime::listeners::Delivery<Row> as harmos_runtime::schema::Described>::FIELDS;
        const MARKER: &'static [u8] = <harmos_runtime::listeners::Delivery<Row> as harmos_runtime::schema::Described>::MARKER;
    }

    impl harmos_runtime::operations::Operation for RowCallback {
        type Error = Infallible;
        async fn run(self, _: harmos_runtime::Context<'_, Self>) -> Result<harmos_runtime::operations::Response<()>, Self::Error> {
            panic!("metadata inspection must not execute callbacks")
        }
    }

    impl lifecycle::Policy for ListeningGuest {
        const INITIALIZATION_POLICY: OperationPolicy = POLICY;
        const SHUTDOWN_POLICY: OperationPolicy = POLICY;
    }

    impl harmos_runtime::Lifecycle for ListeningGuest {
        type Config = ();
        type Error = Infallible;
        type Context = harmos_runtime::guest::kit::Context;
        async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> Result<Self, Self::Error> {
            Ok(Self)
        }
    }

    #[harmos_runtime::guest(id = "unified_listening_guest", requires = (), listeners = AUDIENCE)]
    impl harmos_runtime::guest::Plugin for ListeningGuest {
        const CATALOGS: &'static [harmos_runtime::Catalog] = &[Callbacks::CATALOG];
    }

    /* === Public Functions === */

    /* === Private Functions === */

    fn inspect<P: PluginDeclared>(streams: usize, requires: usize) {
        assert_eq!(harmos_runtime::catalog::count(P::PROVIDES), 1);
        assert_eq!(harmos_runtime::catalog::count(P::REQUIRES), requires);
        assert_eq!(P::STREAMS.len(), streams);
        assert_eq!(P::STREAMS[0].id, "unified_rows");
        let mut bytes = Vec::new();
        for marker in P::markers().unwrap() {
            assert!(marker.starts_with(harmos_domain::PREFIX));
            bytes.extend_from_slice(marker);
        }
        let declaration = PluginInventory::Declaration::inspect(&bytes, PluginInventory::Facility::Guest).unwrap();
        assert_eq!(declaration.id, P::ID);
        assert_eq!(declaration.version.to_string(), env!("CARGO_PKG_VERSION"));
        assert_eq!(declaration.description, P::DESCRIPTION);
        assert!(!declaration.config.is_empty());
        let contracts = declaration.contracts;
        assert_eq!(contracts.provides, Contracts::of::<Selected>().unwrap());
        assert_eq!(contracts.requires.len(), requires);
        if requires != 0 {
            let required = contracts.requires.iter().map(|required| required.contract.clone()).collect::<Vec<_>>();
            assert_eq!(required, contracts.provides);
        }
        assert_eq!(contracts.lifecycle[0].action(), lifecycle::Action::Initialize);
        assert_eq!(contracts.lifecycle[1].action(), lifecycle::Action::Shutdown);
        assert_eq!(contracts.lifecycle[0].contract().operation().policy, POLICY);
        assert_eq!(contracts.lifecycle[1].contract().operation().policy, POLICY);
        assert_eq!(declaration.streams.len(), streams);

        let declaration =
            harmos_runtime::guest::inventory::Declaration::inspect(&bytes, harmos_runtime::guest::inventory::Facility::Guest).unwrap();
        assert_eq!(declaration.operations.len(), contracts.provides.len());
        assert_eq!(declaration.streams.len(), streams);
        let inspected = declaration.contracts;
        assert_eq!(inspected, contracts);
    }

    /* === Tests === */

    #[test]
    fn listener_inventory_contract() {
        let bytes = ListeningGuest::markers().unwrap().concat();
        let declaration =
            harmos_runtime::guest::inventory::Declaration::inspect(&bytes, harmos_runtime::guest::inventory::Facility::Guest).unwrap();
        assert_eq!(declaration.listeners, harmos_runtime::listeners::Declarations::from_tree(AUDIENCE).unwrap());
        assert_eq!(declaration.listeners[0].callback(), &declaration.contracts.provides[0]);
        let bytes = Guest::markers().unwrap().concat();
        let declaration =
            harmos_runtime::guest::inventory::Declaration::inspect(&bytes, harmos_runtime::guest::inventory::Facility::Guest).unwrap();
        assert!(declaration.listeners.is_empty());
        let mut parts = harmos_domain::artifacts::fragments::<serde_json::Value>(
            &ListeningGuest::markers().unwrap().concat(),
            &["contracts", "listeners", "plugin", "contributions", "publications", "observers", "schema"],
        )
        .unwrap();
        let bytes = |parts: &[serde_json::Value]| {
            let mut bytes = Vec::new();
            for part in parts {
                bytes.extend_from_slice(harmos_domain::PREFIX);
                bytes.extend_from_slice(&serde_json::to_vec(part).unwrap());
                bytes.push(0);
            }
            bytes
        };
        parts.iter_mut().find(|part| part["kind"] == "listeners").unwrap()["listeners"][0]["callback"]["version"] = 2.into();
        assert!(
            harmos_runtime::guest::inventory::Declaration::inspect(&bytes(&parts), harmos_runtime::guest::inventory::Facility::Guest)
                .is_err()
        );
        parts.iter_mut().find(|part| part["kind"] == "listeners").unwrap()["listeners"][0]["callback"]["version"] = 1.into();
        parts.retain(|part| part["kind"] != "contracts" || part["entry"]["direction"] != "provides");
        assert!(
            harmos_runtime::guest::inventory::Declaration::inspect(&bytes(&parts), harmos_runtime::guest::inventory::Facility::Guest)
                .is_err()
        );
    }

    #[test]
    fn context_selects_guest() {
        inspect::<Guest>(1, 0);
        assert_eq!(Guest::DESCRIPTION, "Guest Authored Plugin");
    }
}

/* === Tests === */

#[test]
fn catalog_projects_borrowed() {
    #[derive(harmos_runtime::Described, serde::Deserialize, serde::Serialize)]
    struct First(String);

    impl harmos_runtime::state::Record for First {
        const ENTRY: harmos_runtime::state::RecordEntry<Self> = harmos_runtime::state::RecordEntry::new("projection.first");
    }

    #[derive(harmos_runtime::Described, serde::Deserialize, serde::Serialize)]
    struct Second(String);

    impl harmos_runtime::state::Record for Second {
        const ENTRY: harmos_runtime::state::RecordEntry<Self> = harmos_runtime::state::RecordEntry::new("projection.second");
    }

    #[harmos_runtime::records]
    enum Facts {
        First(First),
        Second(Second),
    }

    let first = Facts::First(First("first".to_owned()));
    let projected = harmos_runtime::state::RecordProjection::<First>::project(&first).unwrap();
    let Facts::First(original) = &first else { unreachable!("the selected record variant") };
    assert!(std::ptr::eq(projected, original));
    assert!(harmos_runtime::state::RecordProjection::<Second>::project(&first).is_none());

    let second = Facts::Second(Second("second".to_owned()));
    assert!(harmos_runtime::state::RecordProjection::<First>::project(&second).is_none());
    assert_eq!(harmos_runtime::state::RecordProjection::<Second>::project(&second).unwrap().0, "second",);

    let single = sheet::Fact::Noted(sheet::Noted("single".to_owned()));
    assert_eq!(harmos_runtime::state::RecordProjection::<sheet::Noted>::project(&single).unwrap().0, "single",);
}
