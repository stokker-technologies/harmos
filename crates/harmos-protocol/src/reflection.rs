/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Reflect Generated Host Operation Messages.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

use prost_reflect::{DescriptorPool, MessageDescriptor, ReflectMessage};
use std::sync::OnceLock;

// Standard Library Imports

// Third Party Imports

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ReflectMessage for crate::host::Catalog {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("harmos.host.v1.Catalog").expect("generated host message")
    }
}
impl ReflectMessage for crate::host::CatalogPage {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("harmos.host.v1.CatalogPage").expect("generated host message")
    }
}

impl ReflectMessage for crate::v1::Sleep {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("harmos.v1.Sleep").expect("generated clock message")
    }
}
impl ReflectMessage for crate::v1::SleepResponse {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("harmos.v1.SleepResponse").expect("generated clock message")
    }
}
impl ReflectMessage for crate::v1::Now {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("harmos.v1.Now").expect("generated clock message")
    }
}
impl ReflectMessage for crate::v1::Time {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("harmos.v1.Time").expect("generated clock message")
    }
}

impl ReflectMessage for crate::host::InstanceState {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("harmos.host.v1.InstanceState").expect("generated message")
    }
}

impl ReflectMessage for crate::host::Watch {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("harmos.host.v1.Watch").expect("generated message")
    }
}

impl ReflectMessage for crate::host::Cancel {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("harmos.host.v1.Cancel").expect("generated message")
    }
}

impl ReflectMessage for crate::host::Status {
    fn descriptor(&self) -> MessageDescriptor {
        pool().get_message_by_name("harmos.host.v1.Status").expect("generated message")
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn pool() -> &'static DescriptorPool {
    static POOL: OnceLock<DescriptorPool> = OnceLock::new();
    POOL.get_or_init(|| DescriptorPool::decode(include_bytes!("descriptors.bin").as_slice()).expect("generated protocol descriptors"))
}

/* === Tests === */
