//! The batched-fold **wire** probe: how many Fold instructions actually fit.
//!
//! `research/liveness-policy-profile` seals the batched-fold row with one
//! honest caveat: `cluster_packet_budget: UNMODELED_BANK_TRANSPORT_ONLY`.  The
//! bank harness (`solana-program-test`) transports transactions in process, so
//! no packet is ever framed and the 1,232-byte legacy budget is never applied.
//! The sealed fewest-transaction plan `[12, 12, 8]` was chosen on **compute**
//! alone.
//!
//! This probe answers the transport half, two ways, and neither is a model:
//!
//! 1. **Serialization.** It builds the real `FoldResolutionWork` instruction
//!    data through the real layout encoder and the real reference envelope,
//!    composes N of them into a real legacy message with the real eight-role
//!    account set, serializes it exactly as the runtime frames a packet, and
//!    reports the byte count for every N.
//! 2. **Transport.** It hands the same bytes to a real validator's
//!    `sendTransaction`.  A packet inside the budget is admitted by transport
//!    (the program then refuses it on state, which is not the question being
//!    asked); a packet over the budget is refused by transport before any
//!    execution.  That refusal is the discharge.
//!
//! There is deliberately no attempt to make the folds *succeed*: standing up a
//! sealed `SourceArchive` on a live validator would measure the resolution
//! plane, and the caveat is about the wire.

use crate::pda::Deriver;
use crate::quotes;
use crate::rpc::Rpc;
use crate::wire::{self, Instruction, Message};
use clutch_solana_layout::resolution_work::FoldResolutionWorkV1;
use clutch_solana_layout::{Hash32, Intent};
use solana_keypair::{Keypair, Signer};

/// One measured batch width.
#[derive(Clone, Debug)]
pub struct Row {
    /// Fold instructions in the transaction.
    pub folds: u8,
    /// Exact serialized transaction bytes.
    pub bytes: usize,
    /// Whether it is inside the legacy packet budget.
    pub fits: bool,
    /// The compute limit the keeper would request for this width.
    pub limit_cu: u32,
    /// Whether a W1 row backs that limit.
    pub quoted: bool,
    /// Whether the batch's cost is inside the profile's raw admission bound.
    /// A batch can admit on compute and still not frame into a packet, which
    /// is the whole point of this probe.
    pub admits_on_compute: bool,
    /// What a real validator's `sendTransaction` did with these exact bytes.
    pub transport: Option<Transport>,
}

/// A real validator's transport verdict.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Transport {
    /// The packet was accepted for execution.
    Admitted,
    /// The packet was refused before execution.
    Refused,
}

/// The identities one resolution plane is anchored on.
#[derive(Clone, Debug)]
pub struct PlaneIds {
    /// Program id, base58.
    pub program_id_b58: String,
    /// Realm namespace.
    pub realm: Hash32,
    /// Market identity.
    pub market: Hash32,
    /// Feed identity the source spec and archive are keyed by.
    pub feed: Hash32,
    /// Window identity the sealed archive is keyed by.
    pub window: Hash32,
    /// Immutable terms digest.
    pub terms: Hash32,
}

/// The probe's whole answer.
#[derive(Clone, Debug)]
pub struct Answer {
    /// Every measured width, ascending.
    pub rows: Vec<Row>,
    /// The largest width inside the packet budget.
    pub largest_fitting: u8,
    /// Fixed bytes before the first Fold instruction.
    pub base_bytes: usize,
    /// Marginal bytes per additional Fold instruction.
    pub per_fold_bytes: usize,
    /// The corrected fewest-transaction plan, given the measured packet bound.
    pub plan: FoldPlan,
}

/// The fewest-transaction fold plan the packet answer actually admits.
///
/// The sealed projection chose `[12, 12, 8]` on compute alone.  Twelve Fold
/// instructions do not frame, so that plan cannot be sent.  What *can* be sent
/// is a packet-full of **record-dense** folds: `MAX_FOLD_RECORDS_V1` records
/// per instruction times the largest instruction count that frames.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoldPlan {
    /// Fold instructions per transaction — the packet bound.
    pub instructions_per_packet: u8,
    /// Records per Fold instruction — the layout bound.
    pub records_per_instruction: u8,
    /// Records one transaction can carry.
    pub records_per_packet: u32,
    /// Transactions needed for a whole 32-record work item.
    pub transactions_for_max_work: u32,
    /// Derived compute cost of a full record-dense packet.
    pub packet_cu: u64,
    /// The limit that cost quotes to, or `None` when it STOPs on headroom.
    pub packet_limit_cu: Option<u32>,
}

impl FoldPlan {
    /// Derive the plan from the measured instruction-count bound.
    #[must_use]
    pub fn from_packet_bound(instructions_per_packet: u8) -> Self {
        let records_per_instruction =
            clutch_solana_layout::resolution_work::MAX_FOLD_RECORDS_V1;
        let records_per_packet =
            u32::from(instructions_per_packet) * u32::from(records_per_instruction);
        let packet_cu = u64::from(quotes::RESOLUTION_FOLD_4_MEASURED_CU)
            * u64::from(instructions_per_packet);
        Self {
            instructions_per_packet,
            records_per_instruction,
            records_per_packet,
            transactions_for_max_work: if records_per_packet == 0 {
                0
            } else {
                quotes::RESOLUTION_MAX_RECORDS.div_ceil(records_per_packet)
            },
            packet_cu,
            packet_limit_cu: quotes::selected_limit(packet_cu),
        }
    }
}

/// Build one batch of `folds` singleton Fold instructions.
///
/// Returns the serialized, signed transaction.  The account set is the real
/// eight-role Fold shape plus the program and the compute-budget program.
fn batch(
    program_id: [u8; 32],
    worker: &Keypair,
    plane: &[[u8; 32]; 6],
    blockhash: &[u8; 32],
    folds: u8,
    limit_cu: u32,
) -> Result<Vec<u8>, String> {
    let [market, terms, source_spec, source_archive, work, reserve] = *plane;
    let clock = wire::base58_decode_32(wire::CLOCK_SYSVAR)?;
    let compute_budget = wire::base58_decode_32(wire::COMPUTE_BUDGET)?;
    let worker_key = worker.pubkey().to_bytes();
    let message = Message::new(
        &[worker_key],
        &[],
        &[work, reserve],
        &[
            market,
            terms,
            source_spec,
            source_archive,
            clock,
            program_id,
            compute_budget,
        ],
    );
    let roles = [
        worker_key,
        market,
        terms,
        source_spec,
        source_archive,
        work,
        reserve,
        clock,
    ];
    let mut instructions = vec![wire::compute_limit_instruction(
        &message,
        &compute_budget,
        limit_cu,
    )];
    for offset in 0..u64::from(folds) {
        let data = wire::request(
            0,
            &Intent::FoldResolutionWork(FoldResolutionWorkV1 {
                work_commitment: [0x11; 32],
                archive_account: source_archive,
                archive_commitment: [0x22; 32],
                // The optimistic cursor each successive Fold in the batch
                // expects: a real batch walks the cursor forward by one
                // record per instruction.
                expected_cursor: 100 + offset,
                record_count: 1,
            }),
        );
        instructions.push(Instruction {
            program_index: message.index(&program_id),
            accounts: message.indices(&roles),
            data,
        });
    }
    let mut transaction = wire::serialize(&message, &instructions, blockhash);
    wire::sign(&mut transaction, &[worker])?;
    Ok(transaction)
}

/// Measure every width in `widths`, optionally asking a live validator.
///
/// # Errors
/// Returns an error when a derivation, serialization, or RPC call fails.
pub fn run(
    plane_ids: &PlaneIds,
    widths: &[u8],
    rpc: Option<&Rpc>,
) -> Result<Answer, String> {
    let PlaneIds {
        program_id_b58,
        realm,
        market,
        feed,
        window,
        terms,
    } = plane_ids;
    let (realm, market, feed, window, terms) = (*realm, *market, *feed, *window, *terms);
    let program_id = wire::base58_decode_32(program_id_b58)?;
    let mut deriver = Deriver::new(program_id_b58);
    let market_account = deriver.find(&[
        clutch_sbf::seeds::SEED_MARKET,
        &realm.bytes(),
        &market.bytes(),
    ])?;
    let terms_account = deriver.find(&[
        clutch_sbf::seeds::SEED_TERMS,
        &realm.bytes(),
        &terms.bytes(),
    ])?;
    let source_spec = deriver.find(&[clutch_sbf::seeds::SEED_SOURCE_SPEC, &feed.bytes()])?;
    let source_archive = deriver.find(&[
        clutch_sbf::seeds::SEED_SOURCE_ARCHIVE,
        &feed.bytes(),
        &window.bytes(),
    ])?;
    let work = deriver.find(&[clutch_sbf::seeds::SEED_RESOLUTION_WORK, &market.bytes()])?;
    let reserve = deriver.find(&[
        clutch_sbf::seeds::SEED_RESOLUTION_RESERVE,
        &market.bytes(),
        &work.bytes,
    ])?;
    let plane = [
        market_account.bytes,
        terms_account.bytes,
        source_spec.bytes,
        source_archive.bytes,
        work.bytes,
        reserve.bytes,
    ];

    // A throwaway signer: the probe measures framing and transport, and never
    // needs an identity that owns anything.
    let worker = Keypair::new();
    let blockhash = match rpc {
        Some(rpc) => rpc.blockhash()?,
        None => [0x33; 32],
    };

    let mut rows = Vec::with_capacity(widths.len());
    for folds in widths.iter().copied() {
        let quote = quotes::fold_batch(folds);
        let transaction = batch(
            program_id,
            &worker,
            &plane,
            &blockhash,
            folds,
            quote.limit_cu,
        )?;
        let bytes = transaction.len();
        let fits = bytes <= wire::PACKET_BUDGET_BYTES;
        // Admitted by transport; the program's own refusal is a separate
        // question this probe deliberately does not ask.
        let transport = rpc.map(|rpc| {
            if rpc.submit(&transaction).is_ok() {
                Transport::Admitted
            } else {
                Transport::Refused
            }
        });
        rows.push(Row {
            folds,
            bytes,
            fits,
            limit_cu: quote.limit_cu,
            quoted: quote.quoted(),
            admits_on_compute: quotes::admits_on_compute(u64::from(
                quote.measured_cu.unwrap_or(quotes::RESOLUTION_FOLD_MEASURED_CU * u32::from(folds)),
            )),
            transport,
        });
    }

    let largest_fitting = rows
        .iter()
        .filter(|row| row.fits)
        .map(|row| row.folds)
        .max()
        .unwrap_or(0);
    // Two adjacent widths give the exact marginal cost and the exact base.
    let (base_bytes, per_fold_bytes) = match (rows.first(), rows.last()) {
        (Some(low), Some(high)) if high.folds > low.folds => {
            let per = (high.bytes - low.bytes) / usize::from(high.folds - low.folds);
            (low.bytes - per * usize::from(low.folds), per)
        }
        _ => (0, 0),
    };
    Ok(Answer {
        plan: FoldPlan::from_packet_bound(largest_fitting),
        rows,
        largest_fitting,
        base_bytes,
        per_fold_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fold_instruction_carries_the_frozen_envelope_and_body_widths() {
        let data = wire::request(
            0,
            &Intent::FoldResolutionWork(FoldResolutionWorkV1 {
                work_commitment: [1; 32],
                archive_account: [2; 32],
                archive_commitment: [3; 32],
                expected_cursor: 7,
                record_count: 1,
            }),
        );
        // 13-byte reference envelope + FOLD_RESOLUTION_WORK_BYTES (107).
        assert_eq!(
            data.len(),
            13 + clutch_solana_layout::resolution_work::FOLD_RESOLUTION_WORK_BYTES
        );
    }

    #[test]
    fn the_corrected_plan_beats_the_sealed_one_by_going_record_dense() {
        // Six instructions frame; twelve do not.  Four records per
        // instruction is the layout's own bound, so a framed packet still
        // carries twenty-four records.
        let plan = FoldPlan::from_packet_bound(6);
        assert_eq!(plan.records_per_packet, 24);
        assert_eq!(plan.transactions_for_max_work, 2);
        assert!(
            plan.packet_limit_cu.is_some(),
            "a full record-dense packet must still admit on compute"
        );
    }

    #[test]
    fn the_batch_grows_by_a_constant_per_fold() {
        let worker = Keypair::new();
        let plane = [[4_u8; 32], [5; 32], [6; 32], [7; 32], [8; 32], [9; 32]];
        let program = [10_u8; 32];
        let one = batch(program, &worker, &plane, &[0; 32], 1, 200_000).expect("one fold frames");
        let two = batch(program, &worker, &plane, &[0; 32], 2, 200_000).expect("two folds frame");
        let three =
            batch(program, &worker, &plane, &[0; 32], 3, 200_000).expect("three folds frame");
        assert_eq!(two.len() - one.len(), three.len() - two.len());
    }
}
