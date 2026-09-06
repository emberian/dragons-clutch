//! WHICH RENT THE DEPLOYED RESOLUTION PRICES A CLOSURE RECEIPT WITH.
//!
//! `SourceClosureReceiptV3` partitions the funding ledger's lamports into a
//! remaining native principal, a rent reserve (`ledger_rent_lamports`, offset
//! 376) and everything above them (`ledger_lamport_surplus`, offset 384). The
//! sum of the three is the ledger's own balance and is what conservation cares
//! about. The SPLIT between the last two is a choice of rate, and the program
//! that executes makes it -- not the host that predicts it, and not the ruling
//! this tree currently believes.
//!
//! Two questions, and this tree kept answering both with one number:
//!
//!   * WHAT RENT DID THIS ACCOUNT ALREADY PAY. Answered by
//!     `funded_rent_recovery_v1`: the rate recovered from the account's own
//!     bytes, which is what every guard on an account a founding already
//!     bought must use. `ec373d90d` taught the terminal session exactly that
//!     and was right to.
//!   * WHAT WILL THE DEPLOYED PROGRAM WRITE. Answered by the deployment, whose
//!     Resolution may predate this tree by any amount. Cohort-15's prices the
//!     ledger from the Rent sysvar OF THE MOMENT, so when devnet dropped its
//!     rate from 6,333 to 5,080 lamports per byte at the epoch-1141 boundary
//!     with that cohort live, the program wrote `392 x 5,080 = 1,991,360` and
//!     a surplus of 491,176 while the host predicted `392 x 6,333 = 2,482,536`
//!     and a surplus of zero. Both were right about their own question. Their
//!     sum, 2,482,536, was identical -- which is why the disagreement is a
//!     partition and not a loss.
//!
//! Measured on devnet 2026-09-04: the first `ResolutionCloseFund` ever to
//! execute (`3rDH7V5X...`, slot 493,003,631) wrote a 416-byte receipt that
//! differed from its plan in exactly those two u64s and in `closed_at`, and in
//! no other byte.
//!
//! So the answer is keyed on the DEPLOYMENT, by the same argument and the same
//! table shape `core_bump_projection.rs` uses for Core's Product-graph
//! nibbles: a deployment's identity is its bytes, a row can be written the
//! moment the candidate is built, and a driver with no statement about a
//! deployed cohort refuses rather than guesses.

use solana_program::rent::Rent;

use crate::{
    Error, Result,
    model::{CheckedLocalMutableSetPinV1, ProgramPin},
};

/// The Resolution role's name in a plan and in a substrate reading.
const RESOLUTION_ROLE_NAME_V1: &str = "resolution";

/// A plan pin whose deployment is bytes the chain was already holding. The
/// other value, `"genesis-install"`, describes an ELF this run installs itself
/// out of the plan's own checked candidate.
const OBSERVED_DEPLOYMENT_SOURCE_V1: &str = "observed-programdata-account";

/// How a deployed Resolution splits a closing ledger's lamports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DeployedClosureRentRuleV1 {
    /// `rent.minimum_balance(len)` off the Rent sysvar the instruction is
    /// executing against. Every Resolution built before the funded-rate change.
    LiveRentSysvar,
    /// The rate the ledger was FUNDED at, recovered from or recorded in its own
    /// bytes. The shape RENT-FLOORS/PROGRAMS-16 rules a future program into.
    FundedRate,
}

impl DeployedClosureRentRuleV1 {
    const fn name(self) -> &'static str {
        match self {
            Self::LiveRentSysvar => "the live Rent sysvar",
            Self::FundedRate => "the rate the ledger was funded at",
        }
    }
}

/// Deployed Resolutions that price a closing ledger from the live Rent sysvar.
///
/// Keyed by the CHECKED CANDIDATE ELF digest -- the exact raw build output the
/// release gate certifies and the deploy runbook records. Append-only: a row
/// describes a cohort that exists on some chain, and deleting one would make
/// that cohort's closure receipts unpredictable again.
const LIVE_RENT_SYSVAR_CLOSURE_RESOLUTION_ELF_SHA256_V1: [&str; 2] = [
    // cohort-17, 846,656 bytes, deployed to devnet as
    // gYWBUAqMzr5V6HzvB8xhTETUZGdPSDr7dD5A3raqPGt at slot 493,941,536, from
    // deploy commit 932edc83fc5a108fa362be216c84a3b0f78f29b4. These are also
    // cohort-16's and cohort-16.1's bytes, which never reached a closure.
    //
    // The rule is MEASURED IN THE BYTES' OWN SOURCE, not inherited: at that
    // commit `programs/dclutch-resolution-proof-sbf/src/core_effect.rs:367`
    // computes `ledger_rent_lamports = rent.minimum_balance(
    // RESOLUTION_FUNDING_LEDGER_BYTES)` and `programs/dclutch-core-sbf/src/
    // resolution.rs:542` does the same. The one place a funded rate is named in
    // that program is `pre_market_funding_v1.rs:146`, which DERIVES it when the
    // ledger is CREATED -- that is how the rate reaches offset 12 and it is not
    // the closure. So the closing ledger is priced from the Rent sysvar of the
    // executing slot, which is this list. RENT-FLOORS/PROGRAMS-16 has not
    // landed, and FUNDED_RATE_CLOSURE_RESOLUTION_ELF_SHA256_V1 is still empty.
    "7be8a398be52342546a953cccc04b7276411041eca0081990d1e43be5ed7c34b",
    // cohort-15, 829,888 bytes, deployed to devnet as
    // 24AkUjtXg61La45u7KTge8u4dKpVqkzirmzycVyckFgn at slot 492,745,773. The
    // deploy commit is 1cae26fd61defbefd20bcd52acc449b6e94e64ed, which predates
    // `afab02c25` -- so nothing in these bytes can consult a funded rate,
    // because the recovery did not exist when they were built. Confirmed on
    // chain: receipt AmSYWY9vyWbryAkf16LJPgKawHvtuxUVSSfvGgFTi5MB reads
    // ledger_rent_lamports 1,991,360 = 392 x 5,080, the sysvar rate of the
    // executing slot, against a ledger funded at 6,333.
    "24af85048c086c28b529960f2b785b58a794026c73a7e7799aa6da6df340ac9d",
];

/// Deployed Resolutions that price a closing ledger from its funded rate.
///
/// Empty on purpose, and the emptiness is a measurement rather than an
/// omission: no program in this tree consults a funded rate today.
/// `programs/dclutch-resolution-proof-sbf` and
/// `programs/dclutch-core-sbf/src/resolution.rs` both reach the partition
/// through `rent.minimum_balance`. The first cohort deployed after
/// RENT-FLOORS/PROGRAMS-16 lands takes the first row here, and it can be
/// written from the candidate the release gate certifies, before the deploy is
/// paid for.
const FUNDED_RATE_CLOSURE_RESOLUTION_ELF_SHA256_V1: [&str; 0] = [];

/// The digest that names a deployment, in the order the plan spells it.
fn pin_elf_sha256_v1(pin: &ProgramPin) -> &str {
    if pin.checked_candidate_elf_sha256.is_empty() {
        &pin.elf_sha256
    } else {
        &pin.checked_candidate_elf_sha256
    }
}

/// Did THIS RUN install those bytes, out of the plan's own checked candidate.
///
/// The same narrow question `core_bump_projection.rs` asks, for the same
/// reason: a local-mutable profile installs the seven checked ELFs into genesis
/// and then authenticates them by reading the ProgramData accounts back, so its
/// pin says `observed-programdata-account` about bytes it wrote itself, and the
/// candidate digest is HOST-DEPENDENT (platform-tools embeds its CI build path
/// in the standard library), so hand-recording it would need one row per commit
/// per builder OS forever.
fn installed_into_this_runs_genesis_v1(
    resolution: &ProgramPin,
    local_mutable: Option<&CheckedLocalMutableSetPinV1>,
) -> bool {
    let Some(set) = local_mutable else {
        return false;
    };
    let digest = pin_elf_sha256_v1(resolution);
    if digest.is_empty() {
        return false;
    }
    set.roles.iter().any(|role| {
        role.role == RESOLUTION_ROLE_NAME_V1
            && role.program_id == resolution.program_id
            && role.checked_candidate_elf_sha256 == digest
            && role.live_elf_sha256 == digest
    })
}

/// How the Resolution a plan names splits a closing ledger's lamports.
///
/// A pin that installs THIS TREE's checked candidate gets this tree's answer,
/// which is `LiveRentSysvar` and is a property of the program sources beside
/// this file. A pin that OBSERVED a ProgramData account is describing bytes
/// deployed at some other time by some other commit; for those the driver
/// either has a recorded statement or has none, and having none is a refusal.
pub(crate) fn deployed_closure_rent_rule_v1(
    resolution: &ProgramPin,
    local_mutable: Option<&CheckedLocalMutableSetPinV1>,
) -> Result<DeployedClosureRentRuleV1> {
    let digest = pin_elf_sha256_v1(resolution);
    if LIVE_RENT_SYSVAR_CLOSURE_RESOLUTION_ELF_SHA256_V1.contains(&digest) {
        return Ok(DeployedClosureRentRuleV1::LiveRentSysvar);
    }
    if FUNDED_RATE_CLOSURE_RESOLUTION_ELF_SHA256_V1.contains(&digest) {
        return Ok(DeployedClosureRentRuleV1::FundedRate);
    }
    if resolution.deployment_source != OBSERVED_DEPLOYMENT_SOURCE_V1 {
        return Ok(DeployedClosureRentRuleV1::LiveRentSysvar);
    }
    if installed_into_this_runs_genesis_v1(resolution, local_mutable) {
        return Ok(DeployedClosureRentRuleV1::LiveRentSysvar);
    }
    Err(Error::new(format!(
        "REFUSED: this driver states no closure rent rule for the deployed Resolution \
         {} (checked candidate {digest}), so it cannot predict how that program will split a \
         closing funding ledger between ledger_rent_lamports and ledger_lamport_surplus. Record \
         the deployment's rule beside its candidate digest in closure_receipt_projection.rs",
        resolution.program_id,
    )))
}

/// One deployed program's split of a closing funding ledger's lamports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ClosureRentPartitionV1 {
    pub(crate) ledger_rent_lamports: u64,
    pub(crate) ledger_lamport_surplus: u64,
}

/// Reprice one closure receipt's rent partition under the deployed rule.
///
/// `funded` is the funded-rate answer the semantic operator computed, and its
/// two components' SUM is the invariant this repricing may never move: it is
/// the ledger balance the close discharges, minus the principal, and no rate
/// changes it. So the sum is checked exactly, both before and after; only the
/// boundary inside it moves.
pub(crate) fn project_closure_rent_partition_v1(
    rule: DeployedClosureRentRuleV1,
    ledger_data_len: usize,
    live_rent: &Rent,
    funded: ClosureRentPartitionV1,
) -> Result<ClosureRentPartitionV1> {
    let invariant = funded
        .ledger_rent_lamports
        .checked_add(funded.ledger_lamport_surplus)
        .ok_or_else(|| {
            Error::new("REFUSED: closure receipt rent partition overflowed its own sum")
        })?;
    let projected = match rule {
        DeployedClosureRentRuleV1::FundedRate => funded,
        DeployedClosureRentRuleV1::LiveRentSysvar => {
            let ledger_rent_lamports = live_rent.minimum_balance(ledger_data_len);
            let ledger_lamport_surplus =
                invariant.checked_sub(ledger_rent_lamports).ok_or_else(|| {
                    Error::new(format!(
                        "REFUSED: the deployed Resolution prices {ledger_data_len} bytes at \
                         {ledger_rent_lamports} lamports off the live Rent sysvar, which is more \
                         than the {invariant} lamports this closing ledger holds above its \
                         remaining principal; the receipt this plan predicts could not be written"
                    ))
                })?;
            ClosureRentPartitionV1 {
                ledger_rent_lamports,
                ledger_lamport_surplus,
            }
        }
    };
    if projected
        .ledger_rent_lamports
        .checked_add(projected.ledger_lamport_surplus)
        != Some(invariant)
    {
        return Err(Error::new(format!(
            "REFUSED: projecting a closure receipt under {} moved its rent partition's sum away \
             from {invariant}",
            rule.name(),
        )));
    }
    Ok(projected)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cohort-15's Resolution, exactly as `plan-seal.json` spells it.
    const COHORT_15_RESOLUTION_ELF_SHA256: &str =
        "24af85048c086c28b529960f2b785b58a794026c73a7e7799aa6da6df340ac9d";

    fn pin(digest: &str) -> ProgramPin {
        pin_from(digest, OBSERVED_DEPLOYMENT_SOURCE_V1)
    }

    fn pin_from(digest: &str, deployment_source: &str) -> ProgramPin {
        ProgramPin {
            program_id: "24AkUjtXg61La45u7KTge8u4dKpVqkzirmzycVyckFgn".into(),
            programdata_id: "B8N72QJgwKypTfgr9TsLDtKWjrCFeJ9n7FjsjV1xgB71".into(),
            elf_path: String::new(),
            elf_sha256: digest.into(),
            checked_candidate_elf_path: String::new(),
            checked_candidate_elf_sha256: digest.into(),
            live_elf_sha256: digest.into(),
            live_elf_padding_bytes: 0,
            semantic_release_id: String::new(),
            artifact_release_id: String::new(),
            upgrade_authority: None,
            deployment_slot: 492_745_773,
            deployment_source: deployment_source.into(),
            programdata_sha256: String::new(),
        }
    }

    /// The exact figures the chain wrote for market 1.
    const MARKET_1_LEDGER_BYTES: usize = 264;
    const MARKET_1_FUNDED_RENT: u64 = 2_482_536;
    const MARKET_1_CHAIN_RENT: u64 = 1_991_360;
    const MARKET_1_CHAIN_SURPLUS: u64 = 491_176;

    /// Devnet's post-epoch-1141 rate: 392 x 5,080 = 1,991,360 over 264 bytes.
    fn devnet_rent_after_epoch_1141() -> Rent {
        Rent {
            lamports_per_byte_year: 2_540,
            exemption_threshold: 2.0,
            burn_percent: 50,
        }
    }

    fn funded() -> ClosureRentPartitionV1 {
        ClosureRentPartitionV1 {
            ledger_rent_lamports: MARKET_1_FUNDED_RENT,
            ledger_lamport_surplus: 0,
        }
    }

    #[test]
    fn cohort_15s_resolution_prices_a_closing_ledger_from_the_live_sysvar() {
        assert_eq!(
            deployed_closure_rent_rule_v1(&pin(COHORT_15_RESOLUTION_ELF_SHA256), None)
                .expect("cohort-15's Resolution has a recorded rule"),
            DeployedClosureRentRuleV1::LiveRentSysvar
        );
    }

    #[test]
    fn the_live_sysvar_rule_reproduces_the_receipt_the_chain_wrote() {
        let projected = project_closure_rent_partition_v1(
            DeployedClosureRentRuleV1::LiveRentSysvar,
            MARKET_1_LEDGER_BYTES,
            &devnet_rent_after_epoch_1141(),
            funded(),
        )
        .expect("the projection holds the sum");
        assert_eq!(
            projected,
            ClosureRentPartitionV1 {
                ledger_rent_lamports: MARKET_1_CHAIN_RENT,
                ledger_lamport_surplus: MARKET_1_CHAIN_SURPLUS,
            }
        );
        assert_eq!(
            projected.ledger_rent_lamports + projected.ledger_lamport_surplus,
            MARKET_1_FUNDED_RENT
        );
    }

    #[test]
    fn the_funded_rate_rule_keeps_the_operators_own_partition() {
        assert_eq!(
            project_closure_rent_partition_v1(
                DeployedClosureRentRuleV1::FundedRate,
                MARKET_1_LEDGER_BYTES,
                &devnet_rent_after_epoch_1141(),
                funded(),
            )
            .expect("the funded rule changes nothing"),
            funded()
        );
    }

    /// THE WRONG RULE FOR THE DEPLOYMENT REFUSES BY NAME.
    ///
    /// A deployment nobody has recorded is the only shape this can take at the
    /// door: the two rules disagree by 491,176 lamports on market 1's own
    /// receipt, so guessing is exactly the guess this table exists to forbid.
    #[test]
    fn a_deployment_with_no_recorded_rule_refuses_by_name() {
        let error = deployed_closure_rent_rule_v1(&pin(&"ab".repeat(32)), None)
            .expect_err("an unrecorded deployment is a refusal");
        let text = error.to_string();
        assert!(
            text.contains("states no closure rent rule for the deployed Resolution")
                && text.contains("24AkUjtXg61La45u7KTge8u4dKpVqkzirmzycVyckFgn"),
            "{text}"
        );
    }

    /// A pin this run installed itself gets this tree's answer, so the loopback
    /// lifecycle needs no row: the programs beside this file reach the
    /// partition through `rent.minimum_balance`.
    #[test]
    fn a_resolution_this_run_installs_needs_no_row() {
        let digest = "cd".repeat(32);
        assert_eq!(
            deployed_closure_rent_rule_v1(&pin_from(&digest, "genesis-install"), None)
                .expect("an installed checked candidate is this tree's Resolution"),
            DeployedClosureRentRuleV1::LiveRentSysvar
        );
        assert!(
            deployed_closure_rent_rule_v1(&pin(&digest), None).is_err(),
            "the same digest observed on a chain this run did not write is still unrecorded"
        );
    }

    /// A RECEIPT WHOSE SUM MOVED REFUSES.
    ///
    /// The rate is a partition, never a source of lamports: a live rate that
    /// prices the ledger above everything it holds describes a receipt the
    /// program could not have written.
    #[test]
    fn a_live_rate_above_the_ledgers_own_lamports_refuses() {
        let error = project_closure_rent_partition_v1(
            DeployedClosureRentRuleV1::LiveRentSysvar,
            MARKET_1_LEDGER_BYTES,
            &Rent {
                lamports_per_byte_year: 1_000_000,
                exemption_threshold: 2.0,
                burn_percent: 50,
            },
            funded(),
        )
        .expect_err("a rate the ledger cannot pay is a refusal");
        assert!(
            error.to_string().contains("more than the 2482536 lamports"),
            "{error}"
        );
    }
}
