/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Ordered Slash-Relative Directory Selection

use harmos_domain::model::storage::DirectoryRule;

pub(super) fn selected(path: &str, rules: &[DirectoryRule]) -> bool {
    rules.iter().fold(false, |included, rule| if matches(&rule.pattern, path) { rule.include } else { included })
}

fn matches(pattern: &str, path: &str) -> bool {
    let pattern = pattern.as_bytes();
    let path = path.as_bytes();
    let mut current = vec![false; path.len() + 1];
    let mut next = vec![false; path.len() + 1];
    current[0] = true;
    let mut offset = 0;
    while offset < pattern.len() {
        next.fill(false);
        if pattern[offset] == b'*' {
            let recursive = pattern.get(offset + 1) == Some(&b'*');
            offset += if recursive { 2 } else { 1 };
            let prefix = recursive && pattern.get(offset) == Some(&b'/');
            if prefix {
                offset += 1;
            }
            let mut earlier = false;
            for index in 0..=path.len() {
                earlier |= current[index];
                next[index] = current[index]
                    || if prefix {
                        index > 0 && path[index - 1] == b'/' && earlier
                    } else {
                        index > 0 && next[index - 1] && (recursive || path[index - 1] != b'/')
                    };
            }
        } else {
            for (index, byte) in path.iter().enumerate() {
                next[index + 1] = current[index] && *byte == pattern[offset];
            }
            offset += 1;
        }
        std::mem::swap(&mut current, &mut next);
    }
    current[path.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(!selected("private/secret.bin", &rules));
        assert!(selected("private/released/data.bin", &rules));
        assert!(!selected("anything", &[]));
        assert!(selected("public/data.bin", &rules));
    }
}
