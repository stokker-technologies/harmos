/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use crate::state::support::{Bank, HOUSE, alice, deposit, runtime};
use harmos_runtime::{Application, Runtime, state::Focus, state::ManagedState, state::Never, state::Position};

struct State {
    value: i64,
}
struct Plain;

#[harmos_runtime::lanes]
pub enum PlainLanes {
    #[default]
    Open,
}
impl harmos_runtime::Lifecycle for Plain {
    type Config = ();
    type Error = std::convert::Infallible;
    type Context = harmos_runtime::Startup<Self>;

    async fn prepare(_: (), _: &mut harmos_runtime::Setup<Self>) -> std::result::Result<Self, Self::Error> {
        Ok(Self)
    }

    async fn initialize(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn stop(&mut self, _: &Self::Context) -> std::result::Result<(), Self::Error> {
        Ok(())
    }

    async fn cleanup(&mut self) -> std::result::Result<(), Self::Error> {
        Ok(())
    }
}

impl Application for Plain {
    type Lanes = PlainLanes;
    type State = State;
    type Transaction = Never;
    type Record = ();
    type Streams = ();
}

#[tokio::test]
async fn cloned_read_handles() {
    let runtime = runtime().await;
    let manager = runtime.state.as_principal(alice());
    let state: ManagedState<Bank> = manager.state();
    let cloned = state.clone();
    let receipt = manager.clone().commit(deposit(40)).await.unwrap();
    let value = cloned.read(|state| state.balance(HOUSE)).await.unwrap();
    assert_eq!(value.value, 40);
    assert_eq!(value.position, receipt.frontier);

    let undone = manager.undo(alice(), Focus::all()).await.unwrap();
    let value = state.read(|state| state.balance(HOUSE)).await.unwrap();
    assert_eq!(value.value, 0);
    assert_eq!(value.position, undone.frontier);

    let redone = manager.redo(alice(), Focus::all()).await.unwrap();
    let value = cloned.read(|state| state.balance(HOUSE)).await.unwrap();
    assert_eq!(value.value, 40);
    assert_eq!(value.position, redone.frontier);
    runtime.shutdown().await.unwrap();
}

#[tokio::test]
async fn state_survives_stop() {
    let runtime = runtime().await;
    let manager = runtime.state.clone();
    let state = manager.state();
    manager.commit(deposit(7)).await.unwrap();
    let through = runtime.shutdown().await.unwrap();
    assert_eq!(through, Position(1));
    assert!(manager.commit(deposit(1)).await.is_err());
    drop((manager, runtime));
    let value = state.read(|state| state.balance(HOUSE)).await.unwrap();
    assert_eq!(value.value, 7);
    assert_eq!(value.position, through);
}

#[tokio::test]
async fn handles_share_nonclone() {
    let runtime = Runtime::<Plain>::launch(State { value: 19 }, ()).await.unwrap();
    let manager = runtime.state.clone();
    let state = manager.state().clone();
    let value = state.read(|state| state.value).await.unwrap();
    assert_eq!(value.value, 19);
    assert_eq!(value.position, Position::ORIGIN);
    runtime.shutdown().await.unwrap();
}
