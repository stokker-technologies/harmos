// @generated from client composition descriptors.
use prost_reflect::{DescriptorPool, MessageDescriptor, ReflectMessage};
fn pool() -> &'static DescriptorPool {
static POOL: std::sync::OnceLock<DescriptorPool> = std::sync::OnceLock::new();
POOL.get_or_init(|| DescriptorPool::decode(include_bytes!("client-descriptors.bin").as_slice()).expect("generated client descriptors"))
}
impl ReflectMessage for crate::v1::JobsStartRequest { fn descriptor(&self) -> MessageDescriptor { pool().get_message_by_name("application.v1.JobsStartRequest").expect("generated fixture message") } }
impl ReflectMessage for crate::v1::JobsStartResponse { fn descriptor(&self) -> MessageDescriptor { pool().get_message_by_name("application.v1.JobsStartResponse").expect("generated fixture message") } }
impl ReflectMessage for crate::v1::jobs::ProgressWatchRequest { fn descriptor(&self) -> MessageDescriptor { pool().get_message_by_name("application.v1.jobs.ProgressWatchRequest").expect("generated fixture message") } }
impl ReflectMessage for crate::v1::jobs::ProgressWatchResponse { fn descriptor(&self) -> MessageDescriptor { pool().get_message_by_name("application.v1.jobs.ProgressWatchResponse").expect("generated fixture message") } }
