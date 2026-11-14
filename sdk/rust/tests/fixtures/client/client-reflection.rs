// @generated from client composition descriptors.
use prost_reflect::{DescriptorPool, MessageDescriptor, ReflectMessage};
fn pool() -> &'static DescriptorPool {
static POOL: std::sync::OnceLock<DescriptorPool> = std::sync::OnceLock::new();
POOL.get_or_init(|| DescriptorPool::decode(include_bytes!("client-descriptors.bin").as_slice()).expect("generated client descriptors"))
}
impl ReflectMessage for crate::host::JobsStartRequest { fn descriptor(&self) -> MessageDescriptor { pool().get_message_by_name("application.host.v1.JobsStartRequest").expect("generated fixture message") } }
impl ReflectMessage for crate::host::JobsStartResponse { fn descriptor(&self) -> MessageDescriptor { pool().get_message_by_name("application.host.v1.JobsStartResponse").expect("generated fixture message") } }
impl ReflectMessage for crate::host::jobs::ProgressWatchRequest { fn descriptor(&self) -> MessageDescriptor { pool().get_message_by_name("application.host.v1.jobs.ProgressWatchRequest").expect("generated fixture message") } }
impl ReflectMessage for crate::host::jobs::ProgressWatchResponse { fn descriptor(&self) -> MessageDescriptor { pool().get_message_by_name("application.host.v1.jobs.ProgressWatchResponse").expect("generated fixture message") } }
impl ReflectMessage for crate::host::plugins::AnnotationsGetRequest { fn descriptor(&self) -> MessageDescriptor { pool().get_message_by_name("application.host.v1.plugins.AnnotationsGetRequest").expect("generated fixture message") } }
impl ReflectMessage for crate::host::plugins::AnnotationsGetResponse { fn descriptor(&self) -> MessageDescriptor { pool().get_message_by_name("application.host.v1.plugins.AnnotationsGetResponse").expect("generated fixture message") } }
