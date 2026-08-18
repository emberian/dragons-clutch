//! The machine-readable §2 registry and the TAX-6 coarsening relation.

use std::collections::BTreeMap;

use crate::json::Value;
use crate::sha256;

/// One registry row.
#[derive(Clone, Debug)]
pub struct CodeRow {
    pub name: String,
    pub by_design: bool,
    /// TAX-6: the exact codes this code is allowed to stand in for.
    pub coarsens: Vec<u32>,
}

/// The loaded taxonomy, plus the digest a manifest must pin under DIG-3.
#[derive(Debug)]
pub struct Taxonomy {
    pub version: u64,
    pub digest: String,
    codes: BTreeMap<u32, CodeRow>,
}

impl Taxonomy {
    pub fn load(text: &str) -> Result<Self, String> {
        let value = crate::json::parse(text)?;
        let digest = sha256::hex(value.to_jcs().as_bytes());
        let version = value.require("taxonomy_version")?.as_small()?;
        let mut codes = BTreeMap::new();
        for row in value.require("codes")?.as_array()? {
            let code = u32::try_from(row.require("code")?.as_small()?)
                .map_err(|_| "taxonomy code out of range".to_string())?;
            let name = row.require("name")?.as_str()?.to_string();
            let by_design = row.require("by_design")?.as_bool()?;
            let mut coarsens = Vec::new();
            if let Some(list) = row.get("coarsens") {
                for item in list.as_array()? {
                    coarsens.push(
                        u32::try_from(item.as_small()?)
                            .map_err(|_| "coarsens entry out of range".to_string())?,
                    );
                }
            }
            if codes
                .insert(
                    code,
                    CodeRow {
                        name: name.clone(),
                        by_design,
                        coarsens,
                    },
                )
                .is_some()
            {
                return Err(format!("taxonomy code {code} is declared twice"));
            }
        }
        Ok(Self {
            version,
            digest,
            codes,
        })
    }

    pub fn row(&self, code: u32) -> Result<&CodeRow, String> {
        self.codes
            .get(&code)
            .ok_or_else(|| format!("code {code} is not defined by the pinned taxonomy (VER-8)"))
    }

    pub fn len(&self) -> usize {
        self.codes.len()
    }

    /// TAX-6 / D5: an executor's `observed` code satisfies `expected` iff it is
    /// the same code, or `observed` declares a coarsening that contains
    /// `expected`.  Never a sibling, never an unrelated code.
    pub fn accepts(&self, expected: u32, observed: u32) -> Result<Coarsening, String> {
        if expected == observed {
            return Ok(Coarsening::Exact);
        }
        let row = self.row(observed)?;
        if row.coarsens.contains(&expected) {
            Ok(Coarsening::Coarsened { to: observed })
        } else {
            Err(format!(
                "expected {expected} ({}), observed {observed} ({}), and {observed} declares no coarsening over {expected}",
                self.row(expected)?.name,
                row.name
            ))
        }
    }
}

/// How an observation satisfied an expectation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Coarsening {
    Exact,
    Coarsened { to: u32 },
}

/// The taxonomy value carried by an implementation refusal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Refusal {
    pub code: u32,
    pub frame: &'static str,
    /// The Rust variant, verbatim, for the report.
    pub variant: String,
}

impl Refusal {
    pub fn new(code: u32, frame: &'static str, variant: impl Into<String>) -> Self {
        Self {
            code,
            frame,
            variant: variant.into(),
        }
    }
}

/// What one executor returned for one step.
#[derive(Clone, Debug)]
pub enum Observed {
    /// A successful transition, with the canonical success value (COMP-7).
    Ok(Value),
    /// An `Err` on the implementation's own result type.
    Error(Refusal),
    /// A refusal carried inside a successful value (TAX-4).
    ///
    /// No surface this executor drives returns one yet: the only landed
    /// refusal-valued result is `clutch_vertical_model::ResolveDecision::Refused`,
    /// and the `model` family has no vector here.  The variant exists so that
    /// TAX-4's distinction is representable rather than collapsed the first
    /// time a model vector lands.
    #[allow(dead_code)]
    Refused(Refusal),
}
