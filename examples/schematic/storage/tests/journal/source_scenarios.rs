/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

use super::Model;
use harmos::storage::Source;
use schematic_storage::Jsonl;

#[test]
fn contract_scenario() {
    {
        {
            let mut builder = tokio::runtime::Builder::new_current_thread();
            builder.enable_all().build().expect("scenario runtime is valid").block_on(async {
                let directory = tempfile::tempdir().unwrap();
                let path = directory.path().join("journal.jsonl");
                std::fs::write(&path, "{\"schema\":1,\"generation\":0,\"origin\":0}\n").unwrap();
                let mut log = Jsonl::<Model>::open(&path).unwrap();
                std::fs::write(&path, vec![b' '; 8192]).unwrap();
                assert!(Jsonl::<Model>::open(&path).is_err());
                assert!(log.load().await.is_err());
            });
        }
    }
}
