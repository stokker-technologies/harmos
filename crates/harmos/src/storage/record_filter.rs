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
// Standard Library Imports

// Third Party Imports
use serde::{Deserialize, Serialize};
// Local Imports
use super::codec::{Repeated, product};
use super::frame::plan::{MAX_WIDTH, heap, invalid};
use super::frame::{Expr, Predicate, ScalarField, satisfies};
use super::reduce::ScalarValue;
use crate::{MemorySize, Result, schema};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
// Bounded scalar predicates for saved record queries. Columns identify payload
// or envelope fields; clauses combine with OR, predicates within each with AND.
// Backends validate
// the selected schema before applying predicates under the original read cut.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordFilter {
    pub columns: Vec<ScalarField>,
    pub clauses: Vec<RecordClause>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordClause {
    pub predicates: Vec<Predicate>,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl RecordFilter {
    pub fn check(&self) -> Result<()> {
        if self.columns.is_empty()
            || self.columns.len() > MAX_WIDTH
            || self.clauses.is_empty()
            || self.clauses.len() > MAX_WIDTH
            || self
                .clauses
                .iter()
                .any(|clause| clause.predicates.is_empty())
            || self.clauses.iter().fold(0usize, |count, clause| {
                count.saturating_add(clause.predicates.len())
            }) > MAX_WIDTH
        {
            return Err(invalid(
                "record filter requires 1..64 columns and 1..64 total predicates in nonempty clauses",
            ));
        }
        for column in &self.columns {
            column.check()?;
        }
        for predicate in self.clauses.iter().flat_map(|clause| &clause.predicates) {
            let Expr::Col(index) = *predicate.expr() else {
                return Err(invalid("record filters require original columns"));
            };
            let column = self
                .columns
                .get(index)
                .ok_or(invalid("record filter column out of range"))?;
            predicate.check_kind(column.kind)?;
        }
        Ok(())
    }

    pub fn check_schema(&self, view: schema::graph::View<'_>) -> Result<()> {
        self.check()?;
        for column in &self.columns {
            column.check_schema(view)?;
        }
        Ok(())
    }

    // Match an already validated scalar projection, without decoding another
    // payload or allocating a second index. Presence and equality to a non-null
    // owner reject absent owners.
    pub fn matches(&self, values: &[ScalarValue]) -> Result<bool> {
        self.check()?;
        if values.len() != self.columns.len() {
            return Err(invalid("record projection width differs from filter"));
        }
        for (value, column) in values.iter().zip(&self.columns) {
            value.check()?;
            if value.kind().is_some_and(|kind| kind != column.kind) {
                return Err(invalid("record projection differs from declared type"));
            }
        }
        for clause in &self.clauses {
            let mut matched = true;
            for predicate in &clause.predicates {
                let Expr::Col(index) = *predicate.expr() else {
                    unreachable!("checked original column")
                };
                if !satisfies(&values[index], predicate)? {
                    matched = false;
                    break;
                }
            }
            if matched {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
impl MemorySize for RecordFilter {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(heap(&self.columns))
            .saturating_add(heap(&self.clauses))
    }
}
product!(RecordFilter {
    columns: Repeated<ScalarField> => 1,
    clauses: Repeated<RecordClause> => 2,
});

impl MemorySize for RecordClause {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>().saturating_add(heap(&self.predicates))
    }
}
product!(RecordClause {
    predicates: Repeated<Predicate> => 1,
});

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
