/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Face-Local Wrappers and Deliberate Stream Cardinality.

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
fn service_packages_separate_audiences_and_only_declared_methods_stream() {
    let descriptor = harmos_protocol::context::TimeGetRequest::default().descriptor();
    let pool = descriptor.parent_pool();
    let mut instances = Vec::new();
    for service in pool.services() {
        let face = if service.full_name().starts_with("harmos.plugin.v1.") {
            "plugin"
        } else if service.full_name().starts_with("harmos.context.v1.") {
            "context"
        } else {
            assert!(service.full_name().starts_with("harmos.v1."), "{}", service.full_name());
            "client"
        };
        assert_ne!(service.name(), "Plugin");
        assert_eq!(service.parent_file().services().len(), 1);
        if face == "plugin" {
            instances.push(service.full_name().to_owned());
        }
        for method in service.methods() {
            assert_eq!(method.input().name(), format!("{}{}Request", service.name(), method.name()));
            assert_eq!(method.output().name(), format!("{}{}Response", service.name(), method.name()));
            assert_eq!(method.input().parent_file().package_name(), service.parent_file().package_name());
            assert_eq!(method.output().parent_file().package_name(), service.parent_file().package_name());
            let carrier = face == "plugin" && matches!(method.name(), "Invoke" | "Samples");
            assert_eq!(method.is_client_streaming(), carrier);
            assert_eq!(method.is_server_streaming(), carrier || method.name().starts_with("Watch"));
            assert!(!method.name().starts_with("Run"));
        }
    }
    assert_eq!(instances, ["harmos.plugin.v1.Instance"]);
    for name in ["CatalogEntries", "Invocations", "Time", "Storage"] {
        let client = pool.get_service_by_name(&format!("harmos.v1.{name}")).unwrap();
        let context = pool.get_service_by_name(&format!("harmos.context.v1.{name}")).unwrap();
        for child in context.methods() {
            let parent = client.methods().find(|method| method.name() == child.name()).unwrap();
            assert_ne!(parent.input().full_name(), child.input().full_name());
            assert_ne!(parent.output().full_name(), child.output().full_name());
        }
    }
}
