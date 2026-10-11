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
use dependency::catalog::{Catalog, Contains, Member, Visitor};

// Standard Library Imports

// Third Party Imports

// Local Imports


/* === Constants === */
const _: () = assert!(<Entries as Catalog<Names>>::ENTRIES[0] == 7);
const _: () = assert!(<Empty as Catalog<Names>>::ENTRIES.is_empty());
const _: fn() = membership::<Entries>;

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
struct Row;
struct Names;

/* === Enums === */
#[dependency::catalog]
enum Entries { Row(Row) }
#[dependency::catalog]
enum Empty {}

/* === Traits === */

/* === Implementations === */
impl Visitor for Names { type Entry = usize; }
impl Member<Row> for Names { const ENTRY: usize = 7; }

/* === Public Functions === */

/* === Private Functions === */
fn membership<T: Contains<Row>>() {}

/* === Tests === */
