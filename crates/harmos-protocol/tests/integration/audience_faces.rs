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

// Third Party Imports

// Local Imports

use prost_reflect::ReflectMessage;

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
fn executors_and_callers_project_host_operations_for_each_audience() {
    use harmos_protocol::host::TimeGetRequest;
    use harmos_protocol::model::Callers;
    use harmos_protocol::schema::{Executor, method_callers, operation_identity};
    let descriptor = TimeGetRequest::default().descriptor();
    let pool = descriptor.parent_pool();

    // Project Every Host Method From Its Mandatory Option.
    let mut clients = std::collections::BTreeSet::new();
    let mut plugins = std::collections::BTreeSet::new();
    for service in pool.services() {
        let id = format!("{}/{}", service.full_name(), service.methods().next().unwrap().name());
        let executor = operation_identity(&id).unwrap();
        for method in service.methods() {
            let callers = method_callers(&method).unwrap();
            let id = format!("{}/{}", service.full_name(), method.name());
            if executor == Executor::Host && matches!(callers, Callers::Clients | Callers::All) {
                clients.insert(id.clone());
            }
            if executor == Executor::Host && matches!(callers, Callers::Plugins | Callers::All) {
                plugins.insert(id);
            }
        }
    }

    // Both Callers Resolve The Same Declaration And Payload Graph.
    for id in ["harmos.host.v1.Time/Get", "harmos.host.v1.Operations/List", "harmos.host.v1.plugins.Settings/List"] {
        assert!(clients.contains(id));
        assert!(plugins.contains(id));
    }
    assert!(clients.contains("harmos.host.v1.Invocations/Create"));
    assert!(!plugins.contains("harmos.host.v1.Invocations/Create"));
    assert!(!plugins.contains("harmos.host.v1.plugins.Settings/Set"));
    assert!(pool.files().filter(|file| file.package_name() == "harmos.model.v1").all(|file| file.services().len() == 0));

    // Runtime Projections Restore Caller Options After Data-Only Descriptor Assembly.
    use prost::Message;
    let raw = pool.encode_to_vec();
    let types = prost_types::FileDescriptorSet::decode(raw.as_slice()).unwrap();
    let callers = pool
        .services()
        .flat_map(|service| service.methods().collect::<Vec<_>>())
        .map(|method| (format!("{}/{}", method.parent_service().full_name(), method.name()), method_callers(&method).unwrap()))
        .collect();
    let retained = harmos_protocol::schema::with_method_callers(types, &callers).unwrap();
    let service = retained.get_service_by_name("harmos.host.v1.Time").unwrap();
    assert_eq!(method_callers(&service.methods().next().unwrap()).unwrap(), Callers::All);
    let service = retained.get_service_by_name("harmos.host.v1.Invocations").unwrap();
    let create = service.methods().find(|method| method.name() == "Create").unwrap();
    assert_eq!(method_callers(&create).unwrap(), Callers::Clients);
}
