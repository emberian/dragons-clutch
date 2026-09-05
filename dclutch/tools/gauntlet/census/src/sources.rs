//! One walk and one parse of every first-party Rust source, shared by every scanner.
//!
//! Until 2026-09-04 the constant index, the admission index, the refusal
//! sweep, the magic sweep, the preimage sweep and the per-program dispatch
//! walk each walked `crates/` and `programs/` and parsed every file on their
//! own -- five parses of one tree, and five places to disagree about what the
//! tree is. This is the one place.
//!
//! Order matters and is preserved: `crates/` then `programs/`, each sorted by
//! path, which is what every scanner iterated before, so a dedup that keeps
//! the first of two equal rows keeps the same row. A file belongs to the
//! INNERMOST package whose directory contains it, so a nested program-test
//! crate owns its own files rather than the package around it.

use std::{
    fs,
    path::{Path, PathBuf},
};

/// One parsed source file and the package that compiles it.
pub struct Source {
    pub path: PathBuf,
    /// Repo-relative, for provenance strings.
    pub relative: String,
    /// The innermost package directory containing the file, if any.
    pub package: Option<String>,
    pub text: String,
    pub file: syn::File,
}

pub struct Sources {
    /// Every package directory under `crates/` and `programs/`, innermost first.
    pub packages: Vec<(String, PathBuf)>,
    /// Every parsable `.rs` file under `crates/` then `programs/`, sorted within each.
    pub files: Vec<Source>,
}

impl Sources {
    pub fn load(root: &Path) -> Result<Sources, String> {
        let packages = crate::bands::package_directories(root)?;
        let mut files = Vec::new();
        for directory in ["crates", "programs"] {
            let base = root.join(directory);
            if !base.is_dir() {
                continue;
            }
            for path in crate::enumerate::rust_sources(&base)? {
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(file) = syn::parse_file(&text) else {
                    continue;
                };
                let package = packages
                    .iter()
                    .find(|(_, candidate)| path.starts_with(candidate))
                    .map(|(name, _)| name.clone());
                files.push(Source {
                    relative: crate::enumerate::relative(root, &path),
                    path,
                    package,
                    text,
                    file,
                });
            }
        }
        Ok(Sources { packages, files })
    }

    /// The parsed files under one directory, in tree order.
    pub fn under<'a>(&'a self, directory: &'a Path) -> impl Iterator<Item = &'a Source> + 'a {
        self.files
            .iter()
            .filter(move |source| source.path.starts_with(directory))
    }

    /// The files one package compiles: under its directory, and owned by it
    /// rather than by a package nested inside it.
    pub fn owned_by<'a>(
        &'a self,
        package: &'a str,
        directory: &'a Path,
    ) -> impl Iterator<Item = &'a Source> + 'a {
        self.under(directory)
            .filter(move |source| source.package.as_deref() == Some(package))
    }
}
