/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Verify Opened File Publication Identity and Replacement Refusal.

/* === Imports === */

// Standard Library Imports
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};

// Third Party Imports

// Local Imports
use harmos_runtime::filesystem::publication::install_opened;

/* === Tests === */

#[test]
fn opened_install_preserves_identity_and_refuses_replacement() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source");
    let destination = root.path().join("destination");
    let mut file = OpenOptions::new().create_new(true).read(true).write(true).open(&source).unwrap();
    file.write_all(b"evidence").unwrap();
    install_opened(&file, &source, &destination).unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"evidence");
    assert_eq!(fs::read(&destination).unwrap(), bytes);
    let other = root.path().join("other");
    let mut replacement = File::create(&other).unwrap();
    replacement.write_all(b"different").unwrap();
    assert!(install_opened(&replacement, &other, &destination).is_err());
    assert_eq!(fs::read(&destination).unwrap(), b"evidence");
    assert_eq!(fs::read(&other).unwrap(), b"different");
}
