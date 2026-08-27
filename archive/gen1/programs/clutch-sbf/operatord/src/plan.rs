//! The committed plan, exactly as `clutch_sbf_harness` emits it.
//!
//! This is a reader, never a writer.  The plan is produced by calling
//! `clutch_sbf_harness::emit_general_committed_plan` in process; these types
//! only describe what that emitter wrote so the daemon can drive it and the
//! browser can be shown it.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// One general-clearing committed plan.
#[derive(Debug, Deserialize)]
pub struct Plan {
    pub program_id: String,
    pub payer: String,
    pub actor: String,
    pub holder: String,
    pub trader_c: String,
    pub trader_d: String,
    /// True while the validator must preload zeroed, program-owned market
    /// PDAs that an ordinary wallet cannot create.  Never a lifecycle a
    /// permissionless caller could have driven from a blank bank.
    pub genesis_assisted: bool,
    #[serde(default)]
    pub precreated_program_accounts: Vec<String>,
    pub conservation: Conservation,
    pub steps: Vec<Step>,
}

/// The conservation-offset table: which reloads carry the terminal values,
/// where in each account image they sit, and what they must sum to.
#[derive(Debug, Deserialize)]
pub struct Conservation {
    pub endowed_total: u64,
    pub split_total: u64,
    pub offsets: Offsets,
    pub positions: Vec<PositionRef>,
    pub hoard: PositionRef,
    pub hoard_token: PositionRef,
    pub expected: Expected,
}

#[derive(Debug, Deserialize)]
pub struct Offsets {
    pub position_cash: usize,
    pub position_reserved: usize,
    pub position_internal0: usize,
    pub position_internal1: usize,
    pub hoard_collateral: usize,
    pub token_amount: usize,
}

#[derive(Debug, Deserialize)]
pub struct PositionRef {
    pub step: String,
    pub role: String,
}

#[derive(Debug, Deserialize)]
pub struct Expected {
    pub cash_total: u64,
    pub eggs_outcome0: u64,
    pub eggs_outcome1: u64,
    pub locked: u64,
    pub custody: u64,
}

#[derive(Debug, Deserialize)]
pub struct Step {
    pub name: String,
    pub family: String,
    pub oracle: String,
    pub note: String,
    pub tx: String,
    pub kind: String,
    pub bytes: usize,
    pub instructions: usize,
    #[serde(default)]
    pub compute_limit: Option<u32>,
    #[serde(default)]
    pub expect_code: Option<u64>,
    #[serde(default)]
    pub expect_code_hex: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub compare: Vec<Compare>,
    #[serde(default)]
    pub identical_to_pre: Vec<String>,
    /// Absolute slot this step must not be submitted before.
    #[serde(default)]
    pub wait_slot: Option<u64>,
    /// Relative wait: `slot(step) + delta` must be reached first.
    #[serde(default)]
    pub wait_after: Option<WaitAfter>,
}

#[derive(Debug, Deserialize)]
pub struct WaitAfter {
    pub step: String,
    pub delta: u64,
}

#[derive(Debug, Deserialize)]
pub struct Compare {
    pub role: String,
    pub address: String,
    pub expected: String,
    /// Slot-valued fields of the expected image, filled from observed
    /// confirmation slots.  The bank's clock is the one plan input a
    /// pregenerated expectation cannot carry.
    #[serde(default)]
    pub slot_patches: Vec<SlotPatch>,
}

#[derive(Debug, Deserialize)]
pub struct SlotPatch {
    pub offset: usize,
    pub base: String,
    pub delta: u64,
}

impl Plan {
    /// Read the plan the emitter wrote into `dir`.
    pub fn load(dir: &Path) -> Result<Self> {
        let plan: Self = serde_json::from_slice(&fs::read(dir.join("committed.json"))?)?;
        if plan.genesis_assisted && plan.precreated_program_accounts.is_empty() {
            return Err("genesis-assisted plan does not enumerate its precreated accounts".into());
        }
        Ok(plan)
    }

    /// Every watched address, and the one address each role maps to.
    ///
    /// A role that names two addresses is a plan bug, not a display problem,
    /// so it is refused here rather than rendered.
    pub fn watch_index(&self) -> Result<(BTreeSet<String>, BTreeMap<String, String>)> {
        let mut watched = BTreeSet::new();
        let mut roles: BTreeMap<String, String> = BTreeMap::new();
        for entry in self.steps.iter().flat_map(|step| step.compare.iter()) {
            watched.insert(entry.address.clone());
            if let Some(previous) = roles.insert(entry.role.clone(), entry.address.clone()) {
                if previous != entry.address {
                    return Err(format!(
                        "role {} maps to both {} and {}",
                        entry.role, previous, entry.address
                    )
                    .into());
                }
            }
        }
        for step in &self.steps {
            for role in &step.identical_to_pre {
                if !roles.contains_key(role) {
                    return Err(format!("{}: unknown identical-to-pre role {role}", step.name).into());
                }
            }
        }
        Ok((watched, roles))
    }
}
