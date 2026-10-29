/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Ordered Slash-Relative Directory Selection

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
// Third Party Imports
use globset::{GlobBuilder, GlobMatcher};
// Local Imports
use crate::{Error, Result};
use harmos_domain::model::storage::DirectoryRule;

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

pub(super) struct Rules(Vec<(GlobMatcher, bool)>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl Rules {
    pub(super) fn compile(rules: &[DirectoryRule]) -> Result<Self> {
        rules
            .iter()
            .map(|rule| {
                let glob = GlobBuilder::new(&recursive(&rule.pattern)).literal_separator(true).backslash_escape(false).build();
                let glob = glob.map_err(|_| Error::InvalidInput {
                    field: "directory rule",
                    reason: "pattern is not a valid glob",
                })?;
                Ok((glob.compile_matcher(), rule.include))
            })
            .collect::<Result<_>>()
            .map(Self)
    }

    pub(super) fn selected(&self, path: &str) -> bool {
        self.0.iter().rev().find(|(glob, _)| glob.is_match(path)).is_some_and(|(_, include)| *include)
    }
}

/* === Public Functions === */

/* === Private Functions === */

/// Spells a `**` inside a segment (`**.json`, `logs**`) as an alternation that still crosses `/`;
/// globset only treats a whole `**` segment as recursive.
fn recursive(pattern: &str) -> String {
    pattern
        .split('/')
        .map(|segment| if segment == "**" { segment.to_owned() } else { segment.replace("**", "{*,*/**/*}") })
        .collect::<Vec<_>>()
        .join("/")
}

/* === Tests === */

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(pattern: &str, path: &str) -> bool {
        Rules::compile(&[DirectoryRule { pattern: pattern.into(), include: true }]).unwrap().selected(path)
    }

    #[test]
    fn recursive_patterns() {
        assert!(matches("*.json", "a.json"));
        assert!(!matches("*.json", "nested/a.json"));
        assert!(matches("**/*.json", "a.json"));
        assert!(matches("**/*.json", "nested/deeper/a.json"));
        assert!(matches("data/**/sample.*", "data/sample.bin"));
        assert!(matches("data/**/sample.*", "data/a/b/sample.bin"));
        assert!(!matches("data/*.json", "Data/a.json"));
        assert!(!matches("**/a.json", "a.json/extra"));
        assert!(matches("**.json", "a.json"));
        assert!(matches("**.json", "nested/deeper/a.json"));
        assert!(matches("logs**", "logs"));
        assert!(matches("logs**", "logs/a/b.txt"));
        assert!(!matches("logs**", "other/logs"));
    }

    #[test]
    fn ancestor_rules_order() {
        let rules = vec![
            DirectoryRule { pattern: "**".into(), include: true },
            DirectoryRule {
                pattern: "private/**".into(),
                include: false,
            },
            DirectoryRule {
                pattern: "private/released/**".into(),
                include: true,
            },
        ];
        let rules = Rules::compile(&rules).unwrap();
        assert!(!rules.selected("private/secret.bin"));
        assert!(rules.selected("private/released/data.bin"));
        assert!(!Rules::compile(&[]).unwrap().selected("anything"));
        assert!(rules.selected("public/data.bin"));
        assert!(Rules::compile(&[DirectoryRule { pattern: "[".into(), include: true }]).is_err());
    }
}
