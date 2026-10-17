/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Modules === */
use crate::scenario_runtime;

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::fs;
use std::path::Path;

// Third Party Imports
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Meta, Token};

// Local Imports

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */
#[derive(Default)]
struct Tests {
    count: usize,
}

/* === Enums === */

/* === Traits === */

/* === Implementations === */
impl<'ast> Visit<'ast> for Tests {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if item
            .attrs
            .iter()
            .any(|attribute| test_attribute(&attribute.meta))
        {
            self.count += 1;
        }
        visit::visit_item_fn(self, item);
    }
}

/* === Public Functions === */

/* === Private Functions === */
fn test_attribute(meta: &Meta) -> bool {
    if meta
        .path()
        .segments
        .last()
        .is_some_and(|part| part.ident == "test")
    {
        return true;
    }
    let Meta::List(list) = meta else { return false };
    if !list.path.is_ident("cfg_attr") {
        return false;
    }
    let arguments = list
        .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        .expect("valid cfg_attr metadata");
    arguments.iter().skip(1).any(test_attribute)
}

fn count(source: &str) -> usize {
    let file = syn::parse_file(source).expect("valid Rust source");
    let mut tests = Tests::default();
    tests.visit_file(&file);
    tests.count
}

fn inspect(directory: &Path, findings: &mut Vec<String>) {
    for entry in fs::read_dir(directory).expect("read maintained source directory") {
        let entry = entry.expect("read source entry");
        let path = entry.path();
        let kind = entry.file_type().expect("source entry type");
        if kind.is_dir() {
            if !matches!(
                entry.file_name().to_str(),
                Some("target" | ".stkr" | ".git" | ".jj")
            ) {
                inspect(&path, findings);
            }
        } else if kind.is_file() && path.extension().is_some_and(|extension| extension == "rs") {
            let source = fs::read_to_string(&path).expect("read Rust source");
            let total = count(&source);
            if total > 32 {
                findings.push(format!(
                    "{}: {total} authored tests exceed 32",
                    path.display()
                ));
            }
        }
    }
}

/* === Tests === */

#[test]
fn contract_scenario() {
    {
        scenario_runtime::check("authored_test_limit", || {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap();
            let mut findings = Vec::new();
            for directory in ["crates", "examples", "tests"] {
                inspect(&root.join(directory), &mut findings);
            }
            findings.sort();
            assert!(findings.is_empty(), "{}", findings.join("\n"));
        });
    }
    {
        scenario_runtime::check("nested_conditional_counts", || {
            let source = r##"
        // #[test] fn comment() {}
        const TEXT: &str = r#"#[test] fn text() {}"#;
        #[cfg(test)] mod tests {
            #[test] fn first() {}
            mod nested {
                #[tokio::test(start_paused = true)] async fn second() {}
                #[cfg_attr(feature = "optional", test)] fn third() {}
                #[cfg_attr(feature = "optional", cfg_attr(unix, test))] fn fourth() {}
                #[test] #[cfg_attr(unix, test)] fn counted_once() {}
                fn helper() {}
            }
        }
    "##;
            assert_eq!(count(source), 5);
            let functions = (0..33)
                .map(|index| format!("#[test] fn test_{index}() {{}}\n"))
                .collect::<String>();
            assert_eq!(count(&functions), 33);
        });
    }
}
