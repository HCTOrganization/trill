//! `extract` is `bundle`'s inverse: what it writes, `pack` reads back.

#![cfg(feature = "extract")]

mod common;

use common::{manifest, target, TempDir};
use std::io::Write as _;
use trill_patch::layout::DEFAULT_TEMPLATE;
use trill_patch::{bundle, extract, Package};

/// An original Tango-era manifest: one global override, no per-ROM
/// tables, no README, no save templates.
const LEGACY_MANIFEST: &str = r#"
format = 1
name = "legacy"
version = "1.0.0"
title = "Legacy"

[rom_overrides]
language = "en-US"
charset = [" ", "A"]
"#;

/// Everything a package can hold: two ROMs, overrides, a README, and
/// both flavours of save template.
fn full() -> bundle::Builder {
    let mut manifest = manifest("test_patch", "1.2.3", "group:testing");
    manifest.rom_overrides.insert(
        target("BR5E_00"),
        trill_patch::Overrides {
            legal_chip_ranges: Some(vec![[1, 202], [221, 280], [301, 305]]),
            ..Default::default()
        },
    );
    let mut b = bundle::Builder::new(manifest);
    b.set_readme("# hello");
    b.add_rom(target("BR6E_00"), b"bps-6".to_vec());
    b.add_rom(target("BR5E_00"), b"bps-5".to_vec());
    b.add_save_template(target("BR6E_00"), DEFAULT_TEMPLATE, b"save".to_vec())
        .unwrap();
    b.add_save_template(target("BR6E_00"), "gregar", b"save-gregar".to_vec())
        .unwrap();
    b
}

#[test]
fn extraction_reproduces_the_tree_that_packs_back_to_the_same_bytes() {
    let packages = TempDir::new();
    let out = TempDir::new();
    let path = full().write_file(packages.path()).unwrap().path;

    let extracted = extract::to_dir(&path, out.path(), false).unwrap();

    assert_eq!(extracted.dir, out.path().join("test_patch-1.2.3"));
    assert_eq!(
        (extracted.roms, extracted.save_templates, extracted.has_readme),
        (2, 2, true)
    );
    assert_eq!(std::fs::read(extracted.dir.join("README.md")).unwrap(), b"# hello");
    assert_eq!(std::fs::read(extracted.dir.join("roms/BR6E_00.bps")).unwrap(), b"bps-6");
    assert_eq!(std::fs::read(extracted.dir.join("roms/BR5E_00.bps")).unwrap(), b"bps-5");
    assert_eq!(std::fs::read(extracted.dir.join("saves/BR6E_00.sav")).unwrap(), b"save");
    assert_eq!(
        std::fs::read(extracted.dir.join("saves/BR6E_00.gregar.sav")).unwrap(),
        b"save-gregar"
    );

    // The point of the tool: a released package can be edited and rebuilt.
    assert_eq!(
        bundle::read_dir(&extracted.dir).unwrap().to_vec().unwrap(),
        full().to_vec().unwrap()
    );
}

#[test]
fn an_original_tangopatch_extracts_with_an_upgraded_manifest() {
    let packages = TempDir::new();
    let out = TempDir::new();

    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    archive.start_file("manifest.toml", options).unwrap();
    archive.write_all(LEGACY_MANIFEST.as_bytes()).unwrap();
    archive.start_file("roms/BR5E_00.bps", options).unwrap();
    archive.write_all(b"bps-5").unwrap();
    archive.start_file("roms/BR6E_00.bps", options).unwrap();
    archive.write_all(b"bps-6").unwrap();
    // The extension is the only thing that made this a `.tangopatch`.
    let path = packages.path().join("legacy-1.0.0.tangopatch");
    std::fs::write(&path, archive.finish().unwrap().into_inner()).unwrap();

    let extracted = extract::to_dir(&path, out.path(), false).unwrap();

    assert_eq!(
        (extracted.roms, extracted.save_templates, extracted.has_readme),
        (2, 0, false)
    );
    assert!(!extracted.dir.join("saves").exists());
    let manifest = std::fs::read_to_string(extracted.dir.join("manifest.toml")).unwrap();
    assert!(manifest.contains("format = 2"), "{manifest}");
    assert!(manifest.contains("[rom_overrides.BR5E_00]"), "{manifest}");
    assert!(manifest.contains("[rom_overrides.BR6E_00]"), "{manifest}");

    // The upgrade survives the round trip back into a package.
    let repacked = bundle::read_dir(&extracted.dir).unwrap().to_vec().unwrap();
    let repacked = Package::read(std::io::Cursor::new(repacked)).unwrap();
    assert_eq!(repacked.manifest().format, 2);
    assert_eq!(repacked.manifest().rom_overrides.len(), 2);
    assert_eq!(repacked.targets().count(), 2);
}

#[test]
fn an_existing_directory_is_only_written_over_on_request() {
    let packages = TempDir::new();
    let out = TempDir::new();
    let path = full().write_file(packages.path()).unwrap().path;
    extract::to_dir(&path, out.path(), false).unwrap();

    let err = extract::to_dir(&path, out.path(), false).unwrap_err().to_string();
    assert!(err.contains("already exists"), "{err}");

    // Overwriting rewrites what extraction owns and leaves the rest of the
    // directory — an author's working copy — alone.
    let notes = out.path().join("test_patch-1.2.3/notes.txt");
    std::fs::write(&notes, b"keep me").unwrap();
    extract::to_dir(&path, out.path(), true).unwrap();

    assert_eq!(std::fs::read(&notes).unwrap(), b"keep me");
    assert_eq!(
        std::fs::read(out.path().join("test_patch-1.2.3/roms/BR6E_00.bps")).unwrap(),
        b"bps-6"
    );
}
