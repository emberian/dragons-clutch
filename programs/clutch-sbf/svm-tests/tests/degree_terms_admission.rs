//! Real-SBF bank evidence that the smooth rungs are admitted, and that every
//! side condition of the Lean constructions they denote is enforced by the
//! deployed program rather than only by the host codec.
//!
//! `blank_bank_joined_lifecycle_degree_two` / `_degree_three` already run a
//! whole create/split/resolve/redeem walk on degree-2 and degree-3 markets, so
//! the *positive* claim is covered. What was missing is the other half: proof
//! that a malformed smooth Terms artifact cannot be sealed into the bank at
//! all. A market whose knot vector lies about its spacing, or whose anchor
//! count does not match its degree, has no B-spline behind it — the Lean
//! linkage theorems are all stated over `uniformStoredKnots origin gap count`
//! with `0 < gap` and `2 ≤ count`, and above degree one there is no nonuniform
//! counterpart at all. Each refusal below therefore names the hypothesis it is
//! protecting.
//!
//! The refusals are asserted at `SealArtifact`, which is the last moment
//! before the bytes become a permanent, content-addressed account other
//! instructions will trust. Every case also asserts that a prefunded final
//! account is rolled back byte-exactly, so a rejected smooth Terms leaves no
//! squatting residue at its canonical address.

use {
    clutch_sbf::seeds,
    clutch_solana_layout::{
        account_len,
        artifact::{ArtifactKind, ARTIFACT_CHUNK_BYTES},
        CodecError, Hash32, Intent, PayoutVectorBytes, TermsAccount, MAX_KNOTS, MAX_OUTCOMES,
        MAX_PAYOUTS, PAYOUT_MAP_UNUSED, UNIFORM_SPACING_NONE,
    },
    clutch_svm_fixture::{fixture_terms, layout_request, PROGRAM_ID, RENT_SYSVAR, SYSTEM_PROGRAM},
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_test::{tokio, ProgramTest, ProgramTestContext},
    solana_signer::Signer,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

const CLOCK_SYSVAR: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);
const UPLOADER_LAMPORTS: u64 = 2_000_000_000;

/// Five claims, `D = 8`, and a `2^3`-spaced anchor grid: the shape the
/// per-degree joined lifecycle founds its smooth markets on.
const CLAIMS: u8 = 5;
const DENOMINATOR: u64 = 8;
const SPACING: u8 = 3;

/* ---- encoded field offsets ---------------------------------------------
 *
 * `TermsAccount::encode` validates before it writes, so it cannot produce a
 * malformed body. These offsets let the test patch a valid encoding into the
 * exact bytes the malformed struct would have produced; every case reasserts
 * that with `TermsAccount::decode` before a transaction is sent, so a wrong
 * offset fails here rather than silently weakening the bank assertion. */
const OFFSET_BASIS_DEGREE: usize = 2
    + 32
    + (4 * 32)
    + 1
    + 1
    + (MAX_PAYOUTS * (8 + MAX_OUTCOMES * 8))
    + 4
    + 2
    + (8 * 4)
    + (4 * 3)
    + 2
    + 1
    + 1;
const OFFSET_KNOT_COUNT: usize = OFFSET_BASIS_DEGREE + 1;
const OFFSET_SPACING: usize = OFFSET_BASIS_DEGREE + 2;
const OFFSET_PAYOUT_MAP: usize = OFFSET_BASIS_DEGREE + 5 + 8 + 8 + 4 + 4 + 32;
const OFFSET_KNOTS: usize = OFFSET_PAYOUT_MAP + MAX_OUTCOMES;
const _: () = assert!(OFFSET_KNOTS + (MAX_KNOTS * 16) + 8 + 7 + 1 + 1 == account_len::TERMS);

fn uploader() -> Keypair {
    Keypair::new_from_array([
        0x3d, 0x08, 0xb1, 0x57, 0x24, 0xe6, 0x9a, 0x0c, 0x71, 0x42, 0xcd, 0x35, 0x8f, 0x1b, 0x60,
        0x93, 0x2a, 0xf7, 0x05, 0xd4, 0x66, 0x19, 0xbe, 0x83, 0x4c, 0x27, 0xa0, 0x5f, 0xe1, 0x38,
        0x74, 0x9b,
    ])
}

fn empty_system_account(lamports: u64) -> Account {
    Account {
        lamports,
        data: Vec::new(),
        owner: SYSTEM_PROGRAM,
        executable: false,
        rent_epoch: 0,
    }
}

fn derive_stage(funder: Address, context: Hash32, digest: Hash32) -> Address {
    Address::find_program_address(
        &[
            seeds::SEED_ARTIFACT_STAGE,
            funder.as_ref(),
            &[ArtifactKind::Terms.byte()],
            &context.bytes(),
            &digest.bytes(),
        ],
        &PROGRAM_ID,
    )
    .0
}

fn derive_final(context: Hash32, digest: Hash32) -> (Address, u8) {
    Address::find_program_address(
        &[seeds::SEED_TERMS, &context.bytes(), &digest.bytes()],
        &PROGRAM_ID,
    )
}

/// A valid smooth Terms artifact at `basis_degree ∈ {2, 3}`.
///
/// `knot_count = n + 1 − d` is the §2.1 count rule; the anchors are a positive
/// uniform `2^SPACING` grid, which is what `uniform_rust_expanded_knot_linkage`
/// is stated over, and the payout map is entirely unused because a
/// derived-basis market has no cell-to-preset table.
fn smooth_terms(realm: Hash32, degree: u8) -> TermsAccount {
    let mut terms = fixture_terms(
        realm,
        Hash32::from_bytes([0x77; 32]),
        Hash32::from_bytes([0x78; 32]),
    );
    terms.outcome_count = CLAIMS;
    terms.payout_count = 1;
    terms.payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    let mut weights = [0_u64; MAX_OUTCOMES];
    weights[0] = DENOMINATOR;
    terms.payouts[0] = PayoutVectorBytes {
        denominator: DENOMINATOR,
        weights,
    };
    terms.failure_payout_index = 0;
    terms.payout_map = [PAYOUT_MAP_UNUSED; MAX_OUTCOMES];
    terms.basis_degree = degree;
    terms.knot_count = CLAIMS + 1 - degree;
    terms.uniform_log2_spacing = SPACING;
    terms.knots = [0; MAX_KNOTS];
    for (index, knot) in terms
        .knots
        .iter_mut()
        .take(usize::from(terms.knot_count))
        .enumerate()
    {
        *knot = (index as u128) * (1 << SPACING);
    }
    terms.terms = terms.recomputed_terms_digest().expect("smooth terms body");
    terms
}

/// Encode a valid smooth Terms at its canonical PDA bump.
fn encode_smooth(realm: Hash32, degree: u8) -> (Vec<u8>, Hash32) {
    let mut terms = smooth_terms(realm, degree);
    terms.stored_bump = derive_final(realm, terms.terms).1;
    let mut body = vec![0; account_len::TERMS];
    assert_eq!(terms.encode(&mut body), Ok(account_len::TERMS));
    (body, terms.terms)
}

/// Take a valid encoding to the exact bytes `mutate` would have produced, by
/// patching the named fields and restoring the self-certifying digest the
/// mutated struct computes for itself.
///
/// The digest is restored on purpose: without it every case below would refuse
/// as `NonCanonicalIdentity` and prove nothing about the basis rules.
fn malformed_body(
    realm: Hash32,
    degree: u8,
    mutate: impl Fn(&mut TermsAccount),
) -> (Vec<u8>, Hash32) {
    let (mut body, _) = encode_smooth(realm, degree);
    let mut hostile = smooth_terms(realm, degree);
    hostile.stored_bump = body[account_len::TERMS - 2];
    mutate(&mut hostile);
    body[OFFSET_BASIS_DEGREE] = hostile.basis_degree;
    body[OFFSET_KNOT_COUNT] = hostile.knot_count;
    body[OFFSET_SPACING] = hostile.uniform_log2_spacing;
    body[OFFSET_PAYOUT_MAP..OFFSET_PAYOUT_MAP + MAX_OUTCOMES].copy_from_slice(&hostile.payout_map);
    for (index, knot) in hostile.knots.iter().enumerate() {
        let at = OFFSET_KNOTS + (index * 16);
        body[at..at + 16].copy_from_slice(&knot.to_le_bytes());
    }
    let digest = hostile
        .recomputed_terms_digest()
        .expect("malformed terms still has a body");
    body[2..34].copy_from_slice(&digest.bytes());
    (body, digest)
}

fn new_bank(extra: &[(Address, Account)]) -> ProgramTest {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("clutch_sbf", PROGRAM_ID, None);
    for (address, account) in extra {
        test.add_account(*address, account.clone());
    }
    test
}

fn begin_ix(funder: Address, stage: Address, context: Hash32, digest: Hash32) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::BeginArtifact {
                kind: ArtifactKind::Terms,
                context,
                digest,
                exact_len: ArtifactKind::Terms.exact_len() as u16,
                expires_slot: 1_000,
            },
        ),
        vec![
            AccountMeta::new(funder, true),
            AccountMeta::new(stage, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

fn write_ix(
    funder: Address,
    stage: Address,
    context: Hash32,
    digest: Hash32,
    cursor: usize,
    body: &[u8],
) -> Instruction {
    let chunk_len = (body.len() - cursor).min(ARTIFACT_CHUNK_BYTES);
    let mut chunk = [0; ARTIFACT_CHUNK_BYTES];
    chunk[..chunk_len].copy_from_slice(&body[cursor..cursor + chunk_len]);
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::WriteArtifact {
                kind: ArtifactKind::Terms,
                context,
                digest,
                cursor: cursor as u16,
                chunk_len: chunk_len as u16,
                chunk,
            },
        ),
        vec![
            AccountMeta::new_readonly(funder, true),
            AccountMeta::new(stage, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

fn seal_ix(
    funder: Address,
    stage: Address,
    final_account: Address,
    context: Hash32,
    digest: Hash32,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::SealArtifact {
                kind: ArtifactKind::Terms,
                context,
                digest,
                exact_len: ArtifactKind::Terms.exact_len() as u16,
            },
        ),
        vec![
            AccountMeta::new(funder, true),
            AccountMeta::new(stage, false),
            AccountMeta::new(final_account, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

async fn send(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    signer: &Keypair,
) -> Result<(), TransactionError> {
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer, signer],
        blockhash,
    );
    context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .unwrap()
        .result
}

fn custom(result: Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => code,
        other => panic!("expected a custom refusal, got {other:?}"),
    }
}

async fn account(context: &mut ProgramTestContext, address: Address) -> Option<Account> {
    context.banks_client.get_account(address).await.unwrap()
}

/// Stage every chunk of `body`, then attempt the seal and return its result.
async fn stage_then_seal(
    context: &mut ProgramTestContext,
    author: &Keypair,
    realm: Hash32,
    digest: Hash32,
    body: &[u8],
) -> (Result<(), TransactionError>, Address) {
    let stage = derive_stage(author.pubkey(), realm, digest);
    let (final_account, _) = derive_final(realm, digest);
    send(
        context,
        begin_ix(author.pubkey(), stage, realm, digest),
        author,
    )
    .await
    .expect("smooth terms upload begins");
    let mut cursor = 0;
    while cursor < body.len() {
        send(
            context,
            write_ix(author.pubkey(), stage, realm, digest, cursor, body),
            author,
        )
        .await
        .expect("smooth terms chunk writes");
        cursor += ARTIFACT_CHUNK_BYTES.min(body.len() - cursor);
    }
    let result = send(
        context,
        seal_ix(author.pubkey(), stage, final_account, realm, digest),
        author,
    )
    .await;
    (result, final_account)
}

#[tokio::test]
async fn degree_two_and_three_terms_seal_into_the_bank() {
    let author = uploader();
    for (index, degree) in [2_u8, 3].into_iter().enumerate() {
        let realm = Hash32::from_bytes([0x90 + index as u8; 32]);
        let (body, digest) = encode_smooth(realm, degree);
        assert_eq!(
            TermsAccount::decode(&body).map(|t| t.basis_degree),
            Ok(degree),
            "the host codec admits degree {degree} before the bank is asked"
        );
        let mut context = new_bank(&[(author.pubkey(), empty_system_account(UPLOADER_LAMPORTS))])
            .start_with_context()
            .await;
        let (result, final_account) =
            stage_then_seal(&mut context, &author, realm, digest, &body).await;
        result.unwrap_or_else(|error| panic!("degree {degree} terms must seal: {error:?}"));

        let sealed = account(&mut context, final_account)
            .await
            .expect("the sealed smooth terms account exists");
        assert_eq!(sealed.owner, PROGRAM_ID);
        assert_eq!(sealed.data.len(), account_len::TERMS);
        let decoded = TermsAccount::decode(&sealed.data).expect("sealed bytes decode");
        assert_eq!(decoded.basis_degree, degree);
        assert_eq!(
            usize::from(decoded.knot_count),
            usize::from(CLAIMS) + 1 - usize::from(degree)
        );
        assert_eq!(decoded.uniform_log2_spacing, SPACING);
        assert_eq!(decoded.terms, digest);
        assert!(
            account(&mut context, derive_stage(author.pubkey(), realm, digest))
                .await
                .is_none()
        );
    }
}

#[tokio::test]
async fn malformed_degree_two_and_three_terms_refuse_at_seal() {
    let author = uploader();
    let mut case = 0_u8;
    for degree in [2_u8, 3] {
        /* Each entry is (name, mutation, expected refusal, Lean hypothesis).
         * The hypotheses live in `lean/DragonsClutch/BSpline.lean`. */
        #[allow(clippy::type_complexity)]
        let battery: Vec<(&str, Box<dyn Fn(&mut TermsAccount)>, CodecError)> = vec![
            (
                // `RustExpandedKnotLinkage`'s `degree ≤ 3`: above it no
                // construction exists to admit.
                "degree above the implemented ladder",
                Box::new(|t: &mut TermsAccount| t.basis_degree = 4),
                CodecError::InvalidEnum,
            ),
            (
                // The §2.1 count rule `K = n + 1 − d`. The local block has
                // arity `d + 1` (`clampedDegreeTwo_length`,
                // `clampedDegreeThree_length`) and `pad_length` places it
                // inside the `n`-vector, so the neighbouring degree's anchor
                // count denotes a different-width vector.
                "the neighbouring degree's anchor count",
                Box::new(move |t: &mut TermsAccount| {
                    t.basis_degree = if degree == 2 { 3 } else { 2 };
                }),
                CodecError::InvalidCount,
            ),
            (
                // `uniform_rust_expanded_knot_linkage` is stated only over
                // `uniformStoredKnots`; above degree one there is no
                // nonuniform counterpart, so the sentinel has no model.
                "the nonuniform sentinel above degree one",
                Box::new(|t: &mut TermsAccount| t.uniform_log2_spacing = UNIFORM_SPACING_NONE),
                CodecError::InvalidEnum,
            ),
            (
                // The array is the single semantic owner: a declaration it
                // refutes would name a different Lean grid than the one
                // stored.
                "a spacing declaration the anchors refute",
                Box::new(|t: &mut TermsAccount| t.uniform_log2_spacing = SPACING + 2),
                CodecError::InvalidEnum,
            ),
            (
                // `hgap : 0 < gap`, and `BasisFunsCell.distinct`: a flat pair
                // makes the recurrence divide by zero.
                "a flat anchor pair",
                Box::new(|t: &mut TermsAccount| t.knots[1] = t.knots[0]),
                CodecError::InvalidCount,
            ),
            (
                // Derived-basis markets have no cell-to-preset map.
                "a live payout map entry",
                Box::new(|t: &mut TermsAccount| t.payout_map[0] = 0),
                CodecError::NonCanonicalPadding,
            ),
            (
                // Canonical zero padding beyond the active anchor prefix.
                "a live anchor beyond the active prefix",
                Box::new(|t: &mut TermsAccount| t.knots[MAX_KNOTS - 1] = u128::MAX),
                CodecError::NonCanonicalPadding,
            ),
        ];

        for (name, mutate, expected) in battery {
            let realm = Hash32::from_bytes([0xA0 + case; 32]);
            case += 1;
            let (body, digest) = malformed_body(realm, degree, mutate);
            assert_eq!(
                TermsAccount::decode(&body),
                Err(expected),
                "degree {degree}: {name} must be exactly this refusal off-chain first"
            );

            let (final_account, _) = derive_final(realm, digest);
            let prefund = empty_system_account(1);
            let mut context = new_bank(&[
                (author.pubkey(), empty_system_account(UPLOADER_LAMPORTS)),
                (final_account, prefund.clone()),
            ])
            .start_with_context()
            .await;
            let (result, sealed_at) =
                stage_then_seal(&mut context, &author, realm, digest, &body).await;
            assert_eq!(sealed_at, final_account);
            assert_eq!(
                custom(result),
                clutch_sbf::error::codec_code(expected),
                "degree {degree}: {name} must refuse in the bank with the same class"
            );
            assert_eq!(
                account(&mut context, final_account).await.unwrap(),
                prefund,
                "degree {degree}: {name} leaves no residue at the canonical address"
            );
        }
    }
    assert_eq!(case, 14, "both smooth degrees ran the whole battery");
}
