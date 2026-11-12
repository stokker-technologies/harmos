/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::*;

#[tokio::test]
async fn oversized_entry_warns_and_preserves_recovery() {
    // Register The Unobserved Dispatcher Too: Parallel Tests Register Callsites Without
    // A Subscriber, Which Tracing's Single-Dispatcher Cache Otherwise Treats As Disabled.
    let _unobserved = tracing::Dispatch::new(tracing::subscriber::NoSubscriber::default());
    let warnings = Warnings::default();
    let subscriber = tracing_subscriber::registry().with(warnings.clone());
    async {
        let store = Store::new();
        let storage = store.manager(config());
        let mut policy = policy(10000);
        policy.max_bytes = NonZeroUsize::new(1024).unwrap();
        let runtime = launch(policy.clone(), &storage).await;
        runtime.state.commit(deposit(4)).await.unwrap();
        crate::support::app::shutdown(&runtime).await.unwrap();
        let storage = store.manager(config());
        let runtime = launch(policy.clone(), &storage).await;
        let journal = runtime.state.as_principal(PrincipalId::new("editor"));
        harmos_runtime::records::RecordManager::new(journal.clone()).publish(Noted("x".repeat(2048))).await.unwrap();
        crate::support::app::shutdown(&runtime).await.unwrap();
        assert_eq!(store.snapshot().through.sequence, 1);
        {
            let captured = warnings.0.lock().unwrap();
            assert!(!captured.is_empty());
            for warning in captured.iter() {
                assert!(warning.size_bytes > 2048, "{warning:?}");
                assert_eq!(warning.segment_limit, policy.max_bytes.get() as u64);
                assert_eq!(warning.replay_entries, 1);
            }
        }
        let storage = store.manager(config());
        let runtime = launch(policy, &storage).await;
        assert_eq!(balance(&runtime).await, 4);
        assert_eq!(runtime.state.applied(), Position(2));
        let facts: Vec<_> = runtime.state.query::<Fact>(Position(1)).await.collect();
        assert_eq!(facts.len(), 1);
        assert!(facts[0].is_ok());
        crate::support::app::shutdown(&runtime).await.unwrap();
    }
    .with_subscriber(subscriber)
    .await;
}
