/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Report One Exit Policy For Packaging And Binding Commands.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */
// Standard Library Imports
// Third Party Imports
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

/* === Enums === */
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Packaging(#[from] harmos::packaging::Error),
    #[error(transparent)]
    Bindings(#[from] harmos::bindings::python::Error),
}

/* === Traits === */

/* === Implementations === */
impl Error {
    pub fn exit_code(&self) -> u8 {
        use harmos::bindings::python::Error as Bindings;
        use harmos::packaging::Error as Packaging;
        match self {
            Self::Packaging(
                Packaging::Declaration(harmos::packaging::DeclarationError::OversizedFragment)
                | Packaging::Bundle(harmos::bundle::Error::Limit(_)),
            )
            | Self::Bindings(Bindings::Limit) => 4,
            Self::Packaging(
                Packaging::Settings(_)
                | Packaging::Component(_)
                | Packaging::Collision(_)
                | Packaging::Declaration(_),
            )
            | Self::Bindings(
                Bindings::Settings(_)
                | Bindings::Unsupported(_)
                | Bindings::Collision(_)
                | Bindings::Selection(_)
                | Bindings::Exists(_),
            ) => 2,
            _ => 3,
        }
    }
}

/* === Public Functions === */

/* === Private Functions === */

/* === Tests === */
