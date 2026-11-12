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

use std::io::Write;
use harmos_protocol::status::{DiagnosticsGetRequest,DiagnosticsGetResponse};
use harmos_operations::execution::operations::ServingError;
use sha2::{Digest,Sha256};
use super::{Status,failure};


// Standard Library Imports

// Third Party Imports

// Local Imports


/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub struct Entry { pub path: String, pub text: String }
pub struct Range { pub data: Vec<u8>, pub total_length: u64, pub sha256: Vec<u8> }
pub(super) struct Archive { pub name: String, bytes: Vec<u8>, digest: Vec<u8> }
struct Bounded { bytes: Vec<u8>, maximum: usize }


/* === Enums === */

/* === Traits === */

/// Immutable Content Boundary Shared by Unary Ranged Reads.
pub trait ContentSource {
 fn range(&self, name: &str, offset: u64, maximum_bytes: usize) -> Result<Range, ServingError>;
}


/* === Implementations === */

impl Write for Bounded {
 fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
  if self.bytes.len().saturating_add(bytes.len()) > self.maximum { return Err(std::io::Error::other("archive byte budget exceeded")); }
  self.bytes.extend_from_slice(bytes); Ok(bytes.len())
 }
 fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
}
impl ContentSource for Status {
 fn range(&self, name: &str, offset: u64, maximum_bytes: usize) -> Result<Range, ServingError> {
  let action = "Status.Diagnostics.Get";
  if maximum_bytes == 0 || maximum_bytes > self.settings.range_bytes.get() { return Err(failure(action,"range exceeds configured byte bound")); }
  let state = self.state.lock().expect("status lock is healthy");
  let archive = state.archive.as_ref().filter(|archive| archive.name == name).ok_or_else(|| failure(action,"archive is unknown or expired"))?;
  let offset = usize::try_from(offset).map_err(|_| failure(action,"range offset is unsupported"))?;
  if offset > archive.bytes.len() { return Err(failure(action,"range offset exceeds archive length")); }
  let end = offset.saturating_add(maximum_bytes).min(archive.bytes.len());
  Ok(Range { data: archive.bytes[offset..end].to_vec(), total_length: archive.bytes.len() as u64, sha256: archive.digest.clone() })
 }
}
impl Status {
 pub(super) fn diagnostics(&self, request: DiagnosticsGetRequest) -> Result<DiagnosticsGetResponse, ServingError> {
  let action = "Status.Diagnostics.Get";
  if request.max_bytes == 0 || request.max_bytes > self.settings.range_bytes.get() as u64 { return Err(failure(action,"range exceeds configured byte bound")); }
  let name = if request.name.is_empty() {
   if request.offset != 0 { return Err(failure(action,"first archive range must start at zero")); }
   self.archive()?
  } else { request.name };
  let range = self.range(&name,request.offset,request.max_bytes as usize)?;
  Ok(DiagnosticsGetResponse { name, data: range.data, total_length: range.total_length, sha256: range.sha256 })
 }
 fn archive(&self) -> Result<String, ServingError> {
  let action = "Status.Diagnostics.Get";
  let (status,providers,logs,traces) = {
   let mut state = self.state.lock().expect("status lock is healthy");
   let page = harmos_protocol::v1::PageRequest { page_size: u32::MAX, max_bytes: self.settings.history_bytes.get().saturating_add(self.settings.history_items.get().saturating_mul(16)).saturating_add(128) as u64, page_token: String::new() };
   let logs = state.logs.page(None,&page,action)?.0;
   let traces = state.traces.page(None,&page,action)?.0;
   (self.snapshot(&state), state.providers.values().cloned().collect::<Vec<_>>(), logs,traces)
  };
  let mut entries = vec![Entry { path:"host.txt".into(), text:format!("{status:#?}") }, Entry { path:"logs.txt".into(),text:format!("{logs:#?}") }, Entry { path:"trace-events.txt".into(),text:format!("{traces:#?}") }];
  for provider in providers { entries.extend(provider.diagnostics().map_err(|_| failure(action,"contribution unavailable"))?); }
  let mut paths = std::collections::BTreeSet::new();
  let mut builder = tar::Builder::new(Bounded { bytes: Vec::new(), maximum: self.settings.archive_bytes.get() });
  for mut entry in entries {
   entry.path = self.redaction.text(&entry.path);
   if entry.path.is_empty() || entry.path.starts_with('/') || entry.path.contains('\\') || entry.path.split('/').any(|part| part.is_empty() || part==".." || part==".") || entry.path.contains(':') || !paths.insert(entry.path.clone()) {
    return Err(failure(action,"archive entry path is unsafe or duplicated"));
   }
   let text = self.redaction.text(&entry.text);
   let mut header = tar::Header::new_gnu(); header.set_size(text.len() as u64); header.set_mode(0o600); header.set_mtime(0); header.set_cksum();
   builder.append_data(&mut header,&entry.path,text.as_bytes()).map_err(|_| failure(action,"archive entry exceeds configured budget or has an invalid path"))?;
  }
  let bytes = builder.into_inner().map_err(|_| failure(action,"archive exceeds configured byte budget"))?.bytes;
  let digest = Sha256::digest(&bytes).to_vec(); let name = digest.iter().map(|byte|format!("{byte:02x}")).collect::<String>();
  self.state.lock().expect("status lock is healthy").archive = Some(Archive { name:name.clone(),bytes,digest });
  Ok(name)
 }
}


/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
