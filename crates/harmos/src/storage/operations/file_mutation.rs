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

// Local Imports

/* === Constants === */

/* === Macros === */
crate::storage::codec::product!(FileMutation {
    mutation: crate::storage::codec::Nested<crate::storage::MutationId> => 1,
    session: crate::storage::codec::Nested<crate::storage::FileSession> => 2,
});

/* === Type Aliases === */

/* === Structs === */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileMutation {
    pub mutation: crate::storage::MutationId,
    pub session: crate::storage::FileSession,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl crate::MemorySize for FileMutation {
    fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.mutation.memory_size()
            - std::mem::size_of::<crate::storage::MutationId>()
            + self.session.memory_size()
            - std::mem::size_of::<crate::storage::FileSession>()
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
