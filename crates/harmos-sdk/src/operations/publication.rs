/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */

use super::access::AccessError;
use harmos_domain::model::operations::publication::FollowingRow;
use std::{future::Future, pin::Pin, sync::Arc};

/* === Type Aliases === */

pub type Published<'a> = Pin<Box<dyn Future<Output = Result<(), AccessError>> + Send + 'a>>;

/* === Structs === */

#[derive(Clone)]
pub struct PublicationAccess(pub Arc<dyn PublicationHost>);

pub struct Publication<'a> {
    pub stream: &'a str,
    pub schema: u32,
    pub sequence: Option<u64>,
    pub published_unix_micros: Option<i64>,
    pub context: &'a mut Option<String>,
    pub payload: &'a mut Vec<u8>,
    pub following: &'a mut Vec<FollowingRow>,
}

/* === Traits === */

pub trait PublicationHost: Send + Sync {
    fn publish<'a>(&'a self, publication: Publication<'a>) -> Published<'a>;
}
