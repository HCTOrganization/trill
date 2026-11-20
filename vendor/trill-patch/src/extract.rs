//! Unpacking a package back into a source tree (features `package` +
//! `bundle`).
//!
//! The inverse of [`crate::bundle::read_dir`]: extracting a built package
//! reproduces the directory `pack` was pointed at, so a released package
//! can be inspected, edited, and rebuilt. That includes the original
//! Tango packages — a `.tangopatch` is the same zip under the old name,
//! so nothing here looks at the extension. A package carrying a legacy
//! manifest comes out with the current format, since the reader upgrades
//! it on the way in.

use crate::layout::{MANIFEST_PATH, README_PATH, ROMS_DIR, SAVES_DIR};
use crate::{Error, Package};
use std::path::{Path, PathBuf};

/// What [`to_dir`] wrote.
#[derive(Debug, Clone)]
pub struct Extracted {
    /// `<out_dir>/<name>-<version>/`.
    pub dir: PathBuf,
    pub roms: usize,
    pub save_templates: usize,
    pub has_readme: bool,
}

/// Unpack `package` into `<out_dir>/<name>-<version>/`.
///
/// An existing directory is refused unless `overwrite` is set, because
/// it's just as likely to be someone's working copy as a previous
/// extraction. Overwriting rewrites the files this function owns and
/// leaves anything else there alone: the flag means "redo this
/// extraction", not "delete whatever is at that path".
pub fn to_dir(package: &Path, out_dir: &Path, overwrite: bool) -> Result<Extracted, Error> {
    let mut package = Package::open(package)?;
    let dir = out_dir.join(package.manifest().stem());

    if dir.exists() && !overwrite {
        return Err(Error::Invalid(format!(
            "{} already exists; pass --force to overwrite it",
            dir.display()
        )));
    }
    // A package always has at least one BPS, so the roms directory is
    // unconditional; saves are created only if something lands in them.
    let roms_dir = dir.join(ROMS_DIR);
    for path in [&dir, &roms_dir] {
        std::fs::create_dir_all(path).map_err(|e| Error::from(e).at(path))?;
    }

    write_entry(&dir.join(MANIFEST_PATH), package.manifest().to_toml()?.as_bytes())?;

    let has_readme = match package.readme()? {
        Some(readme) => {
            write_entry(&dir.join(README_PATH), readme.as_bytes())?;
            true
        }
        None => false,
    };

    let mut roms = 0;
    let mut save_templates = 0;
    // Collected up front: the template iterator borrows the package that
    // each payload read then needs mutably.
    for target in package.targets().collect::<Vec<_>>() {
        write_entry(&dir.join(target.rom_path()), &package.bps(target)?)?;
        roms += 1;

        let templates = package.save_templates(target).map(str::to_owned).collect::<Vec<_>>();
        if templates.is_empty() {
            continue;
        }
        let saves_dir = dir.join(SAVES_DIR);
        if !saves_dir.is_dir() {
            std::fs::create_dir_all(&saves_dir).map_err(|e| Error::from(e).at(&saves_dir))?;
        }
        for template in templates {
            let save = package.save_template(target, &template)?;
            write_entry(&dir.join(target.save_path(&template)), &save)?;
            save_templates += 1;
        }
    }

    Ok(Extracted {
        dir,
        roms,
        save_templates,
        has_readme,
    })
}

fn write_entry(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    std::fs::write(path, bytes).map_err(|e| Error::from(e).at(path))
}
