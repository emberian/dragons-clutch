//! PHASE ZERO — the routing table, derived from chain instead of received.
//!
//! Stage one's phase one takes an eleven-row address book: which addresses to
//! observe, and the record digests that address them. The CLI projects that out
//! of one sealed campaign report, and a browser has no campaign report — so as
//! long as the book must be *supplied*, "the reader stops importing JSON" is
//! false no matter how much derivation runs afterwards.
//!
//! Seven of the eleven rows have chain pointers, and finding the seventh is the
//! measurement this module is built on. The Market is the caller's own; the
//! realm and Product record digests are in Core's identity; the result-domain
//! and portfolio digests are inside the Product record; the collateral mint and
//! its token program are in the Realm record.
//!
//! THE PRODUCT-BASIS ROW IS NOT IN THE CLAIMS AGGREGATE, which is where it
//! looks like it should be and where `userPositionAdmissionSnapshot.ts` reads
//! it from. The aggregate's `basis_id` is the SEMANTIC LiabilityBasisV2
//! identity: it authenticates a basis body and cannot address one, because the
//! semantic preimage ignores bytes the record digest covers. Measured on
//! devnet cohort-11: the raw-record PDA derived from `basis_id` under the
//! graded-basis schema is VACANT, while the record the campaign published sits
//! at the PDA of a digest the aggregate does not carry.
//!
//! It is in the REDEEMING WALLET'S OWN ADMISSION RECORD.
//! `ProtocolPositionAdmissionEvidenceV2::linked_basis_record_digest` is the
//! only place on chain that names it, which is exactly right: a wallet holding
//! claims was admitted, and its admission record is what says which product
//! graph it was admitted against. Its address is a PDA of the aggregate and the
//! owner, so it costs no round of its own.
//!
//! THE OTHER FOUR HAVE NO POINTER ANYWHERE, and that is why this module is a
//! compiler rather than a reader. The four `terminal_composition_*` records are
//! not *stored* facts about the market — they are *compiled* from it, by
//! [`compile_native_basis_composition_v1`], which is the same function the
//! founding ran to publish them. So the browser does not look them up; it
//! recompiles them and takes their digests. A record whose bytes the caller can
//! reconstruct needs no document to name it.
//!
//! # Two more rounds, and exactly two
//!
//! Round one is phase one's, plus the owner's admission record: three accounts,
//! all addressable before any read.
//!
//! - **Round two** ([`routing_round_two_addresses_v1`]) — the realm, Product
//!   and product-basis records, addressed by digests round one produced.
//! - **Round three** ([`routing_round_three_addresses_v1`]) — the result-domain
//!   and portfolio records, addressed by digests inside the Product record, and
//!   the price-gate certificate when the basis names one. It cannot merge with
//!   round two: its addresses are inside round two's bytes.
//! - Then [`derive_terminal_routing_table_v1`] compiles and returns the book.
//!
//! Four rounds total for a redemption, where the CLI took two plus two files.

use dclutch_claims::{
    liability_basis_state_v2::LiabilityBasisMarketViewV2,
    protocol_position_v2::{ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2},
};
use dclutch_market::CoreState;
use dclutch_product::payoff::{
    registry_v3::{GRADED_BASIS_RECORD_SCHEMA_ID_V3, PRICE_GATE_RECORD_SCHEMA_ID_V1},
    runtime_v3::ProductBasisV3,
};
use dclutch_product::admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
    RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_market::realm::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_representation_composition_v3_operator::native_categorical_v1::{
    NativeBasisCompositionInputV1, compile_native_basis_composition_v1,
};
use dclutch_wallet_terminal_payout_operator::{
    Error, Result,
    wire::{FinalizedSnapshotV1, record_pair},
};
use solana_program::{hash::hash, pubkey::Pubkey};

use crate::{
    ProtocolCoordinatesV1, RoutedRecordV1, TerminalRecordRoutingV1, TerminalRoutingTableV1,
    claims_aggregate_address_v1, decode_routed_market_v1, observed_custody_context_v1,
};

/// One published record's body, authenticated against the digest that addresses
/// it.
///
/// The digest is not read out of the account; it is the SEED the address was
/// derived from, so re-hashing the observed bytes and comparing is a real check
/// with two independent sources — the chain pointer that named the record, and
/// the bytes the chain returned.
fn record_body<'a>(
    snapshot: &'a FinalizedSnapshotV1,
    registry: Pubkey,
    schema: [u8; 32],
    digest: [u8; 32],
    label: &str,
) -> Result<(&'a [u8], RoutedRecordV1)> {
    let pair = record_pair(registry, schema, digest);
    let account = snapshot.required(pair.raw, label)?;
    if account.owner != registry || account.executable || account.data.is_empty() {
        return Err(Error::new(format!(
            "{label} at {} is owned by {}, not the deployment's Registry {registry}",
            pair.raw, account.owner
        )));
    }
    if hash(&account.data).to_bytes() != digest {
        return Err(Error::new(format!(
            "{label} at {} does not hash to the digest that addresses it",
            pair.raw
        )));
    }
    Ok((
        account.data.as_slice(),
        RoutedRecordV1 {
            digest,
            address: pair.raw,
        },
    ))
}

/// What round one's two observations determine.
struct RoundOneFactsV1 {
    realm_digest: [u8; 32],
    product_digest: [u8; 32],
    basis_digest: [u8; 32],
    release_set: [u8; 32],
}

/// The owner's Claims admission record, addressable before any read.
pub fn claims_admission_address_v1(
    claims: Pubkey,
    aggregate: Pubkey,
    owner: Pubkey,
) -> Result<Pubkey> {
    let seeds = ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), owner.to_bytes())
        .map_err(|error| Error::new(format!("Claims admission seeds: {error:?}")))?;
    Ok(Pubkey::find_program_address(&seeds.as_slices(), &claims).0)
}

/// PHASE ZERO, ROUND ONE — the three accounts every later round hangs off.
///
/// Phase one reads two of these; the third, the owner's admission record, is
/// what carries the linked-basis record digest. All three are PDAs of the
/// deployment's coordinates and the caller's own Market and wallet, so a
/// browser with a deployment table and a connected wallet can name them
/// without reading anything first.
pub fn routing_round_one_addresses_v1(
    coordinates: &ProtocolCoordinatesV1,
    market: Pubkey,
    owner: Pubkey,
) -> Result<[Pubkey; 3]> {
    let aggregate = claims_aggregate_address_v1(coordinates.claims, market);
    Ok([
        market,
        aggregate,
        claims_admission_address_v1(coordinates.claims, aggregate, owner)?,
    ])
}

fn round_one_facts_v1(
    coordinates: &ProtocolCoordinatesV1,
    market: Pubkey,
    owner: Pubkey,
    round_one: &FinalizedSnapshotV1,
) -> Result<RoundOneFactsV1> {
    let market_account = round_one.required(market, "Core Market")?;
    let state: CoreState = decode_routed_market_v1(market_account, coordinates)?;
    let aggregate_key = claims_aggregate_address_v1(coordinates.claims, market);
    let aggregate_account = round_one.required(aggregate_key, "Claims aggregate")?;
    // Reuse phase one's own authentication of the aggregate rather than
    // repeating its clauses here; a second copy would be the mirror this whole
    // sequence exists to avoid.
    observed_custody_context_v1(aggregate_account, coordinates.claims, market)?;
    let aggregate = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Claims aggregate: {error:?}")))?;
    if aggregate.realm_id != state.identity.realm_id.to_bytes()
        || aggregate.registry_program != state.identity.registry_program.to_bytes()
        || aggregate.release_set != state.identity.selected_release_set.to_bytes()
    {
        return Err(Error::new(
            "the Claims aggregate and the Core Market disagree about Realm, Registry or release set",
        ));
    }
    // THE THIRD ACCOUNT, and the join that makes it safe to believe.
    //
    // The admission record is Claims-owned and addressed by the aggregate and
    // the owner, so it cannot be substituted; and its evidence is checked
    // against the two accounts already read rather than trusted. A record that
    // names another market, release set, Product graph or semantic basis is
    // refused here, where the reason can be stated, instead of surfacing forty
    // accounts later as a frame refusal.
    let admission_key = claims_admission_address_v1(coordinates.claims, aggregate_key, owner)?;
    let admission_account = round_one.required(admission_key, "Claims admission record")?;
    if admission_account.owner != coordinates.claims || admission_account.executable {
        return Err(Error::new(format!(
            "the Claims admission record at {admission_key} is owned by {}, not the deployment's Claims program {}",
            admission_account.owner, coordinates.claims
        )));
    }
    let admission = ProtocolPositionAdmissionV2::decode(&admission_account.data)
        .map_err(|error| Error::new(format!("Claims admission record: {error:?}")))?;
    let request = admission.request();
    let evidence = admission.evidence();
    if request.market != market.to_bytes()
        || request.position_owner != owner.to_bytes()
        || request.release_set != state.identity.selected_release_set.to_bytes()
        || evidence.product_record_digest != state.identity.product_record.to_bytes()
        || evidence.semantic_basis_id != aggregate.basis_id
    {
        return Err(Error::new(
            "the Claims admission record names another Market, owner, release set, Product record or semantic basis than this Market's own state",
        ));
    }
    if evidence.linked_basis_record_digest == [0; 32] {
        return Err(Error::new(
            "the Claims admission record carries no linked-basis record digest",
        ));
    }

    Ok(RoundOneFactsV1 {
        realm_digest: state.identity.realm_id.to_bytes(),
        product_digest: state.identity.product_record.to_bytes(),
        // NOT `aggregate.basis_id`: that is the semantic identity, which
        // authenticates a basis body and cannot address one.
        basis_digest: evidence.linked_basis_record_digest,
        release_set: state.identity.selected_release_set.to_bytes(),
    })
}

/// PHASE ZERO, ROUND TWO — the three records round one's digests address.
///
/// The realm record (Core's `realm_id`), the Product record (Core's
/// `product_record`) and the linked product-basis record (the aggregate's
/// `basis_id`). Every one is a raw-record PDA of a digest the chain already
/// published; none is read out of a document.
pub fn routing_round_two_addresses_v1(
    coordinates: &ProtocolCoordinatesV1,
    market: Pubkey,
    owner: Pubkey,
    round_one: &FinalizedSnapshotV1,
) -> Result<Vec<Pubkey>> {
    let facts = round_one_facts_v1(coordinates, market, owner, round_one)?;
    Ok(vec![
        record_pair(
            coordinates.registry,
            REALM_SCHEMA_RELEASE_ID_V1,
            facts.realm_digest,
        )
        .raw,
        record_pair(
            coordinates.registry,
            PRODUCT_RECORD_SCHEMA_ID_V2,
            facts.product_digest,
        )
        .raw,
        record_pair(
            coordinates.registry,
            GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            facts.basis_digest,
        )
        .raw,
    ])
}

/// PHASE ZERO, ROUND THREE — the records round two's BYTES address.
///
/// The result-domain and portfolio digests live inside the Product record and
/// the price-gate digest inside the basis, so these addresses are not knowable
/// until round two has returned. That is the whole reason there are two extra
/// rounds and not one.
///
/// A basis that names no price gate returns two addresses; one that names a
/// certificate returns three. The basis decides, not this function.
pub fn routing_round_three_addresses_v1(
    coordinates: &ProtocolCoordinatesV1,
    market: Pubkey,
    owner: Pubkey,
    round_one: &FinalizedSnapshotV1,
    round_two: &FinalizedSnapshotV1,
) -> Result<Vec<Pubkey>> {
    let facts = round_one_facts_v1(coordinates, market, owner, round_one)?;
    let (product_bytes, _) = record_body(
        round_two,
        coordinates.registry,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        facts.product_digest,
        "Product record",
    )?;
    let product = ProductRecordV2::decode(product_bytes)
        .map_err(|error| Error::new(format!("Product record: {error:?}")))?;
    let (basis_bytes, _) = record_body(
        round_two,
        coordinates.registry,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        facts.basis_digest,
        "product-basis record",
    )?;
    let basis = ProductBasisV3::decode(basis_bytes)
        .map_err(|error| Error::new(format!("product-basis record: {error:?}")))?;
    let mut addresses = vec![
        record_pair(
            coordinates.registry,
            RESULT_DOMAIN_SCHEMA_ID_V2,
            product.result_domain_digest().to_bytes(),
        )
        .raw,
        record_pair(
            coordinates.registry,
            PORTFOLIO_SCHEMA_ID_V2,
            product.portfolio_digest().to_bytes(),
        )
        .raw,
    ];
    let price_gate = basis.price_gate_certificate_digest_v3();
    if price_gate != [0; 32] {
        addresses.push(
            record_pair(
                coordinates.registry,
                PRICE_GATE_RECORD_SCHEMA_ID_V1,
                price_gate,
            )
            .raw,
        );
    }
    Ok(addresses)
}

/// PHASE ZERO — the eleven-row address book, derived rather than received.
///
/// Seven rows are read from chain pointers. The four `terminal_composition_*`
/// rows are RECOMPILED by the same function the founding published them with,
/// which authenticates the whole product graph on the way: that the Product
/// record's two child digests are the bodies observed, that the join reproduces
/// the Product identity, that the basis binds to the same result domain and
/// width, and that the price-gate certificate verifies against the basis. A
/// substituted body cannot reach a composition digest.
pub fn derive_terminal_routing_table_v1(
    coordinates: &ProtocolCoordinatesV1,
    market: Pubkey,
    owner: Pubkey,
    round_one: &FinalizedSnapshotV1,
    round_two: &FinalizedSnapshotV1,
    round_three: &FinalizedSnapshotV1,
) -> Result<TerminalRoutingTableV1> {
    let facts = round_one_facts_v1(coordinates, market, owner, round_one)?;
    let (realm_bytes, realm) = record_body(
        round_two,
        coordinates.registry,
        REALM_SCHEMA_RELEASE_ID_V1,
        facts.realm_digest,
        "realm record",
    )?;
    let realm_record = RealmV1::decode(realm_bytes)
        .map_err(|error| Error::new(format!("realm record: {error:?}")))?;
    let (product_bytes, product) = record_body(
        round_two,
        coordinates.registry,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        facts.product_digest,
        "Product record",
    )?;
    let (basis_bytes, product_basis) = record_body(
        round_two,
        coordinates.registry,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        facts.basis_digest,
        "product-basis record",
    )?;
    let decoded_product = ProductRecordV2::decode(product_bytes)
        .map_err(|error| Error::new(format!("Product record: {error:?}")))?;
    let decoded_basis = ProductBasisV3::decode(basis_bytes)
        .map_err(|error| Error::new(format!("product-basis record: {error:?}")))?;

    let (result_domain_bytes, result_domain) = record_body(
        round_three,
        coordinates.registry,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        decoded_product.result_domain_digest().to_bytes(),
        "result-domain record",
    )?;
    let (portfolio_bytes, portfolio) = record_body(
        round_three,
        coordinates.registry,
        PORTFOLIO_SCHEMA_ID_V2,
        decoded_product.portfolio_digest().to_bytes(),
        "portfolio record",
    )?;
    let price_gate_digest = decoded_basis.price_gate_certificate_digest_v3();
    let price_gate_bytes = if price_gate_digest == [0; 32] {
        None
    } else {
        Some(
            record_body(
                round_three,
                coordinates.registry,
                PRICE_GATE_RECORD_SCHEMA_ID_V1,
                price_gate_digest,
                "price-gate certificate record",
            )?
            .0,
        )
    };

    // THE FOUR WITH NO POINTER. Compiled by the founding's own function, which
    // refuses every cross-record disagreement on the way through, so a book
    // that comes out of here has already proved the graph it addresses.
    let compiled = compile_native_basis_composition_v1(NativeBasisCompositionInputV1 {
        market: market.to_bytes(),
        release_set: facts.release_set,
        product_record_bytes: product_bytes,
        result_domain_bytes,
        portfolio_bytes,
        product_basis_bytes: basis_bytes,
        price_gate_bytes,
    })
    .map_err(|error| {
        Error::new(format!(
            "the native terminal composition does not compile from this market's own records: {error:?}"
        ))
    })?;
    let targets = compiled.publication_targets();
    let composed = |index: usize| -> RoutedRecordV1 {
        let target = &targets[index];
        let digest = hash(target.bytes).to_bytes();
        RoutedRecordV1 {
            digest,
            address: record_pair(coordinates.registry, target.schema_id, digest).raw,
        }
    };

    Ok(TerminalRoutingTableV1 {
        founding_market: market,
        collateral_mint: Pubkey::new_from_array(*realm_record.collateral_mint()),
        token_program: Pubkey::new_from_array(*realm_record.token_program()),
        records: TerminalRecordRoutingV1 {
            realm,
            product,
            result_domain,
            portfolio,
            product_basis,
            composition_descriptor: composed(0),
            composition_graph: composed(1),
            composition_translation: composed(2),
            composition_exposure: composed(3),
        },
    })
}

#[cfg(test)]
mod tests {
    use dclutch_wallet_terminal_payout_operator::{hex32, pubkey};

    use super::*;

    fn coordinates() -> ProtocolCoordinatesV1 {
        let input = dclutch_wallet_terminal_payout_operator::wire::tests::input();
        ProtocolCoordinatesV1 {
            registry: pubkey(&input.programs.registry).unwrap(),
            core: pubkey(&input.programs.core).unwrap(),
            claims: pubkey(&input.programs.claims).unwrap(),
            custody: pubkey(&input.programs.custody).unwrap(),
            resolution: pubkey(&input.programs.resolution).unwrap(),
            release_set: hex32(&input.release_set).unwrap(),
        }
    }

    /// Round one names three accounts, and every one is knowable before a read.
    ///
    /// That is the property that lets a browser holding only a deployment table
    /// and a connected wallet start the sequence: no address here depends on
    /// bytes it has not fetched yet.
    #[test]
    fn round_one_is_three_addresses_and_none_of_them_needs_a_read() {
        let coordinates = coordinates();
        let market = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let keys = routing_round_one_addresses_v1(&coordinates, market, owner).expect("round one");
        assert_eq!(keys[0], market);
        assert_eq!(
            keys[1],
            claims_aggregate_address_v1(coordinates.claims, market)
        );
        assert_eq!(
            keys[2],
            claims_admission_address_v1(coordinates.claims, keys[1], owner).expect("admission")
        );
        assert_eq!(
            keys.iter().collect::<std::collections::BTreeSet<_>>().len(),
            3,
            "three coordinates that collapse to two would authenticate an alias"
        );
    }

    /// The admission record is per OWNER, which is what makes it the right
    /// place for this: a wallet that was admitted carries the graph it was
    /// admitted against.
    #[test]
    fn a_different_owner_reads_a_different_admission_record() {
        let coordinates = coordinates();
        let market = Pubkey::new_unique();
        let aggregate = claims_aggregate_address_v1(coordinates.claims, market);
        let first =
            claims_admission_address_v1(coordinates.claims, aggregate, Pubkey::new_unique())
                .expect("first");
        let second =
            claims_admission_address_v1(coordinates.claims, aggregate, Pubkey::new_unique())
                .expect("second");
        assert_ne!(first, second);
    }

    /// A round one whose accounts are missing refuses by name rather than
    /// producing a book addressed from zeroes.
    #[test]
    fn an_empty_round_one_refuses_by_name() {
        let coordinates = coordinates();
        let market = Pubkey::new_unique();
        let owner = Pubkey::new_unique();
        let vacant = FinalizedSnapshotV1::from_observed(9, 1, &[market], vec![None])
            .expect("a vacant observation is still a snapshot");
        let error = routing_round_two_addresses_v1(&coordinates, market, owner, &vacant)
            .expect_err("a vacant Market must refuse");
        assert!(error.to_string().contains("Core Market"), "{error}");
    }
}
