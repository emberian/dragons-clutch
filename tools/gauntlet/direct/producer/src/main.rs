//! Tier 2 (Direct family) gauntlet campaign producer — stateless AOT accelerator.
//!
//! Submits a fixed, labelled set of real transactions to the real
//! `dclutch-direct-aot-sbf` ELF under `solana-program-test`, asserts the
//! semantics of every one, and emits the gauntlet's campaign evidence
//! document plus the label -> program-address map the census folds.
//!
//! This is a ProgramTest FAST LANE, not a validator campaign. `tools/gauntlet/
//! TIERS.md` sets four conditions on a fast lane; this producer satisfies all
//! four and states each one in the evidence document's `fast_lane` block, so a
//! reader never has to take the claim on trust. The census records the
//! campaign name with every observation it admits, and the report prints it.
//!
//! Nothing here signs against a real cluster, reads a wallet, or touches the
//! network. The only key material is an ephemeral in-memory ProgramTest payer.

use std::{collections::BTreeMap, env, fs, path::PathBuf, process::ExitCode};

use dclutch_core_contract::ContentId;
use dclutch_direct_aot_contract::{
    BUY_SIDE_V2, OPEN_PHASE_V2, SCALAR_BUYER_CLAIMS, SCALAR_BUYER_COLLATERAL, SCALAR_BUYER_FEE_BPS,
    SCALAR_BUYER_FROM, SCALAR_BUYER_GENERATION, SCALAR_BUYER_LIFECYCLE, SCALAR_BUYER_LIMIT,
    SCALAR_BUYER_MAXIMUM, SCALAR_BUYER_OUTCOME, SCALAR_BUYER_SIDE, SCALAR_BUYER_THROUGH,
    FEE_DENOMINATOR_V2, IDENTITY_BUYER_MAKER, IDENTITY_BUYER_MARKET, IDENTITY_SELLER_MAKER,
    IDENTITY_SELLER_MARKET, SCALAR_BUYER_NONCE, SCALAR_BUYER_NEXT_NONCE, SCALAR_EXECUTION_PRICE,
    SCALAR_FEE_OUTPUT, SCALAR_FILL, SCALAR_GROSS_OUTPUT,
    SCALAR_OUTCOME_COUNT, SCALAR_PHASE, SCALAR_POLICY_FEE_BPS, SCALAR_PRICE_SCALE,
    SCALAR_SELLER_CLAIMS, SCALAR_SELLER_COLLATERAL, SCALAR_SELLER_FEE_BPS, SCALAR_SELLER_FROM,
    SCALAR_SELLER_GENERATION, SCALAR_SELLER_LIFECYCLE, SCALAR_SELLER_LIMIT, SCALAR_SELLER_MAXIMUM,
    SCALAR_SELLER_OUTCOME, SCALAR_SELLER_SIDE, SCALAR_SELLER_THROUGH, SCALAR_SLOT,
    SCALAR_SELLER_NEXT_NONCE, SCALAR_VENUE_COLLATERAL, SELL_SIDE_V2,
};
use dclutch_direct_aot_sbf::{
    DIRECT_AOT_ACCEPTED_ACK_BYTES_V1, DIRECT_AOT_BANK_BYTES_V1, DIRECT_AOT_IDENTITIES_V1,
    DIRECT_AOT_REFUSED_ACK_BYTES_V1, DIRECT_AOT_REQUEST_BYTES_V1, DIRECT_AOT_SCALARS_V1,
};
use dclutch_execution_strategy_contract::{
    AcceleratorAckV1, AcceleratorRequestV1, ExecutionDispositionV1, encode_register_bank_into,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use solana_program::{instruction::AccountMeta, instruction::Instruction, pubkey::Pubkey};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{clock::Clock, signature::Signer};
use solana_transaction::Transaction;

/// The gauntlet's protocol compute ceiling. Not adjustable: `TIERS.md` states
/// that a diagnostic budget is a measurement and a measurement never satisfies
/// a gate, so this producer sets the canonical limit and nothing else.
const COMPUTE_LIMIT: u64 = 1_400_000;
/// The gauntlet's protocol heap extent, stated for the record. ProgramTest
/// gives every invocation the runtime default 32 KiB BPF heap; this producer
/// never requests a different one.
const HEAP_BYTES: u64 = 32_768;
/// Solana's legacy packet maximum. The fast lane does not rely on the runtime
/// to enforce it — ProgramTest submits no packet — so this producer serialises
/// each transaction itself and records the extent for a witness to check.
const PACKET_DATA_BYTES: usize = 1_232;

/// The census role label this campaign drives.
const ROLE: &str = "direct-aot";
/// The ELF stem `SBF_OUT_DIR` is expected to carry.
const PROGRAM_STEM: &str = "dclutch_direct_aot_sbf";

/// Byte offset of the eight-byte request magic. Source: the shared
/// `require_common_header` contract in
/// `crates/dclutch-execution-strategy-contract/src/lib.rs` — magic occupies
/// bytes 0..8 of every V1 execution-strategy wire.
const REQUEST_MAGIC_OFFSET: usize = 0;
/// Byte offset of the twelve reserved request-header bytes.
/// Source: `REQUEST_RESERVED_OFFSET` /`REQUEST_RESERVED_BYTES`,
/// `crates/dclutch-execution-strategy-contract/src/lib.rs:96-97`.
const REQUEST_RESERVED_OFFSET: usize = 116;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("dclutch-gauntlet-direct-campaign: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let options = parse_options()?;
    let out = options
        .get("out")
        .cloned()
        .ok_or("--out DIR is required (the campaign writes evidence.json and programs.json there)")?;
    let sbf_out_dir = env::var("SBF_OUT_DIR")
        .map_err(|_| "SBF_OUT_DIR must name the directory holding the built Direct ELFs")?;
    let elf_path = PathBuf::from(&sbf_out_dir).join(format!("{PROGRAM_STEM}.so"));
    let elf = fs::read(&elf_path)
        .map_err(|error| format!("read {}: {error}", elf_path.display()))?;
    let elf_sha256 = hex(&Sha256::digest(&elf));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("tokio runtime: {error}"))?;
    let campaign = runtime.block_on(campaign())?;

    let program_id = gauntlet_program_id(ROLE);
    let evidence = json!({
        "schema": "dclutch-gauntlet-direct-campaign-evidence-v1",
        "campaign": "direct-aot-programtest",
        "producer": "tools/gauntlet/direct/producer",
        "artifact": {
            "role": ROLE,
            "elf_path": elf_path.to_string_lossy(),
            "elf_sha256": elf_sha256,
            "program_id": program_id.to_string(),
        },
        "fast_lane": fast_lane_statement(),
        "transactions": campaign,
    });

    let programs = json!({ ROLE: program_id.to_string() });
    write_atomic(
        &format!("{out}/evidence.json"),
        &to_pretty_line(&evidence)?,
    )?;
    write_atomic(
        &format!("{out}/programs.json"),
        &to_pretty_line(&programs)?,
    )?;
    eprintln!(
        "direct-aot-programtest: {} transactions, program {program_id}, ELF {elf_sha256}",
        campaign.len()
    );
    Ok(())
}

/// The four `TIERS.md` fast-lane conditions, each answered for THIS tier.
///
/// `TIERS.md` requires a ProgramTest-backed tier to state which conditions it
/// meets. A block of prose in a document nobody reads next to the numbers is
/// how a fast lane launders itself into a gate, so the statement rides in the
/// evidence document alongside the transactions it qualifies.
fn fast_lane_statement() -> Value {
    json!({
        "backing": "solana-program-test",
        "finality": "ProgramTest has no finalized commitment. `slot` is the bank slot observed at submission; it orders the campaign and is NOT proof of finality. A validator tier is still owed for that.",
        "no_loader_v3_dependency": "The stateless accelerator authenticates no account at all: the route refuses any frame that carries one. It cannot depend on genesis Loader-v3 ProgramData layout, on SetAuthority(Some -> None), or on deployment slots, because it never reads a program account.",
        "no_packet_limit_dependency": "The route's whole instruction is 584 request bytes with an empty account list. This producer serialises every transaction and records `wire_bytes`; the tier does not ask the runtime to enforce the 1,232-byte packet maximum, it measures against it.",
        "compute_and_heap": "COMPUTE_LIMIT is set to the canonical 1,400,000 and never adjusted; the BPF heap is the runtime default 32,768. No diagnostic budget is used anywhere in this campaign.",
        "real_account_shapes": "Vacuously satisfied and deliberately so: the campaign presents zero accounts on the admitted route, and the one hostile case that presents an account presents a real ProgramTest System-owned payer account, not a fabricated shape.",
        "compute_limit": COMPUTE_LIMIT,
        "heap_bytes": HEAP_BYTES,
        "packet_data_bytes": PACKET_DATA_BYTES,
    })
}

/// One labelled campaign transaction, in the gauntlet's evidence shape.
struct Submission {
    label: &'static str,
    signature: String,
    slot: u64,
    error: Option<String>,
    logs: Vec<String>,
    compute_units: u64,
    wire_bytes: usize,
    /// Decoded accelerator acknowledgement facts, when the route committed.
    ack: Option<Value>,
}

impl Submission {
    fn into_json(self) -> Value {
        json!({
            "label": self.label,
            "signature": self.signature,
            "slot": self.slot,
            "error": self.error.map_or(Value::Null, Value::String),
            "logs": self.logs,
            "compute_units_consumed": self.compute_units,
            "wire_bytes": self.wire_bytes,
            "ack": self.ack.unwrap_or(Value::Null),
        })
    }
}

async fn campaign() -> Result<Vec<Value>, String> {
    let program_id = gauntlet_program_id(ROLE);
    let mut test = ProgramTest::new(PROGRAM_STEM, program_id, None);
    test.prefer_bpf(true);
    test.set_compute_max_units(COMPUTE_LIMIT);
    let mut context = test.start_with_context().await;

    let mut recorded = Vec::new();

    // ---- admitted frames -------------------------------------------------
    // Every one of these must COMMIT and publish an Accepted acknowledgement.
    // The set is deliberately boundary-heavy: an admission relation is only as
    // good as its edges, and a campaign that only ever runs the middle of the
    // range proves the middle of the range.
    for (label, mutate) in admitted_frames() {
        let (scalars, identities) = mutated(mutate);
        recorded.push(
            admit(
                &mut context,
                program_id,
                label,
                &encode_request(&scalars, &identities)?,
                ExecutionDispositionV1::Accepted,
            )
            .await?,
        );
    }

    // ---- semantic refusals that COMMIT ----------------------------------
    // The honest awkward cases. The accelerator is stateless: a semantic
    // refusal is a canonical 160-byte Refused acknowledgement and the
    // transaction SUCCEEDS. The census therefore records these as EXECUTED,
    // not as refusals, and the binding note says so. Crediting them to a
    // refusal code would name a code the program never raised.
    for (label, mutate) in semantic_refusals() {
        let (scalars, identities) = mutated(mutate);
        recorded.push(
            admit(
                &mut context,
                program_id,
                label,
                &encode_request(&scalars, &identities)?,
                ExecutionDispositionV1::Refused,
            )
            .await?,
        );
    }

    // ---- physical refusals ----------------------------------------------
    // NonStatelessFrame: the route's first act is to refuse any account.
    let payer = context.payer.pubkey();
    recorded.push(
        refuse(
            &mut context,
            Instruction {
                program_id,
                accounts: vec![AccountMeta::new_readonly(payer, false)],
                data: canonical_request().to_vec(),
            },
            "Direct AOT frame refuses an account-bearing invocation",
        )
        .await?,
    );

    for (label, data) in physical_refusals()? {
        recorded.push(
            refuse(
                &mut context,
                Instruction {
                    program_id,
                    accounts: Vec::new(),
                    data,
                },
                label,
            )
            .await?,
        );
    }

    Ok(recorded.into_iter().map(Submission::into_json).collect())
}

/// One case: a mutation applied to the canonical admitted frame.
type Mutation = fn(&mut [u64; DIRECT_AOT_SCALARS_V1], &mut [[u8; 32]; DIRECT_AOT_IDENTITIES_V1]);

fn mutated(
    mutate: Mutation,
) -> (
    [u64; DIRECT_AOT_SCALARS_V1],
    [[u8; 32]; DIRECT_AOT_IDENTITIES_V1],
) {
    let mut scalars = canonical_scalars();
    let mut identities = canonical_identities();
    mutate(&mut scalars, &mut identities);
    (scalars, identities)
}

fn put(scalars: &mut [u64; DIRECT_AOT_SCALARS_V1], index: usize, value: u64) {
    if let Some(slot) = scalars.get_mut(index) {
        *slot = value;
    }
}

fn put_identity(
    identities: &mut [[u8; 32]; DIRECT_AOT_IDENTITIES_V1],
    index: usize,
    value: [u8; 32],
) {
    if let Some(slot) = identities.get_mut(index) {
        *slot = value;
    }
}

/// Admitted frames: the canonical fill and the exact edges around it.
///
/// The canonical frame is fill-or-kill (lifecycle 0) at fill 2,000, price
/// 500,000 of scale 1,000,000 and a 25 bps venue rate: gross 1,000, floor fee
/// 2. Every named boundary below is arithmetic on those numbers, and the
/// witness file re-derives gross and fee independently of this program.
fn admitted_frames() -> Vec<(&'static str, Mutation)> {
    vec![
        (
            "Direct AOT admits the canonical fill-or-kill fill",
            |_scalars, _identities| {},
        ),
        (
            "Direct AOT admits a partial fill under a resting lifecycle",
            |scalars, _identities| {
                put(scalars, SCALAR_SELLER_LIFECYCLE, 1);
                put(scalars, SCALAR_BUYER_LIFECYCLE, 1);
                put(scalars, SCALAR_FILL, 1_000);
            },
        ),
        (
            "Direct AOT admits the minimal fill whose floor fee rounds to zero",
            |scalars, _identities| {
                // gross 1, fee floor(1 * 25 / 10_000) = 0. A zero fee is an
                // admitted quote, not a refusal; the boundary belongs in the
                // campaign rather than in a comment.
                put(scalars, SCALAR_SELLER_LIFECYCLE, 2);
                put(scalars, SCALAR_BUYER_LIFECYCLE, 2);
                put(scalars, SCALAR_FILL, 2);
            },
        ),
        (
            "Direct AOT admits a price exactly at the signed seller limit",
            |scalars, _identities| put(scalars, SCALAR_EXECUTION_PRICE, 400_000),
        ),
        (
            "Direct AOT admits a price exactly at the signed buyer limit",
            |scalars, _identities| put(scalars, SCALAR_EXECUTION_PRICE, 600_000),
        ),
        (
            "Direct AOT admits a buyer holding exactly gross plus fee",
            |scalars, _identities| put(scalars, SCALAR_BUYER_COLLATERAL, 1_002),
        ),
        (
            "Direct AOT admits a seller holding exactly the filled claims",
            |scalars, _identities| put(scalars, SCALAR_SELLER_CLAIMS, 2_000),
        ),
        (
            "Direct AOT admits a slot exactly at the tightest window open",
            |scalars, _identities| put(scalars, SCALAR_SLOT, 95),
        ),
        (
            "Direct AOT admits a slot exactly at the tightest window close",
            |scalars, _identities| put(scalars, SCALAR_SLOT, 110),
        ),
        (
            "Direct AOT admits a fee rate exactly at the denominator",
            |scalars, _identities| {
                // The whole gross becomes fee; the buyer must cover both.
                put(scalars, SCALAR_SELLER_FEE_BPS, FEE_DENOMINATOR_V2);
                put(scalars, SCALAR_BUYER_FEE_BPS, FEE_DENOMINATOR_V2);
                put(scalars, SCALAR_POLICY_FEE_BPS, FEE_DENOMINATOR_V2);
            },
        ),
        (
            "Direct AOT admits an execution price exactly at the price scale",
            |scalars, _identities| {
                // gross 2,000, fee 5; the buyer needs 2,005 of collateral.
                put(scalars, SCALAR_EXECUTION_PRICE, 1_000_000);
                put(scalars, SCALAR_BUYER_LIMIT, 1_000_000);
                put(scalars, SCALAR_BUYER_COLLATERAL, 2_005);
            },
        ),
        (
            "Direct AOT admits the last outcome coordinate in the Market",
            |scalars, _identities| {
                put(scalars, SCALAR_OUTCOME_COUNT, 3);
                put(scalars, SCALAR_SELLER_OUTCOME, 2);
                put(scalars, SCALAR_BUYER_OUTCOME, 2);
            },
        ),
    ]
}

/// Semantic refusals: exact-shaped frames the Direct admission relation must
/// reject. Each names the clause it violates, and each is the near twin of an
/// admitted frame above so that the campaign pins the boundary rather than a
/// distant point on the wrong side of it.
fn semantic_refusals() -> Vec<(&'static str, Mutation)> {
    vec![
        ("Direct AOT admission refuses a zero fill", |scalars, _identities| {
            put(scalars, SCALAR_FILL, 0)
        }),
        (
            "Direct AOT admission refuses a partial fill against a fill-or-kill intent",
            |scalars, _identities| put(scalars, SCALAR_FILL, 1_999),
        ),
        (
            "Direct AOT admission refuses a fill above the signed maximum",
            |scalars, _identities| {
                put(scalars, SCALAR_SELLER_LIFECYCLE, 1);
                put(scalars, SCALAR_BUYER_LIFECYCLE, 1);
                put(scalars, SCALAR_FILL, 2_001);
            },
        ),
        (
            "Direct AOT admission refuses a lifecycle that is neither FOK, IOC, nor GTC",
            |scalars, _identities| put(scalars, SCALAR_SELLER_LIFECYCLE, 3),
        ),
        (
            "Direct AOT admission refuses a quote whose exact division has a remainder",
            |scalars, _identities| {
                // 1 * 500,000 / 1,000,000 has a remainder. The Direct quote is
                // exact-integer by construction; there is no rounding boundary
                // here to argue about.
                put(scalars, SCALAR_SELLER_LIFECYCLE, 1);
                put(scalars, SCALAR_BUYER_LIFECYCLE, 1);
                put(scalars, SCALAR_FILL, 1);
            },
        ),
        (
            "Direct AOT admission refuses a matcher price one below the seller limit",
            |scalars, _identities| put(scalars, SCALAR_EXECUTION_PRICE, 399_999),
        ),
        (
            "Direct AOT admission refuses a matcher price one above the buyer limit",
            |scalars, _identities| put(scalars, SCALAR_EXECUTION_PRICE, 600_001),
        ),
        (
            "Direct AOT admission refuses an execution price above the price scale",
            |scalars, _identities| {
                put(scalars, SCALAR_EXECUTION_PRICE, 1_000_001);
                put(scalars, SCALAR_BUYER_LIMIT, 2_000_000);
            },
        ),
        (
            "Direct AOT admission refuses a Market that is not open",
            |scalars, _identities| put(scalars, SCALAR_PHASE, OPEN_PHASE_V2 + 1),
        ),
        (
            "Direct AOT admission refuses a slot one before the tightest window open",
            |scalars, _identities| put(scalars, SCALAR_SLOT, 94),
        ),
        (
            "Direct AOT admission refuses a slot one after the tightest window close",
            |scalars, _identities| put(scalars, SCALAR_SLOT, 111),
        ),
        (
            "Direct AOT admission refuses a maker generation skew",
            |scalars, _identities| put(scalars, SCALAR_BUYER_GENERATION, 4),
        ),
        (
            "Direct AOT admission refuses a selected-outcome mismatch",
            |scalars, _identities| put(scalars, SCALAR_BUYER_OUTCOME, 0),
        ),
        (
            "Direct AOT admission refuses an outcome coordinate outside the Market",
            |scalars, _identities| {
                put(scalars, SCALAR_SELLER_OUTCOME, 2);
                put(scalars, SCALAR_BUYER_OUTCOME, 2);
            },
        ),
        (
            "Direct AOT admission refuses two makers on the same side",
            |scalars, _identities| put(scalars, SCALAR_BUYER_SIDE, SELL_SIDE_V2),
        ),
        (
            "Direct AOT admission refuses a seller presented as the buy side",
            |scalars, _identities| put(scalars, SCALAR_SELLER_SIDE, BUY_SIDE_V2),
        ),
        (
            "Direct AOT admission refuses one maker filling against itself",
            |_scalars, identities| put_identity(identities, IDENTITY_BUYER_MAKER, [11; 32]),
        ),
        (
            "Direct AOT admission refuses two intents signed for different Markets",
            |_scalars, identities| put_identity(identities, IDENTITY_BUYER_MARKET, [102; 32]),
        ),
        (
            "Direct AOT admission refuses a seller one claim short of the fill",
            |scalars, _identities| put(scalars, SCALAR_SELLER_CLAIMS, 1_999),
        ),
        (
            "Direct AOT admission refuses a buyer one unit short of gross plus fee",
            |scalars, _identities| put(scalars, SCALAR_BUYER_COLLATERAL, 1_001),
        ),
        (
            "Direct AOT admission refuses a seller collateral balance that would overflow",
            |scalars, _identities| put(scalars, SCALAR_SELLER_COLLATERAL, u64::MAX),
        ),
        (
            "Direct AOT admission refuses a venue collateral balance that would overflow",
            |scalars, _identities| put(scalars, SCALAR_VENUE_COLLATERAL, u64::MAX),
        ),
        (
            "Direct AOT admission refuses a buyer claim balance that would overflow",
            |scalars, _identities| put(scalars, SCALAR_BUYER_CLAIMS, u64::MAX),
        ),
        (
            "Direct AOT admission refuses a zero price scale",
            |scalars, _identities| put(scalars, SCALAR_PRICE_SCALE, 0),
        ),
        (
            "Direct AOT admission refuses a venue fee rate the makers did not sign",
            |scalars, _identities| put(scalars, SCALAR_POLICY_FEE_BPS, 26),
        ),
        (
            "Direct AOT admission refuses a seller fee rate the policy did not set",
            |scalars, _identities| put(scalars, SCALAR_SELLER_FEE_BPS, 24),
        ),
        (
            "Direct AOT admission refuses a fee rate above the denominator",
            |scalars, _identities| {
                put(scalars, SCALAR_SELLER_FEE_BPS, FEE_DENOMINATOR_V2 + 1);
                put(scalars, SCALAR_BUYER_FEE_BPS, FEE_DENOMINATOR_V2 + 1);
                put(scalars, SCALAR_POLICY_FEE_BPS, FEE_DENOMINATOR_V2 + 1);
            },
        ),
        (
            "Direct AOT admission refuses a replay nonce that is not the maker successor",
            |scalars, _identities| put(scalars, SCALAR_SELLER_NEXT_NONCE, 1),
        ),
        (
            "Direct AOT admission refuses a saturated maker replay nonce",
            |scalars, _identities| {
                put(scalars, SCALAR_BUYER_NONCE, u64::MAX);
                put(scalars, SCALAR_BUYER_NEXT_NONCE, u64::MAX);
            },
        ),
    ]
}

/// Physical refusals: wires the adapter must reject before any admission.
///
/// Every one of these is a `DirectAotSbfError::InvalidRequest`. They are kept
/// distinct rather than collapsed to one case because they exercise different
/// decoder clauses — width, magic, reserved span, and the header's own runtime
/// bank counts — and a decoder that lost one of those clauses would still pass
/// a single-case test.
fn physical_refusals() -> Result<Vec<(&'static str, Vec<u8>)>, String> {
    let canonical = canonical_request();
    let mut cases: Vec<(&'static str, Vec<u8>)> = Vec::new();

    cases.push(("Direct AOT wire refuses an empty instruction", Vec::new()));

    let truncated = canonical
        .get(..DIRECT_AOT_REQUEST_BYTES_V1 - 1)
        .ok_or("truncated request slice")?
        .to_vec();
    cases.push(("Direct AOT wire refuses a truncated request", truncated));

    let mut overlong = canonical.to_vec();
    overlong.push(0);
    cases.push(("Direct AOT wire refuses an over-long request", overlong));

    let mut foreign_magic = canonical.to_vec();
    foreign_magic
        .get_mut(REQUEST_MAGIC_OFFSET..REQUEST_MAGIC_OFFSET + 8)
        .ok_or("request magic span")?
        .copy_from_slice(b"DCLTAIR2");
    cases.push(("Direct AOT wire refuses a foreign request magic", foreign_magic));

    let mut dirty_reserved = canonical.to_vec();
    *dirty_reserved
        .get_mut(REQUEST_RESERVED_OFFSET)
        .ok_or("request reserved span")? = 1;
    cases.push((
        "Direct AOT wire refuses a nonzero reserved request span",
        dirty_reserved,
    ));

    // Header counts that are internally consistent with a SHORTER bank: the
    // wire decodes cleanly as an accelerator request and is refused for being
    // the wrong SHAPE for Direct V2, not for being malformed. This is the case
    // a width check that trusted the header alone would let through.
    cases.push((
        "Direct AOT wire refuses a request declaring a foreign scalar width",
        foreign_width_request(DIRECT_AOT_SCALARS_V1 - 1, DIRECT_AOT_IDENTITIES_V1)?,
    ));
    cases.push((
        "Direct AOT wire refuses a request declaring a foreign identity width",
        foreign_width_request(DIRECT_AOT_SCALARS_V1, DIRECT_AOT_IDENTITIES_V1 - 1)?,
    ));

    Ok(cases)
}

fn foreign_width_request(scalars: usize, identities: usize) -> Result<Vec<u8>, String> {
    let scalar_count = u16::try_from(scalars).map_err(|_| "scalar count width")?;
    let identity_count = u16::try_from(identities).map_err(|_| "identity count width")?;
    let bank_bytes = scalars
        .checked_mul(8)
        .and_then(|value| value.checked_add(identities.checked_mul(32)?))
        .ok_or("foreign bank width")?;
    let mut bank = vec![0_u8; bank_bytes];
    encode_register_bank_into(
        &vec![0_u64; scalars],
        &vec![[7_u8; 32]; identities],
        &mut bank,
    )
    .map_err(|error| format!("foreign bank encode: {error:?}"))?;
    let request = AcceleratorRequestV1::new(
        content(1),
        content(2),
        content(3),
        scalar_count,
        identity_count,
        &bank,
    )
    .map_err(|error| format!("foreign request: {error:?}"))?;
    let mut bytes = vec![0_u8; 128 + bank_bytes];
    request
        .encode_into(&mut bytes)
        .map_err(|error| format!("foreign request encode: {error:?}"))?;
    Ok(bytes)
}

/// Submit an instruction that MUST commit, and check the acknowledgement.
async fn admit(
    context: &mut ProgramTestContext,
    program_id: Pubkey,
    label: &'static str,
    data: &[u8; DIRECT_AOT_REQUEST_BYTES_V1],
    expected: ExecutionDispositionV1,
) -> Result<Submission, String> {
    let instruction = Instruction {
        program_id,
        accounts: Vec::new(),
        data: data.to_vec(),
    };
    let (mut submission, return_data) = submit(context, instruction, label).await?;
    if let Some(error) = &submission.error {
        return Err(format!("`{label}` must commit; the chain refused: {error}"));
    }
    let returned = return_data.ok_or_else(|| format!("`{label}` published no return data"))?;
    let ack = AcceleratorAckV1::decode(&returned)
        .map_err(|error| format!("`{label}` acknowledgement did not decode: {error:?}"))?;
    if ack.disposition() != expected {
        return Err(format!(
            "`{label}` expected {expected:?} but the accelerator published {:?}",
            ack.disposition()
        ));
    }

    // The adapter hashes the exact instruction bytes it was handed. Recompute
    // that digest here with a DIFFERENT SHA-256 implementation (RustCrypto's,
    // against the runtime's syscall) and require the two to agree: an ack that
    // bound the wrong request would otherwise be indistinguishable from one
    // that bound the right one.
    let expected_digest: [u8; 32] = Sha256::digest(data.as_slice()).into();
    if ack.request_digest().as_bytes() != &expected_digest {
        return Err(format!(
            "`{label}` bound request digest {} but the submitted bytes hash to {}",
            hex(ack.request_digest().as_bytes()),
            hex(&expected_digest)
        ));
    }

    let facts = match expected {
        ExecutionDispositionV1::Accepted => {
            if returned.len() != DIRECT_AOT_ACCEPTED_ACK_BYTES_V1 {
                return Err(format!(
                    "`{label}` accepted acknowledgement is {} bytes, not {DIRECT_AOT_ACCEPTED_ACK_BYTES_V1}",
                    returned.len()
                ));
            }
            let scalars = decode_output_scalars(ack.bank())?;
            json!({
                "disposition": "accepted",
                "ack_bytes": returned.len(),
                "request_digest": hex(ack.request_digest().as_bytes()),
                "gross": scalars.get(SCALAR_GROSS_OUTPUT).copied(),
                "fee": scalars.get(SCALAR_FEE_OUTPUT).copied(),
                "fill": scalars.get(SCALAR_FILL).copied(),
                "execution_price": scalars.get(SCALAR_EXECUTION_PRICE).copied(),
                "price_scale": scalars.get(SCALAR_PRICE_SCALE).copied(),
                "policy_fee_bps": scalars.get(SCALAR_POLICY_FEE_BPS).copied(),
            })
        }
        ExecutionDispositionV1::Refused => {
            if returned.len() != DIRECT_AOT_REFUSED_ACK_BYTES_V1 {
                return Err(format!(
                    "`{label}` refusal acknowledgement is {} bytes, not {DIRECT_AOT_REFUSED_ACK_BYTES_V1}",
                    returned.len()
                ));
            }
            json!({
                "disposition": "refused",
                "ack_bytes": returned.len(),
                "request_digest": hex(ack.request_digest().as_bytes()),
            })
        }
    };
    submission.ack = Some(facts);
    Ok(submission)
}

/// Submit an instruction that MUST be refused by the program's own taxonomy.
async fn refuse(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    label: &'static str,
) -> Result<Submission, String> {
    let (submission, _) = submit(context, instruction, label).await?;
    if submission.error.is_none() {
        return Err(format!("`{label}` committed; it must be refused"));
    }
    Ok(submission)
}

async fn submit(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    label: &'static str,
) -> Result<(Submission, Option<Vec<u8>>), String> {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .map_err(|error| format!("`{label}` blockhash: {error}"))?;
    let payer_key = context.payer.pubkey();
    let transaction = Transaction::new_signed_with_payer(
        std::slice::from_ref(&instruction),
        Some(&payer_key),
        &[&context.payer],
        blockhash,
    );
    let signature = transaction
        .signatures
        .first()
        .ok_or_else(|| format!("`{label}` produced no signature"))?
        .to_string();
    let wire_bytes = 1_usize
        .checked_add(transaction.signatures.len().saturating_mul(64))
        .and_then(|prefix| prefix.checked_add(transaction.message_data().len()))
        .ok_or_else(|| format!("`{label}` wire extent overflowed"))?;
    if wire_bytes > PACKET_DATA_BYTES {
        return Err(format!(
            "`{label}` serialises to {wire_bytes} bytes, past Solana's {PACKET_DATA_BYTES}-byte packet maximum"
        ));
    }
    let slot = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .map_err(|error| format!("`{label}` Clock sysvar: {error}"))?
        .slot;
    let processed = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .map_err(|error| format!("`{label}` Banks RPC: {error}"))?;
    let error = processed
        .result
        .as_ref()
        .err()
        .map(|failure| format!("{failure:?}"));
    let metadata = processed
        .metadata
        .ok_or_else(|| format!("`{label}` produced no transaction metadata"))?;
    let return_data = metadata.return_data.and_then(|returned| {
        (returned.program_id == instruction.program_id).then_some(returned.data)
    });
    Ok((
        Submission {
            label,
            signature,
            slot,
            error,
            logs: metadata.log_messages,
            compute_units: metadata.compute_units_consumed,
            wire_bytes,
            ack: None,
        },
        return_data,
    ))
}

fn decode_output_scalars(bank: &[u8]) -> Result<Vec<u64>, String> {
    if bank.len() != DIRECT_AOT_BANK_BYTES_V1 {
        return Err(format!(
            "accepted output bank is {} bytes, not {DIRECT_AOT_BANK_BYTES_V1}",
            bank.len()
        ));
    }
    let scalar_bytes = DIRECT_AOT_SCALARS_V1
        .checked_mul(8)
        .ok_or("scalar span overflow")?;
    let mut output = Vec::with_capacity(DIRECT_AOT_SCALARS_V1);
    for chunk in bank
        .get(..scalar_bytes)
        .ok_or("scalar span")?
        .chunks_exact(8)
    {
        let word: [u8; 8] = chunk.try_into().map_err(|_| "scalar word width")?;
        output.push(u64::from_le_bytes(word));
    }
    Ok(output)
}

/// The canonical admitted Direct V2 frame.
///
/// Fill-or-kill on both sides at the full signed quantity of 2,000, an
/// execution price of 500,000 against a scale of 1,000,000, and a 25 bps venue
/// rate that all three of policy, seller and buyer signed. Gross is therefore
/// exactly 1,000 and the floor fee exactly 2.
fn canonical_scalars() -> [u64; DIRECT_AOT_SCALARS_V1] {
    let mut scalars = [0_u64; DIRECT_AOT_SCALARS_V1];
    for (index, value) in [
        (SCALAR_PHASE, OPEN_PHASE_V2),
        (SCALAR_SLOT, 100),
        (SCALAR_SELLER_FROM, 90),
        (SCALAR_SELLER_THROUGH, 110),
        (SCALAR_BUYER_FROM, 95),
        (SCALAR_BUYER_THROUGH, 120),
        (SCALAR_SELLER_SIDE, SELL_SIDE_V2),
        (SCALAR_BUYER_SIDE, BUY_SIDE_V2),
        (SCALAR_SELLER_GENERATION, 3),
        (SCALAR_BUYER_GENERATION, 3),
        (SCALAR_SELLER_OUTCOME, 1),
        (SCALAR_BUYER_OUTCOME, 1),
        (SCALAR_OUTCOME_COUNT, 2),
        (SCALAR_SELLER_LIFECYCLE, 0),
        (SCALAR_SELLER_MAXIMUM, 2_000),
        (SCALAR_BUYER_LIFECYCLE, 0),
        (SCALAR_BUYER_MAXIMUM, 2_000),
        (SCALAR_SELLER_LIMIT, 400_000),
        (SCALAR_EXECUTION_PRICE, 500_000),
        (SCALAR_BUYER_LIMIT, 600_000),
        (SCALAR_PRICE_SCALE, 1_000_000),
        (SCALAR_SELLER_FEE_BPS, 25),
        (SCALAR_BUYER_FEE_BPS, 25),
        (SCALAR_POLICY_FEE_BPS, 25),
        (SCALAR_FILL, 2_000),
        (SCALAR_SELLER_CLAIMS, 5_000),
        (SCALAR_BUYER_CLAIMS, 200),
        (SCALAR_BUYER_COLLATERAL, 2_000),
        (SCALAR_SELLER_COLLATERAL, 100),
        (SCALAR_VENUE_COLLATERAL, 20),
    ] {
        put(&mut scalars, index, value);
    }
    scalars
}

/// The canonical identity bank: one Market signed by both intents, two
/// distinct makers.
fn canonical_identities() -> [[u8; 32]; DIRECT_AOT_IDENTITIES_V1] {
    let mut identities = [[0_u8; 32]; DIRECT_AOT_IDENTITIES_V1];
    put_identity(&mut identities, IDENTITY_SELLER_MARKET, [101; 32]);
    put_identity(&mut identities, IDENTITY_BUYER_MARKET, [101; 32]);
    put_identity(&mut identities, IDENTITY_SELLER_MAKER, [11; 32]);
    put_identity(&mut identities, IDENTITY_BUYER_MAKER, [12; 32]);
    identities
}

fn canonical_request() -> [u8; DIRECT_AOT_REQUEST_BYTES_V1] {
    encode_request(&canonical_scalars(), &canonical_identities()).unwrap_or_else(|error| {
        panic!("the canonical Direct request must encode: {error}");
    })
}

fn encode_request(
    scalars: &[u64; DIRECT_AOT_SCALARS_V1],
    identities: &[[u8; 32]; DIRECT_AOT_IDENTITIES_V1],
) -> Result<[u8; DIRECT_AOT_REQUEST_BYTES_V1], String> {
    let mut bank = [0_u8; DIRECT_AOT_BANK_BYTES_V1];
    encode_register_bank_into(scalars, identities, &mut bank)
        .map_err(|error| format!("register bank: {error:?}"))?;
    let request = AcceleratorRequestV1::new(
        content(1),
        content(2),
        content(3),
        u16::try_from(DIRECT_AOT_SCALARS_V1).map_err(|_| "scalar width")?,
        u16::try_from(DIRECT_AOT_IDENTITIES_V1).map_err(|_| "identity width")?,
        &bank,
    )
    .map_err(|error| format!("request: {error:?}"))?;
    let mut bytes = [0_u8; DIRECT_AOT_REQUEST_BYTES_V1];
    request
        .encode_into(&mut bytes)
        .map_err(|error| format!("request encode: {error:?}"))?;
    Ok(bytes)
}

fn content(byte: u8) -> ContentId {
    ContentId::new([byte; 32]).unwrap_or_else(|_| {
        panic!("a nonzero fixture content identity must construct");
    })
}

/// The gauntlet's offline program address for a role.
///
/// Byte-identical to `program_id_for` in `tools/gauntlet/run.sh`: SHA-256 over
/// a fixed domain and the role name. No private key exists for it, and it
/// names no deployed program anywhere.
fn gauntlet_program_id(role: &str) -> Pubkey {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/gauntlet/program-id/v1\nrole=");
    hasher.update(role.as_bytes());
    Pubkey::new_from_array(hasher.finalize().into())
}

fn parse_options() -> Result<BTreeMap<String, String>, String> {
    let mut options = BTreeMap::new();
    let arguments: Vec<String> = env::args().skip(1).collect();
    let mut iterator = arguments.iter();
    while let Some(argument) = iterator.next() {
        let name = argument
            .strip_prefix("--")
            .ok_or_else(|| format!("unexpected positional argument: {argument}"))?;
        let value = iterator
            .next()
            .ok_or_else(|| format!("--{name} requires a value"))?;
        if options.insert(name.to_string(), value.clone()).is_some() {
            return Err(format!("--{name} may be supplied only once"));
        }
    }
    Ok(options)
}

fn to_pretty_line(value: &Value) -> Result<Vec<u8>, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| format!("encode: {error}"))?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Write through a same-directory temporary file, per `AGENTS.md`: a failed
/// producer leaves the last accepted output byte-for-byte intact.
fn write_atomic(path: &str, bytes: &[u8]) -> Result<(), String> {
    let target = std::path::Path::new(path);
    let directory = target.parent().unwrap_or_else(|| std::path::Path::new("."));
    fs::create_dir_all(directory)
        .map_err(|error| format!("create {}: {error}", directory.display()))?;
    let temporary = directory.join(format!(
        ".{}.campaign-tmp",
        target
            .file_name()
            .map_or_else(|| "out".into(), |name| name.to_string_lossy().into_owned())
    ));
    fs::write(&temporary, bytes)
        .map_err(|error| format!("write {}: {error}", temporary.display()))?;
    fs::rename(&temporary, target).map_err(|error| format!("replace {path}: {error}"))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut held, byte| {
        held.push_str(&format!("{byte:02x}"));
        held
    })
}
