//! The forty-four-step committed walk, published as it happens.
//!
//! Same submissions, same confirmations, same byte comparisons, and the same
//! two real-clock waits as the sealed lane's runner; the only addition is
//! that every transition is also published to the bus so a browser can watch
//! it.  A step that the runner would fail on fails here, in the same place,
//! for the same reason.

use crate::bus::Bus;
use crate::decode;
use crate::plan::{Compare, Plan, Step};
use crate::rpc;
use serde_json::{json, Value};
use solana_keypair::{read_keypair_file, Keypair, Signer};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Terminal observed images, keyed by the plan's `(step, role)` vocabulary.
type Terminal = BTreeMap<(String, String), Vec<u8>>;

pub struct Walker<'a> {
    pub url: &'a str,
    pub plan_dir: &'a Path,
    pub plan: &'a Plan,
    pub bus: &'a Arc<Bus>,
    pub observed_dir: PathBuf,
}

/// Load exactly the ephemeral signers, and require the plan's roles among them.
pub fn load_keypairs(plan: &Plan, paths: &[PathBuf]) -> Result<Vec<Keypair>> {
    let keypairs: Vec<Keypair> = paths
        .iter()
        .map(read_keypair_file)
        .collect::<std::result::Result<_, _>>()?;
    let mut public_keys = BTreeSet::new();
    for keypair in &keypairs {
        if !public_keys.insert(keypair.pubkey().to_string()) {
            return Err("the same ephemeral signer was supplied more than once".into());
        }
    }
    for (role, address) in [
        ("payer", &plan.payer),
        ("actor", &plan.actor),
        ("holder", &plan.holder),
        ("trader_c", &plan.trader_c),
        ("trader_d", &plan.trader_d),
    ] {
        if !public_keys.contains(address) {
            return Err(format!("no supplied keypair matches plan {role} {address}").into());
        }
    }
    Ok(keypairs)
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    bytes
        .get(offset..offset + 8)
        .and_then(|slice| slice.try_into().ok())
        .map_or(0, u64::from_le_bytes)
}

impl Walker<'_> {
    /// Drive the whole plan.  Returns the terminal images the conservation
    /// epilogue re-derives from.
    pub fn run(&self, keypairs: &[Keypair]) -> Result<Terminal> {
        fs::create_dir_all(&self.observed_dir)?;
        let (watched, role_addresses) = self.plan.watch_index()?;
        let refs: Vec<&Keypair> = keypairs.iter().collect();
        let mut slots: BTreeMap<String, u64> = BTreeMap::new();
        let mut latest: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut terminal: Terminal = BTreeMap::new();

        for (ordinal, step) in self.plan.steps.iter().enumerate() {
            let ordinal = ordinal + 1;
            self.honour_waits(step, &slots)?;
            self.publish(&json!({"type": "step", "ordinal": ordinal, "state": "inflight"}));

            let before = if matches!(step.kind.as_str(), "refuse" | "exhausted") {
                Some(rpc::snapshot(self.url, &watched)?)
            } else {
                None
            };
            let unchanged: BTreeSet<String> = step
                .identical_to_pre
                .iter()
                .filter_map(|role| role_addresses.get(role).cloned())
                .collect();
            let unchanged_before = rpc::snapshot(self.url, &unchanged)?;

            let unsigned = crate::b64_decode(
                fs::read_to_string(self.plan_dir.join(&step.tx))?.trim(),
            )?;
            let signed =
                rpc::sign_transaction(&unsigned, rpc::latest_blockhash(self.url)?, &refs)?;
            let signature = rpc::submit(self.url, &signed)?;
            let status = rpc::await_confirmation(self.url, &signature)?;
            let error = status.get("err").cloned().unwrap_or(Value::Null);
            let slot = status
                .get("slot")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("{}: confirmation carries no slot", step.name))?;
            slots.insert(step.name.clone(), slot);

            match step.kind.as_str() {
                "accept" if error.is_null() => {}
                "accept" => {
                    return Err(format!("{}: expected success, got {error}", step.name).into())
                }
                _ => self.verify_nonaccepting(step, &error, &watched, before.as_ref())?,
            }

            self.reload(step, ordinal, &slots, &mut latest, &mut terminal)?;
            if rpc::snapshot(self.url, &unchanged)? != unchanged_before {
                return Err(format!(
                    "{}: identical-to-pre role changed ({})",
                    step.name,
                    step.identical_to_pre.join(", ")
                )
                .into());
            }

            self.publish(&json!({
                "type": "step",
                "ordinal": ordinal,
                "name": step.name,
                "state": if step.kind == "accept" { "accepted" } else { "refused" },
                "kind": step.kind,
                "confirmation": status.get("confirmationStatus").and_then(Value::as_str).unwrap_or("unknown"),
                "slot": slot,
                "cu": rpc::compute_units(self.url, &signature),
                "reloads": step.compare.len() + step.identical_to_pre.len(),
                "signature": signature,
            }));
            self.publish(&self.live_conservation(&latest));
        }
        Ok(terminal)
    }

    fn honour_waits(&self, step: &Step, slots: &BTreeMap<String, u64>) -> Result<()> {
        let mut tick = |now: u64, target: u64, reason: &str| {
            self.publish(&json!({
                "type": "clock", "slot": now, "target": target, "reason": reason,
                "remaining": target.saturating_sub(now),
            }));
        };
        if let Some(target) = step.wait_slot {
            rpc::wait_for_slot(self.url, target, "plan deadline", &mut tick)?;
        }
        if let Some(after) = &step.wait_after {
            let base = *slots
                .get(&after.step)
                .ok_or_else(|| format!("{}: wait_after names {}", step.name, after.step))?;
            let target = base
                .checked_add(after.delta)
                .ok_or("wait_after slot overflow")?;
            let reason = format!("{} + {}", after.step, after.delta);
            rpc::wait_for_slot(self.url, target, &reason, &mut tick)?;
        }
        Ok(())
    }

    fn verify_nonaccepting(
        &self,
        step: &Step,
        error: &Value,
        watched: &BTreeSet<String>,
        before: Option<&rpc::BankSnapshot>,
    ) -> Result<()> {
        match step.kind.as_str() {
            "refuse" => {
                let expected = step
                    .expect_code
                    .ok_or_else(|| format!("{}: refusal has no expect_code", step.name))?;
                if rpc::custom_error_code(error) != Some(expected) {
                    return Err(format!(
                        "{}: expected Custom({expected:#06x}), got {error}",
                        step.name
                    )
                    .into());
                }
            }
            "exhausted" if rpc::computational_budget_exhausted(error) => {}
            "exhausted" => {
                return Err(format!(
                    "{}: expected ComputationalBudgetExceeded, got {error}",
                    step.name
                )
                .into())
            }
            other => return Err(format!("{}: unknown step kind {other}", step.name).into()),
        }
        if before != Some(&rpc::snapshot(self.url, watched)?) {
            return Err(format!("{}: failed transaction changed watched state", step.name).into());
        }
        Ok(())
    }

    /// Reload, slot-patch, compare, record, and publish one step's accounts.
    fn reload(
        &self,
        step: &Step,
        ordinal: usize,
        slots: &BTreeMap<String, u64>,
        latest: &mut BTreeMap<String, Vec<u8>>,
        terminal: &mut Terminal,
    ) -> Result<()> {
        for entry in &step.compare {
            let observed = rpc::account_bytes(self.url, &entry.address)?
                .ok_or_else(|| format!("{} / {}: account is absent", step.name, entry.role))?;
            let expected = self.expected_image(step, entry, slots)?;
            fs::write(
                self.observed_dir
                    .join(format!("{}.{}.hex", step.name, entry.role)),
                format!("{}\n", clutch_sbf_harness::hex_encode(&observed)),
            )?;
            if observed != expected {
                return Err(format!(
                    "{} / {}: committed bytes differ (observed {}, expected {})",
                    step.name,
                    entry.role,
                    observed.len(),
                    expected.len()
                )
                .into());
            }
            self.publish(&json!({
                "type": "state",
                "ordinal": ordinal,
                "step": step.name,
                "role": entry.role,
                "address": entry.address,
                "bytes": observed.len(),
                "decoded": decode::by_role(&entry.role, &observed),
            }));
            latest.insert(entry.role.clone(), observed.clone());
            terminal.insert((step.name.clone(), entry.role.clone()), observed);
        }
        Ok(())
    }

    fn expected_image(
        &self,
        step: &Step,
        entry: &Compare,
        slots: &BTreeMap<String, u64>,
    ) -> Result<Vec<u8>> {
        let mut expected =
            rpc::hex_decode(&fs::read_to_string(self.plan_dir.join(&entry.expected))?)?;
        for patch in &entry.slot_patches {
            let base = *slots.get(&patch.base).ok_or_else(|| {
                format!(
                    "{} / {}: slot patch names unknown step {}",
                    step.name, entry.role, patch.base
                )
            })?;
            let value = base
                .checked_add(patch.delta)
                .ok_or("slot patch value overflow")?;
            let end = patch
                .offset
                .checked_add(8)
                .filter(|end| *end <= expected.len())
                .ok_or_else(|| {
                    format!("{} / {}: slot patch offset is past the image", step.name, entry.role)
                })?;
            expected[patch.offset..end].copy_from_slice(&value.to_le_bytes());
        }
        Ok(expected)
    }

    /// The conservation strip, re-derived from the bytes observed so far.
    ///
    /// Not read from the plan's `expected` block: every number below comes
    /// out of an account image this walk actually reloaded.  Roles not yet
    /// touched are named, so a partial strip cannot read as a whole one.
    fn live_conservation(&self, latest: &BTreeMap<String, Vec<u8>>) -> Value {
        let rules = &self.plan.conservation;
        let offsets = &rules.offsets;
        let mut cash = 0_u64;
        let mut eggs = [0_u64; 2];
        let mut rows = Vec::new();
        let mut pending = Vec::new();
        for entry in &rules.positions {
            let Some(bytes) = latest.get(&entry.role) else {
                pending.push(entry.role.clone());
                continue;
            };
            let row_cash = u64_at(bytes, offsets.position_cash);
            let held = [
                u64_at(bytes, offsets.position_internal0),
                u64_at(bytes, offsets.position_internal1),
            ];
            cash += row_cash;
            eggs[0] += held[0];
            eggs[1] += held[1];
            rows.push(json!({
                "role": entry.role,
                "cash": row_cash,
                "reserved": u64_at(bytes, offsets.position_reserved),
                "eggs": held,
            }));
        }
        let locked = latest
            .get(&rules.hoard.role)
            .map(|bytes| u64_at(bytes, offsets.hoard_collateral));
        let custody = latest
            .get(&rules.hoard_token.role)
            .map(|bytes| u64_at(bytes, offsets.token_amount));
        if locked.is_none() {
            pending.push(rules.hoard.role.clone());
        }
        if custody.is_none() {
            pending.push(rules.hoard_token.role.clone());
        }
        json!({
            "type": "conservation",
            "live": true,
            "complete": pending.is_empty(),
            "pending": pending,
            "rows": rows,
            "cash_total": cash,
            "eggs": eggs,
            "locked": locked,
            "custody": custody,
            "endowed_total": rules.endowed_total,
            "split_total": rules.split_total,
        })
    }

    fn publish(&self, event: &Value) {
        self.bus.publish(event);
    }
}

/// Re-derive the terminal identities from the runner's observed bytes.
///
/// This is the script's conservation epilogue, moved in process and reported
/// through the same bus.  It reads only images this walk reloaded; the plan's
/// `expected` block is the thing being checked, never the source of the
/// observation.
pub fn conservation_epilogue(plan: &Plan, terminal: &Terminal) -> (Value, bool) {
    let rules = &plan.conservation;
    let offsets = &rules.offsets;
    let image = |reference: &crate::plan::PositionRef| -> Vec<u8> {
        terminal
            .get(&(reference.step.clone(), reference.role.clone()))
            .cloned()
            .unwrap_or_default()
    };
    let mut cash = 0_u64;
    let mut eggs = [0_u64; 2];
    let mut rows = Vec::new();
    for entry in &rules.positions {
        let bytes = image(entry);
        let row_cash = u64_at(&bytes, offsets.position_cash);
        let held = [
            u64_at(&bytes, offsets.position_internal0),
            u64_at(&bytes, offsets.position_internal1),
        ];
        cash += row_cash;
        eggs[0] += held[0];
        eggs[1] += held[1];
        rows.push(json!({
            "role": entry.role, "cash": row_cash,
            "reserved": u64_at(&bytes, offsets.position_reserved), "eggs": held,
        }));
    }
    let locked = u64_at(&image(&rules.hoard), offsets.hoard_collateral);
    let custody = u64_at(&image(&rules.hoard_token), offsets.token_amount);
    let expected = &rules.expected;
    let checks = [
        ("position cash total", cash, expected.cash_total),
        ("eggs outcome 0", eggs[0], expected.eggs_outcome0),
        ("eggs outcome 1", eggs[1], expected.eggs_outcome1),
        ("locked backing", locked, expected.locked),
        ("pooled custody", custody, expected.custody),
    ];
    let identities = [
        (
            "cash_total + locked == endowed_total",
            cash + locked,
            rules.endowed_total,
        ),
        ("eggs[0] == split_total", eggs[0], rules.split_total),
        ("eggs[1] == split_total", eggs[1], rules.split_total),
        ("custody == endowed_total", custody, rules.endowed_total),
    ];
    let mut green = true;
    let render = |label: &str, got: u64, want: u64| {
        json!({"label": label, "observed": got, "expected": want, "ok": got == want})
    };
    let checks: Vec<Value> = checks
        .iter()
        .inspect(|(_, got, want)| green &= got == want)
        .map(|(label, got, want)| render(label, *got, *want))
        .collect();
    let identities: Vec<Value> = identities
        .iter()
        .inspect(|(_, got, want)| green &= got == want)
        .map(|(label, got, want)| render(label, *got, *want))
        .collect();
    (
        json!({
            "type": "conservation", "live": false, "complete": true, "pending": [],
            "rows": rows, "cash_total": cash, "eggs": eggs,
            "locked": locked, "custody": custody,
            "endowed_total": rules.endowed_total, "split_total": rules.split_total,
            "checks": checks, "identities": identities,
            "verdict": if green { "RE-DERIVED-FROM-OBSERVED-BYTES" } else { "FAIL" },
        }),
        green,
    )
}
