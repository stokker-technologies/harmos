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
use wasm_encoder::reencode::{Reencode, ReencodeComponent};
use wit_component::ComponentEncoder;
// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

struct Shipping;

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Reencode for Shipping {
    type Error = std::convert::Infallible;

    fn parse_custom_section(
        &mut self, module: &mut wasm_encoder::Module, section: wasmparser::CustomSectionReader<'_>,
    ) -> Result<(), wasm_encoder::reencode::Error<Self::Error>> {
        if !section.name().starts_with(".debug_") {
            wasm_encoder::reencode::utils::parse_custom_section(self, module, section)?;
        }
        Ok(())
    }
}

impl ReencodeComponent for Shipping {
    fn parse_component_custom_section(
        &mut self, component: &mut wasm_encoder::Component, section: wasmparser::CustomSectionReader<'_>,
    ) -> Result<(), wasm_encoder::reencode::Error<Self::Error>> {
        if !section.name().starts_with(".debug_") {
            wasm_encoder::reencode::component_utils::parse_component_custom_section(self, component, section)?;
        }
        Ok(())
    }
}

/* === Public Functions === */

pub fn componentize(module: &[u8]) -> Result<Vec<u8>, String> {
    ComponentEncoder::default()
        .module(module)
        .map_err(|error| format!("{error:#}"))?
        .validate(true)
        .encode()
        .map_err(|error| format!("{error:#}"))
}

/* === Private Functions === */

pub(super) fn strip_debug(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let parser = wasmparser::Parser::new(0);
    if wasmparser::Parser::is_component(bytes) {
        let mut component = wasm_encoder::Component::new();
        Shipping.parse_component(&mut component, parser, bytes).map_err(|error| error.to_string())?;
        return Ok(component.finish());
    }
    let mut module = wasm_encoder::Module::new();
    Shipping.parse_core_module(&mut module, parser, bytes).map_err(|error| error.to_string())?;
    Ok(module.finish())
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::strip_debug;
    use wasm_encoder::{Component, CustomSection, Module, ModuleSection, NestedComponentSection};

    #[test]
    fn strips_debug_recursively_and_preserves_metadata() {
        let metadata = CustomSection {
            name: "harmos".into(),
            data: (&b"identity"[..]).into(),
        };
        let debug = CustomSection {
            name: ".debug_info".into(),
            data: (&b"debug"[..]).into(),
        };
        let mut module = Module::new();
        module.section(&metadata).section(&debug);
        let mut expected_module = Module::new();
        expected_module.section(&metadata);
        assert_eq!(strip_debug(module.as_slice()).unwrap(), expected_module.as_slice());

        let mut nested = Component::new();
        nested.section(&ModuleSection(&module)).section(&metadata).section(&debug);
        let mut component = Component::new();
        component.section(&NestedComponentSection(&nested)).section(&debug);
        let mut expected_nested = Component::new();
        expected_nested.section(&ModuleSection(&expected_module)).section(&metadata);
        let mut expected = Component::new();
        expected.section(&NestedComponentSection(&expected_nested));
        assert_eq!(strip_debug(component.as_slice()).unwrap(), expected.as_slice());
    }
}
