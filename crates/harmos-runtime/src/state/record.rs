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
pub use harmos_domain::state::{
    CatalogCodec, PreparationError, Prepared, Record, RecordCatalog, RecordProjection, Records,
};

/* === Imports === */
use super::envelope::Recorded;
use super::transaction::DecodeError;
use crate::Application;

/* === Type Aliases === */
pub(crate) type Upgrade<A, W> = fn(W) -> Result<<A as Application>::Record, DecodeError>;

/* === Structs === */
pub(crate) struct PreparedCommit<A: Application> {
    pub(crate) transaction: Prepared,
    pub(crate) records: Vec<(Recorded<A>, Prepared)>,
}

/* === Public Functions === */
pub(crate) fn prepare<A: Application>(
    transaction: &A::Transaction,
    state: &A::State,
    catalog: &super::dispatch::Dispatch<A, crate::storage::Payload<A>>,
) -> Result<PreparedCommit<A>, PreparationError> {
    let (prepared, members) = Records::collect(transaction, state)?;
    let records = members
        .into_iter()
        .map(|(system, member)| {
            if let Some(fact) = system {
                return Ok((Recorded::System(fact), member));
            }
            if !matches!(catalog.has_record(&member.id, member.version), Ok(true)) {
                return Err(PreparationError::Membership);
            }
            let value = A::Record::restore(&member.id, member.version, &member.payload)
                .map_err(|_| PreparationError::Membership)?
                .ok_or(PreparationError::Membership)?;
            if value.identify() != (member.id.clone(), member.version) {
                return Err(PreparationError::Membership);
            }
            Ok((Recorded::Record(value), member))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PreparedCommit {
        transaction: prepared,
        records,
    })
}
