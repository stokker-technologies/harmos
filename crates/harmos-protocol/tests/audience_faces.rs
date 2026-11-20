/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Executor Declarations And Caller Projections.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::collections::BTreeSet;

// Third Party Imports
use prost::Message;
use prost_reflect::ReflectMessage;
use prost_types::FileDescriptorSet;

// Local Imports
use harmos_protocol::host::TimeGetRequest;
use harmos_protocol::model::Callers;
use harmos_protocol::schema::{self, Executor, method_callers, operation_identity};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */

#[test]
fn host_services_are_absent_and_plugin_facilities_declare_only_plugins() {
    let descriptor = TimeGetRequest::default().descriptor();
    let pool = descriptor.parent_pool();
    assert!(pool.files().filter(|file| file.package_name().starts_with("harmos.host.")).all(|file| file.services().len() == 0));
    let expected = BTreeSet::from([
        "harmos.plugin.v1.Time/Get".to_owned(),
        "harmos.plugin.v1.Time/Sleep".to_owned(),
        "harmos.plugin.v1.Invocations/Get".to_owned(),
        "harmos.plugin.v1.Invocations/Cancel".to_owned(),
        "harmos.plugin.v1.Invocations/Watch".to_owned(),
        "harmos.plugin.v1.Operations/List".to_owned(),
        "harmos.plugin.v1.Operations/Get".to_owned(),
        "harmos.plugin.v1.plugins.Settings/List".to_owned(),
        "harmos.plugin.v1.plugins.State/Get".to_owned(),
        "harmos.plugin.v1.plugins.State/Put".to_owned(),
    ]);
    let mut actual = BTreeSet::new();
    for service in pool.services() {
        for method in service.methods() {
            let id = format!("{}/{}", service.full_name(), method.name());
            if expected.contains(&id) {
                assert_eq!(operation_identity(&id).unwrap(), Executor::Plugin);
                assert_eq!(method_callers(&method).unwrap(), Callers::Plugins);
                actual.insert(id);
            }
        }
    }
    assert_eq!(actual, expected);

    // Product Projections Retain Their Explicit Audience Metadata.
    let types = FileDescriptorSet::decode(pool.encode_to_vec().as_slice()).unwrap();
    let callers = actual.into_iter().map(|id| (id, Callers::Plugins)).collect();
    let retained = schema::with_method_callers(types, &callers).unwrap();
    let service = retained.get_service_by_name("harmos.plugin.v1.Time").unwrap();
    assert_eq!(method_callers(&service.methods().next().unwrap()).unwrap(), Callers::Plugins);
}
