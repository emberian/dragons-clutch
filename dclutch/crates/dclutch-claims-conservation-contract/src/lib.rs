#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Conservative complete-set semantics: the missing owner of Claims split and
//! merge.
//!
//! # The hole this closes
//!
//! `dclutch-claims-sbf` already knows how to move a complete set. Its legacy
//! generic route maps `ClaimsAction::MintCompleteSet` and `MergeCompleteSet`
//! onto `dclutch_economic_slice_kernel::BasketAction`, and the slice kernel
//! credits or debits the aggregate's `hoard` scalar by the complete-set count
//! (`crates/dclutch-economic-slice-kernel/src/lib.rs:741-758`). Two distinct
//! things go wrong there, and both are executed rather than asserted in
//! `tests.rs`:
//!
//! 1. **Mint demands no collateral at all.** `execute_basket` credits the Hoard
//!    scalar and returns `Payout { amount: 0 }`. There is no collateral
//!    coordinate anywhere in the call, so an adapter driving the bare action has
//!    nothing to transfer and nothing to refuse: claims come into existence
//!    against a Hoard that received no atoms.
//! 2. **Merge reports a payout in the wrong unit.** It returns
//!    `Payout { amount: complete_quantity }` — complete SETS — while a Custody
//!    transfer moves collateral ATOMS. An adapter that paid `Payout::amount`
//!    would return `quantity` where `quantity * basis_scale` is owed. The error
//!    is invisible at `basis_scale == 1`, which is what the in-tree fixtures use.
//!
//! Neither the legacy generic route nor anything downstream of it builds a
//! `CustodyRequestV1`: the Claims program constructs exactly two, in
//! `rational_terminal_v3.rs:351` and `custody_replay_v1.rs:181`. The program
//! even declares a refusal for this — `ClaimsSbfError::CustodyRequired = 0x5006`,
//! "This action requires the canonical Custody child composition" — and raises
//! it nowhere.
//!
//! This crate owns the join that is missing: one uniform signed claim delta
//! across the complete set, and one Claims-role Custody transfer of exactly
//! `quantity * basis_scale` collateral atoms, derived from the same request so
//! that neither half can be constructed without the other.
//!
//! # What this crate is NOT, and the wall it stops at
//!
//! It is a `no_std`, allocation-free, total contract. It authenticates no
//! account. Realm, Product/Basis, release, owner signature, PDA derivation, the
//! Custody CPI and rollback all stay in the composing SBF adapter, exactly as
//! they do for `dclutch-custody-contract`.
//!
//! **The Claims-owned outer route that would call this does not exist.** Nothing
//! on chain dispatches `CLAIMS_CONSERVATION_REQUEST_MAGIC_V1`; no operator
//! builds it; no client can send it. Split and merge remain UNIMPLEMENTED as
//! user acts, and this crate does not make them reachable. What it does is fix
//! the semantics, the arithmetic, the Custody coupling and the replay
//! coordinate so that the route, when it is written, has one owner to obey
//! rather than a shape to reinvent.
//!
//! # The unit convention, verified rather than assumed
//!
//! A Market's Hoard scalar counts complete SETS, not collateral atoms
//! (`dclutch_economic_slice_kernel::market_hoard`'s own doc, verified against
//! `basket_hoard_and_payout`). The conversion to atoms is `basis_scale`, and
//! `basis_scale` is the authenticated `ProductBasisV3::payout_scale` that Core
//! pins into the founding intent (`programs/dclutch-core-sbf/src/generic_founding_v1.rs:1099`,
//! rejoined at `:1300`). The founding route already spends exactly this
//! product: `ClaimsFoundingRequestV5` computes
//! `collateral = quantity.checked_mul(basis_scale)`
//! (`crates/dclutch-claims-svm/src/founding_v5.rs:279-282`) and its own vector
//! asserts `(7, 11, 77)` at `:983-990`. [`collateral_atoms_v1`] is that same
//! arithmetic and nothing else.
//!
//! The multiplication is EXACT and has no rounding boundary. The protocol's one
//! named rounding boundary in this direction is the opposite projection —
//! `MarketPrincipalCapV1::in_complete_sets`, `floor(cap_atoms / basis_scale)`,
//! named by decision 0013
//! (`crates/dclutch-source-contract/src/principal_capacity_v1.rs:542-565`).
//! Nothing here rounds; an inexact result is a refusal, not a remainder.
//!
//! ## The premise underneath `basis_scale`, and where it is NOT proved
//!
//! `quantity * basis_scale` is the right deposit exactly when one Claims
//! complete set is worth `basis_scale` collateral atoms at terminal. What
//! terminal settlement actually charges the Hoard is
//! `Σ_k supply[k] * claims_payout[k]`, refused as `Insolvent` when it exceeds
//! the Hoard's token balance
//! (`crates/dclutch-claims-svm/src/product_basis_terminal_v3.rs:400-444`), so the
//! premise is `Σ_k claims_payout[k] == basis_scale`.
//!
//! Two of the three steps are verified. `validate_partition` requires the
//! PRODUCT payout vector to sum to `payout_scale` exactly, for every terminal
//! scenario (`product_basis_terminal_v3.rs:391`, `:623-631`). And every
//! generically-founded Market has Claims width equal to Product width:
//! `programs/dclutch-core-sbf/src/generic_founding_v1.rs:1090` refuses with
//! `CoreSbfError::Reference` unless `basis_width == runtime.outcome_count`, and
//! `:1096-1100` sets `claim_count` from that same width.
//!
//! The third is NOT enforced, and the reason is larger than this route.
//! `claims_payout` is the product vector put through `translate_product_payouts`
//! (`crates/dclutch-representation-composition-v3-kernel/src/exposure.rs:485-535`),
//! a per-row rational combination. That kernel's `validate` (`:537-596`)
//! constrains widths, row ordering, nonzero denominators, unique node ids,
//! increasing in-range coordinates and gcd normalization — and states no sum
//! condition at all. No caller supplies one either: every `validate_partition`
//! call in the tree runs on the width-N PRODUCT vector or on a basis record's own
//! table, never on the width-K translated vector.
//!
//! On the generic settlement route the exposure is not even pinned to the
//! Market. `programs/dclutch-claims-sbf/src/terminal_settlement_v3.rs` takes
//! `exposure_id` and `exposure_digest` as ordinary instruction fields
//! (`crates/dclutch-claims-svm/src/terminal_settlement_v3.rs:151-154`),
//! authenticates the account as nothing more than "a finalized record whose
//! bytes hash to that digest" (`terminal_settlement_v3.rs:680-712` — PDA, owner,
//! rent, digest, vacant staging; no market and no product), and joins it on five
//! header fields the record's own author writes plus the two widths
//! (`exposure.rs:383-401`). The one identity check that looks like it would catch
//! a substitution is a tautology: `bundle_id` is assigned from
//! `admission.selected_id` (`exposure.rs:274`) and the adapter sets that field to
//! `input.exposure_id` (`terminal_settlement_v3.rs:393-401`), so it compares the
//! instruction to itself. Registry publication is permissionless
//! (`programs/dclutch-registry-sbf/src/record_v1.rs:1`), and founding pins no
//! exposure identity whatsoever — `generic_founding_v1.rs` contains zero
//! occurrences of exposure, graph, composition or descriptor.
//!
//! Decision 0011 already recorded that "the live route … checks the bundle's
//! identity, digest and width and never the coefficients", and judged it
//! tolerable on one stated premise: "a wrong recipe is a wrong founding rather
//! than a forgeable request"
//! (`docs/decisions/0011-structured-v2-physical-route.md:510-522`). That premise
//! holds for `rational_terminal_v3`, which takes the exposure identity from an
//! authenticated `RepresentationDescriptorV2`. It does NOT hold here, where
//! nothing upstream ever fixed a recipe for the redeemer's choice to be measured
//! against.
//!
//! What that costs is bounded in one direction only. The solvency refusal
//! (`product_basis_terminal_v3.rs:442-444`) caps the SUPPLY-WEIGHTED sum at the
//! Hoard balance, so aggregate over-payment is caught; a recipe that pays less
//! than `basis_scale` per set, or that pays the right total to the wrong
//! coordinate, is not — the inequality says nothing about which claim is paid.
//!
//! None of this is introduced by split or merge: founding already charges
//! `quantity * basis_scale` over the very same coordinates, so a split is exactly
//! as sound as the founding before it. It is recorded here because this crate is
//! the first thing to write the premise down, and because the fix — pinning an
//! exposure identity at founding, the way the descriptor pins it for Rational —
//! belongs at founding, at generic settlement and at this route together. This
//! lane did not attempt it: `programs/dclutch-claims-sbf/` is another lane's.
//! Nothing here has been executed against a chain, and no proof-of-concept was
//! built; what is claimed is what the code paths admit when read.
//!
//! # The replay coordinate
//!
//! Split and merge reuse the EXISTING Custody replay coordinate
//! `[b"dclutch:custody-replay:v1", market, release_set, ExecutionRoleV1::Claims,
//! aggregate.custody_context]`. They do not name a new one. See
//! [`ClaimsConservationRequestV1::custody_replay_seeds`] for the argument and
//! the tests that pin it.

use dclutch_custody_contract::{
    CallerRoleV1, CompartmentV1, ContextV1, CustodyReplaySeedsV1, CustodyRequestV1,
    CustodyVaultSeedsV1, DelegatedCustodyRequestV2, OperationV1,
};
use dclutch_source_contract::MarketPrincipalCapSetsV1;

#[cfg(test)]
extern crate std;

/// Canonical outer-route request magic.
pub const CLAIMS_CONSERVATION_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLCNS01";
/// Implemented schema version.
pub const CLAIMS_CONSERVATION_SCHEMA_VERSION_V1: u16 = 1;
/// Exact canonical request width.
pub const CLAIMS_CONSERVATION_REQUEST_BYTES_V1: usize = 592;

const MAGIC_OFFSET: usize = 0;
const VERSION_OFFSET: usize = 8;
const DIRECTION_OFFSET: usize = 10;
const HEADER_RESERVED_OFFSET: usize = 11;
const HEADER_RESERVED_BYTES: usize = 5;
const REALM_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const RELEASE_SET_OFFSET: usize = 80;
const CUSTODY_CONTEXT_OFFSET: usize = 112;
const AGGREGATE_OFFSET: usize = 144;
const POSITION_OFFSET: usize = 176;
const OWNER_OFFSET: usize = 208;
const EXTERNAL_COLLATERAL_OFFSET: usize = 240;
const HOARD_VAULT_OFFSET: usize = 272;
const MINT_OFFSET: usize = 304;
const TOKEN_PROGRAM_OFFSET: usize = 336;
const CLAIMS_PROGRAM_OFFSET: usize = 368;
const PRODUCT_RECORD_DIGEST_OFFSET: usize = 400;
const LINKED_BASIS_RECORD_DIGEST_OFFSET: usize = 432;
const SEMANTIC_BASIS_ID_OFFSET: usize = 464;
const GENERATION_OFFSET: usize = 496;
const QUANTITY_OFFSET: usize = 504;
const BASIS_SCALE_OFFSET: usize = 512;
const COLLATERAL_ATOMS_OFFSET: usize = 520;
const EXPECTED_MARKET_REVISION_OFFSET: usize = 528;
const EXPECTED_POSITION_REVISION_OFFSET: usize = 536;
const EXPECTED_CUSTODY_REVISION_OFFSET: usize = 544;
const PRE_EXTERNAL_AMOUNT_OFFSET: usize = 552;
const POST_EXTERNAL_AMOUNT_OFFSET: usize = 560;
const PRE_HOARD_AMOUNT_OFFSET: usize = 568;
const POST_HOARD_AMOUNT_OFFSET: usize = 576;
const CLAIM_COUNT_OFFSET: usize = 584;
const TAIL_RESERVED_OFFSET: usize = 588;
const TAIL_RESERVED_BYTES: usize = 4;

const IDENTITY_BYTES: usize = 32;
const SCALAR_BYTES: usize = 8;

/// Every refusal this contract can raise.
///
/// These are contract refusals, not chain codes: no `#[repr(u32)]`, no
/// discriminants, no band. The composing Claims adapter is the owner of the
/// protocol-visible taxonomy and maps these into its own registered sub-band,
/// exactly as `dclutch-claims-sbf` maps `dclutch_custody_contract::Error`
/// today. Declaring a `#[repr(u32)]` enum in a crate that no dispatcher reaches
/// would put a live-looking code in band 5 for a route that cannot be invoked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Input was not exactly [`CLAIMS_CONSERVATION_REQUEST_BYTES_V1`] wide.
    InvalidLength,
    /// Leading bytes were not [`CLAIMS_CONSERVATION_REQUEST_MAGIC_V1`].
    InvalidMagic,
    /// Schema version was not [`CLAIMS_CONSERVATION_SCHEMA_VERSION_V1`].
    InvalidVersion,
    /// A reserved span carried a nonzero byte.
    NonCanonical,
    /// The direction byte was neither split nor merge.
    UnknownDirection,
    /// A required identity coordinate was the zero pubkey.
    ZeroIdentity,
    /// The external collateral account and the Hoard vault were the same key.
    AliasedAccounts,
    /// Quantity, basis scale, or claim count was zero.
    InvalidQuantity,
    /// `quantity * basis_scale` did not fit `u64`.
    CollateralOverflow,
    /// The stated collateral did not equal `quantity * basis_scale`.
    CollateralMismatch,
    /// The stated external-account balances did not move by the collateral.
    ExternalBalanceMismatch,
    /// The stated Hoard balances did not move by the collateral.
    HoardBalanceMismatch,
    /// The Custody replay revision could not advance by exactly one.
    RevisionOverflow,
    /// This direction may not use the requested Custody wire.
    WrongDirectionWire,
    /// Custody refused the derived request's own shape.
    CustodyShape,
    /// A supplied buffer was not the exact runtime complete-set width.
    WidthMismatch,
    /// The proposed complete-set growth exceeded the Market's carried cap.
    PrincipalCapacity,
}

/// Result alias for this contract.
pub type Result<T> = core::result::Result<T, Error>;

/// The two conservative complete-set directions.
///
/// There is no third. A complete-set act either creates uniform claims against
/// new collateral or destroys uniform claims and returns the same collateral
/// class; anything else is a different route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ClaimsConservationDirectionV1 {
    /// Deposit collateral into the Market's Hoard and credit the complete set.
    Split = 0,
    /// Debit the complete set and return the same collateral class.
    Merge = 1,
}

impl ClaimsConservationDirectionV1 {
    const fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Split),
            1 => Ok(Self::Merge),
            _ => Err(Error::UnknownDirection),
        }
    }

    /// The canonical one-byte wire tag.
    #[must_use]
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// The sign every claim coordinate moves in under this direction.
    ///
    /// Uniformity is the whole invariant: the complete set moves as one, so
    /// there is a single sign and a single magnitude for every coordinate.
    #[must_use]
    pub const fn claim_delta_is_credit(self) -> bool {
        matches!(self, Self::Split)
    }

    /// The compartment collateral leaves under this direction.
    #[must_use]
    pub const fn source_compartment(self) -> CompartmentV1 {
        match self {
            Self::Split => CompartmentV1::External,
            Self::Merge => CompartmentV1::HoardPrincipal,
        }
    }

    /// The compartment collateral arrives in under this direction.
    #[must_use]
    pub const fn destination_compartment(self) -> CompartmentV1 {
        match self {
            Self::Split => CompartmentV1::HoardPrincipal,
            Self::Merge => CompartmentV1::External,
        }
    }
}

/// The exact collateral a complete-set act must move.
///
/// This is the sole arithmetic boundary of the conservation join and it is
/// EXACT: `quantity` complete sets are worth `quantity * basis_scale`
/// collateral atoms, and a product that does not fit `u64` is refused rather
/// than truncated. It is byte-for-byte the founding route's own rule
/// (`crates/dclutch-claims-svm/src/founding_v5.rs:276-282`), which is what
/// makes a split the same economic act as a second founding.
pub const fn collateral_atoms_v1(quantity: u64, basis_scale: u64) -> Result<u64> {
    if quantity == 0 || basis_scale == 0 {
        return Err(Error::InvalidQuantity);
    }
    match quantity.checked_mul(basis_scale) {
        Some(atoms) => Ok(atoms),
        None => Err(Error::CollateralOverflow),
    }
}

/// One conservative complete-set act, fully coordinated.
///
/// Every field is a coordinate the composing adapter must independently
/// authenticate. Nothing here is evidence; it is the exact statement the actor
/// is held to, and [`ClaimsConservationRequestV1::validate`] is what makes the
/// statement internally impossible to satisfy dishonestly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsConservationRequestV1 {
    /// Split or merge.
    pub direction: ClaimsConservationDirectionV1,
    /// Immutable Realm content identity.
    pub realm: [u8; 32],
    /// Canonical Core Market identity.
    pub market: [u8; 32],
    /// Immutable selected execution release set.
    pub release_set: [u8; 32],
    /// The Market's Custody namespace, as the Claims aggregate persists it.
    pub custody_context: [u8; 32],
    /// Canonical Claims aggregate account.
    pub aggregate: [u8; 32],
    /// The actor's canonical Position account.
    pub position: [u8; 32],
    /// The Position owner, who must sign.
    pub owner: [u8; 32],
    /// The actor's own external collateral token account.
    pub external_collateral: [u8; 32],
    /// The Market's canonical HoardPrincipal vault.
    pub hoard_vault: [u8; 32],
    /// Realm-selected collateral mint.
    pub mint: [u8; 32],
    /// Realm-selected Token or Token-2022 program.
    pub token_program: [u8; 32],
    /// The current release-selected Claims program, as Custody's caller.
    pub claims_program: [u8; 32],
    /// Digest of the finalized Product record.
    pub product_record_digest: [u8; 32],
    /// Digest of the finalized linked basis record.
    pub linked_basis_record_digest: [u8; 32],
    /// Semantic basis identity.
    pub semantic_basis_id: [u8; 32],
    /// Market generation.
    pub generation: u64,
    /// Exact complete sets created or destroyed.
    pub quantity: u64,
    /// Authenticated `ProductBasisV3::payout_scale`, in atoms per complete set.
    pub basis_scale: u64,
    /// Stated collateral atoms; must equal `quantity * basis_scale`.
    pub collateral_atoms: u64,
    /// Optimistic Claims aggregate revision before execution.
    pub expected_market_revision: u64,
    /// Optimistic Position revision before execution.
    pub expected_position_revision: u64,
    /// Optimistic Claims-role Custody replay revision before execution.
    pub expected_custody_revision: u64,
    /// The actor's external token balance before execution.
    pub pre_external_amount: u64,
    /// The actor's external token balance after execution.
    pub post_external_amount: u64,
    /// The Hoard vault's token balance before execution.
    pub pre_hoard_amount: u64,
    /// The Hoard vault's token balance after execution.
    pub post_hoard_amount: u64,
    /// Runtime complete-set width.
    pub claim_count: u32,
}

impl ClaimsConservationRequestV1 {
    /// Hostile-decode one exact canonical request.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != CLAIMS_CONSERVATION_REQUEST_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if read_array(input, MAGIC_OFFSET)? != CLAIMS_CONSERVATION_REQUEST_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(input, VERSION_OFFSET)? != CLAIMS_CONSERVATION_SCHEMA_VERSION_V1 {
            return Err(Error::InvalidVersion);
        }
        require_zero(input, HEADER_RESERVED_OFFSET, HEADER_RESERVED_BYTES)?;
        require_zero(input, TAIL_RESERVED_OFFSET, TAIL_RESERVED_BYTES)?;
        let value = Self {
            direction: ClaimsConservationDirectionV1::decode(read_byte(input, DIRECTION_OFFSET)?)?,
            realm: read_array(input, REALM_OFFSET)?,
            market: read_array(input, MARKET_OFFSET)?,
            release_set: read_array(input, RELEASE_SET_OFFSET)?,
            custody_context: read_array(input, CUSTODY_CONTEXT_OFFSET)?,
            aggregate: read_array(input, AGGREGATE_OFFSET)?,
            position: read_array(input, POSITION_OFFSET)?,
            owner: read_array(input, OWNER_OFFSET)?,
            external_collateral: read_array(input, EXTERNAL_COLLATERAL_OFFSET)?,
            hoard_vault: read_array(input, HOARD_VAULT_OFFSET)?,
            mint: read_array(input, MINT_OFFSET)?,
            token_program: read_array(input, TOKEN_PROGRAM_OFFSET)?,
            claims_program: read_array(input, CLAIMS_PROGRAM_OFFSET)?,
            product_record_digest: read_array(input, PRODUCT_RECORD_DIGEST_OFFSET)?,
            linked_basis_record_digest: read_array(input, LINKED_BASIS_RECORD_DIGEST_OFFSET)?,
            semantic_basis_id: read_array(input, SEMANTIC_BASIS_ID_OFFSET)?,
            generation: read_u64(input, GENERATION_OFFSET)?,
            quantity: read_u64(input, QUANTITY_OFFSET)?,
            basis_scale: read_u64(input, BASIS_SCALE_OFFSET)?,
            collateral_atoms: read_u64(input, COLLATERAL_ATOMS_OFFSET)?,
            expected_market_revision: read_u64(input, EXPECTED_MARKET_REVISION_OFFSET)?,
            expected_position_revision: read_u64(input, EXPECTED_POSITION_REVISION_OFFSET)?,
            expected_custody_revision: read_u64(input, EXPECTED_CUSTODY_REVISION_OFFSET)?,
            pre_external_amount: read_u64(input, PRE_EXTERNAL_AMOUNT_OFFSET)?,
            post_external_amount: read_u64(input, POST_EXTERNAL_AMOUNT_OFFSET)?,
            pre_hoard_amount: read_u64(input, PRE_HOARD_AMOUNT_OFFSET)?,
            post_hoard_amount: read_u64(input, POST_HOARD_AMOUNT_OFFSET)?,
            claim_count: read_u32(input, CLAIM_COUNT_OFFSET)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Encode the exact canonical bytes, refusing an invalid request.
    pub fn to_bytes(self) -> Result<[u8; CLAIMS_CONSERVATION_REQUEST_BYTES_V1]> {
        self.validate()?;
        let mut output = [0_u8; CLAIMS_CONSERVATION_REQUEST_BYTES_V1];
        put(
            &mut output,
            MAGIC_OFFSET,
            &CLAIMS_CONSERVATION_REQUEST_MAGIC_V1,
        )?;
        put(
            &mut output,
            VERSION_OFFSET,
            &CLAIMS_CONSERVATION_SCHEMA_VERSION_V1.to_le_bytes(),
        )?;
        put(&mut output, DIRECTION_OFFSET, &[self.direction.tag()])?;
        for (offset, value) in [
            (REALM_OFFSET, self.realm),
            (MARKET_OFFSET, self.market),
            (RELEASE_SET_OFFSET, self.release_set),
            (CUSTODY_CONTEXT_OFFSET, self.custody_context),
            (AGGREGATE_OFFSET, self.aggregate),
            (POSITION_OFFSET, self.position),
            (OWNER_OFFSET, self.owner),
            (EXTERNAL_COLLATERAL_OFFSET, self.external_collateral),
            (HOARD_VAULT_OFFSET, self.hoard_vault),
            (MINT_OFFSET, self.mint),
            (TOKEN_PROGRAM_OFFSET, self.token_program),
            (CLAIMS_PROGRAM_OFFSET, self.claims_program),
            (PRODUCT_RECORD_DIGEST_OFFSET, self.product_record_digest),
            (
                LINKED_BASIS_RECORD_DIGEST_OFFSET,
                self.linked_basis_record_digest,
            ),
            (SEMANTIC_BASIS_ID_OFFSET, self.semantic_basis_id),
        ] {
            put(&mut output, offset, &value)?;
        }
        for (offset, value) in [
            (GENERATION_OFFSET, self.generation),
            (QUANTITY_OFFSET, self.quantity),
            (BASIS_SCALE_OFFSET, self.basis_scale),
            (COLLATERAL_ATOMS_OFFSET, self.collateral_atoms),
            (
                EXPECTED_MARKET_REVISION_OFFSET,
                self.expected_market_revision,
            ),
            (
                EXPECTED_POSITION_REVISION_OFFSET,
                self.expected_position_revision,
            ),
            (
                EXPECTED_CUSTODY_REVISION_OFFSET,
                self.expected_custody_revision,
            ),
            (PRE_EXTERNAL_AMOUNT_OFFSET, self.pre_external_amount),
            (POST_EXTERNAL_AMOUNT_OFFSET, self.post_external_amount),
            (PRE_HOARD_AMOUNT_OFFSET, self.pre_hoard_amount),
            (POST_HOARD_AMOUNT_OFFSET, self.post_hoard_amount),
        ] {
            put(&mut output, offset, &value.to_le_bytes())?;
        }
        put(
            &mut output,
            CLAIM_COUNT_OFFSET,
            &self.claim_count.to_le_bytes(),
        )?;
        Ok(output)
    }

    /// Validate every internal relation the actor is held to.
    ///
    /// This is deliberately more than a shape check. The stated collateral, the
    /// stated external balances and the stated Hoard balances must all agree
    /// with `quantity * basis_scale` and with the direction, so a request whose
    /// poststate would not conserve is unconstructible rather than merely
    /// unexecutable.
    pub fn validate(self) -> Result<()> {
        for value in [
            self.realm,
            self.market,
            self.release_set,
            self.custody_context,
            self.aggregate,
            self.position,
            self.owner,
            self.external_collateral,
            self.hoard_vault,
            self.mint,
            self.token_program,
            self.claims_program,
            self.product_record_digest,
            self.linked_basis_record_digest,
            self.semantic_basis_id,
        ] {
            if value == [0; IDENTITY_BYTES] {
                return Err(Error::ZeroIdentity);
            }
        }
        if self.external_collateral == self.hoard_vault {
            return Err(Error::AliasedAccounts);
        }
        if self.claim_count == 0 {
            return Err(Error::InvalidQuantity);
        }
        let atoms = collateral_atoms_v1(self.quantity, self.basis_scale)?;
        if atoms != self.collateral_atoms {
            return Err(Error::CollateralMismatch);
        }
        if self.expected_custody_revision.checked_add(1).is_none() {
            return Err(Error::RevisionOverflow);
        }
        let (external_debited, hoard_credited) = match self.direction {
            ClaimsConservationDirectionV1::Split => (true, true),
            ClaimsConservationDirectionV1::Merge => (false, false),
        };
        let external_ok = if external_debited {
            self.pre_external_amount.checked_sub(atoms) == Some(self.post_external_amount)
        } else {
            self.pre_external_amount.checked_add(atoms) == Some(self.post_external_amount)
        };
        if !external_ok {
            return Err(Error::ExternalBalanceMismatch);
        }
        let hoard_ok = if hoard_credited {
            self.pre_hoard_amount.checked_add(atoms) == Some(self.post_hoard_amount)
        } else {
            self.pre_hoard_amount.checked_sub(atoms) == Some(self.post_hoard_amount)
        };
        if !hoard_ok {
            return Err(Error::HoardBalanceMismatch);
        }
        Ok(())
    }

    /// The Custody replay this act advances.
    ///
    /// # Why this reuses an existing coordinate rather than naming a new one
    ///
    /// The coordinate is
    /// `[b"dclutch:custody-replay:v1", market, release_set, role, context]`
    /// (`crates/dclutch-custody-contract/src/lib.rs:337-345`), and its meaning,
    /// read off the code rather than off a comment, is *the ordered cursor of
    /// one execution role's Custody effects against one Market's namespace*:
    /// `CustodyReplayV1::advance` binds `request.caller_role` and requires
    /// `request.expected_revision == self.next_revision` (`lib.rs:971-1025`),
    /// and the record carries no actor, no order, no nonce and no action tag
    /// (`lib.rs:924-930`).
    ///
    /// A split or a merge is a Claims-role Custody transfer against this
    /// Market's Hoard. That is the same act class the coordinate already names,
    /// and the Claims program already builds requests on it:
    /// `execute_terminal_custody_v3`
    /// (`programs/dclutch-claims-sbf/src/rational_terminal_v3.rs:340-391`) uses
    /// exactly `(market, release_set, Claims, custody_context)` for its
    /// HoardPrincipal-to-External payout, and `custody_replay_v1::process`
    /// (`.../custody_replay_v1.rs:181-219`) is the route that creates it.
    ///
    /// Naming a *second* Claims-role replay over the same Hoard would fork the
    /// serialization of one pool of principal. The Vault seeds deliberately omit
    /// the role while the replay seeds include it, and the contract states why:
    /// "A Market's Hoard is one principal pool; a replay is one caller's cursor
    /// over it" (`crates/dclutch-custody-contract/src/lib.rs:348-363`). Two
    /// cursors for one role would be two callers' claims to be that one caller.
    ///
    /// The alternative existing coordinate was considered and rejected as an
    /// ALIAS, not as a near miss: `[b"dclutch:rational-replay:v2",
    /// descriptor_id, actor]` is per-actor but carries no market, no release
    /// set and no action tag (`RationalReplayV2 { descriptor, actor, revision }`,
    /// `crates/dclutch-rational-representation-v2-contract/src/replay.rs:24-28`),
    /// so it does not name a Market's collateral at all.
    ///
    /// # The cost this decision accepts, stated rather than hidden
    ///
    /// Because the coordinate is per-Market-per-role and not per-actor, every
    /// split, merge and terminal payout in a Market contends on ONE strictly
    /// serial `u64`. For terminal payout that is a rare act; a public split and
    /// merge is not, so this route makes a per-Market serial counter the
    /// throughput bound of the Market. That is a liveness property, not a
    /// conservation hole, and it is the price of not forking the cursor. Lifting
    /// it means changing the Custody coordinate for every role at once, which is
    /// not this crate's decision to make.
    #[must_use]
    pub fn custody_replay_seeds(self) -> CustodyReplaySeedsV1 {
        CustodyReplaySeedsV1::new(
            self.market,
            self.release_set,
            CallerRoleV1::Claims,
            self.custody_context,
        )
    }

    /// The canonical HoardPrincipal vault this act moves collateral through.
    #[must_use]
    pub fn hoard_vault_seeds(self) -> CustodyVaultSeedsV1 {
        CustodyVaultSeedsV1::new(
            self.market,
            self.release_set,
            self.custody_context,
            CompartmentV1::HoardPrincipal,
        )
    }

    /// Build the exact Custody transfer this act couples to.
    ///
    /// `request_digest` is the SHA-256 of the complete canonical request bytes,
    /// computed by the adapter — this crate hashes nothing. It becomes Custody's
    /// `semantic.parent_request_digest`, which is how a Custody receipt is
    /// bound back to the one conservation request that authorized it.
    ///
    /// The returned request is passed through Custody's OWN validator before it
    /// is handed back. Today that call is REDUNDANT and known to be: every
    /// shape rule `CustodyRequestV1::validate` applies to a Transfer is already
    /// implied by [`ClaimsConservationRequestV1::validate`] plus the pairing
    /// this method constructs, and a mutation that deletes the call kills no
    /// test. It is kept because Custody's operation-shape table is not this
    /// crate's to assume constant, and
    /// `tests::custody_itself_refuses_the_shapes_this_contract_is_careful_not_to_build`
    /// pins the three rules the construction is threading so that a change on
    /// either side is a red test rather than a CPI-time surprise. The delegated
    /// successor's validator, by contrast, IS load-bearing here — see
    /// [`ClaimsConservationRequestV1::delegated_custody_request`].
    pub fn custody_request(self, request_digest: [u8; 32]) -> Result<CustodyRequestV1> {
        self.validate()?;
        if request_digest == [0; IDENTITY_BYTES] {
            return Err(Error::ZeroIdentity);
        }
        let resulting_revision = self
            .expected_custody_revision
            .checked_add(1)
            .ok_or(Error::RevisionOverflow)?;
        let split = matches!(self.direction, ClaimsConservationDirectionV1::Split);
        let (source, destination) = if split {
            (self.external_collateral, self.hoard_vault)
        } else {
            (self.hoard_vault, self.external_collateral)
        };
        // Exactly one side is Custody-owned, so exactly one vault context is
        // present; Custody's `validate` refuses any other pairing.
        let (source_vault_context, destination_vault_context) = if split {
            ([0; IDENTITY_BYTES], self.custody_context)
        } else {
            (self.custody_context, [0; IDENTITY_BYTES])
        };
        let (source_owner, destination_owner) = if split {
            (self.owner, [0; IDENTITY_BYTES])
        } else {
            ([0; IDENTITY_BYTES], self.owner)
        };
        let request = CustodyRequestV1 {
            operation: OperationV1::Transfer,
            caller_role: CallerRoleV1::Claims,
            source_compartment: self.direction.source_compartment(),
            destination_compartment: self.direction.destination_compartment(),
            release_set: self.release_set,
            market: self.market,
            realm: self.realm,
            context: self.custody_context,
            caller_program: self.claims_program,
            semantic: ContextV1 {
                candidate: [0; IDENTITY_BYTES],
                source_owner,
                destination_owner,
                order: [0; IDENTITY_BYTES],
                parent_request_digest: request_digest,
                order_nonce: 0,
                generation: self.generation,
                page_index: 0,
                execution_index: 0,
                transfer_index: 0,
            },
            source,
            destination,
            source_vault_context,
            destination_vault_context,
            mint: self.mint,
            token_program: self.token_program,
            payer: [0; IDENTITY_BYTES],
            rent_refund: [0; IDENTITY_BYTES],
            expected_revision: self.expected_custody_revision,
            resulting_revision,
            amount: self.collateral_atoms,
            rent_lamports: 0,
        };
        request.validate().map_err(|_| Error::CustodyShape)?;
        Ok(request)
    }

    /// Build the exact delegated-allowance successor a SPLIT must use.
    ///
    /// A split debits the actor's OWN external token account, and Custody's V1
    /// `Transfer` refuses an `External` source outright
    /// (`programs/dclutch-custody-sbf/src/lib.rs:1387-1391`, returning
    /// `CustodySbfError::Instruction`) precisely so that an apparently correct
    /// balance delta cannot leave hidden delegated spending authority behind.
    /// The distinct V2 wire is therefore not an optimization; it is the only
    /// admissible shape for a split, and this method is the only way to build
    /// one from a conservation request.
    ///
    /// Every allowance coordinate is DERIVED, never carried on the wire,
    /// because one split is one atomic total debit: the actor authorizes
    /// exactly `collateral_atoms` to the Custody transfer authority, the single
    /// transfer consumes all of it, and the delegation is fully revoked. Any
    /// residual allowance after a split would be exactly the hidden authority
    /// the V2 wire exists to forbid.
    ///
    /// `custody_transfer_authority` is the Custody-owned
    /// `[b"dclutch:custody-authority:v1", market, release_set]` PDA. This crate
    /// derives no addresses; the adapter supplies it and Custody re-derives and
    /// re-checks it (`custody-sbf/src/delegated.rs:71-80`).
    pub fn delegated_custody_request(
        self,
        request_digest: [u8; 32],
        custody_transfer_authority: [u8; 32],
    ) -> Result<DelegatedCustodyRequestV2> {
        if !matches!(self.direction, ClaimsConservationDirectionV1::Split) {
            return Err(Error::WrongDirectionWire);
        }
        if custody_transfer_authority == [0; IDENTITY_BYTES] {
            return Err(Error::ZeroIdentity);
        }
        let custody = self.custody_request(request_digest)?;
        let request = DelegatedCustodyRequestV2 {
            custody,
            starts_atomic_debit: true,
            terminal: true,
            delegate_before: custody_transfer_authority,
            delegate_after: [0; IDENTITY_BYTES],
            total_debit: self.collateral_atoms,
            allowance_before: self.collateral_atoms,
            allowance_after: 0,
        };
        request.validate().map_err(|_| Error::CustodyShape)?;
        Ok(request)
    }

    /// Write the uniform little-endian complete-set quantity vector.
    ///
    /// `output` must be exactly `claim_count * 8` bytes. Every coordinate
    /// receives the same `quantity`: that equality IS the complete set, and the
    /// economic kernels refuse a non-uniform vector for these actions
    /// (`crates/dclutch-economic-slice-kernel/src/lib.rs:663`, and
    /// `crates/dclutch-claims-svm/src/lib.rs:480-486` for the plan wire).
    pub fn write_uniform_quantities(self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let count = usize::try_from(self.claim_count).map_err(|_| Error::WidthMismatch)?;
        let width = count
            .checked_mul(SCALAR_BYTES)
            .ok_or(Error::WidthMismatch)?;
        if output.len() != width {
            return Err(Error::WidthMismatch);
        }
        let encoded = self.quantity.to_le_bytes();
        let mut index = 0_usize;
        while index < count {
            let offset = index
                .checked_mul(SCALAR_BYTES)
                .ok_or(Error::WidthMismatch)?;
            put(output, offset, &encoded)?;
            index = index.checked_add(1).ok_or(Error::WidthMismatch)?;
        }
        Ok(())
    }

    /// Decide this act's complete-set growth against the Market's carried cap.
    ///
    /// Only a split grows principal; a merge is admitted unconditionally here
    /// because it shrinks the outstanding set count. `outstanding_sets` is the
    /// aggregate's Hoard scalar before the act, in complete-set units, and
    /// `principal_cap_sets` is the Market's carried cap already projected into
    /// complete sets by decision 0013's one named rounding boundary.
    pub fn admit_capacity(self, outstanding_sets: u64, principal_cap_sets: u64) -> Result<()> {
        self.validate()?;
        if !matches!(self.direction, ClaimsConservationDirectionV1::Split) {
            return Ok(());
        }
        MarketPrincipalCapSetsV1::read(principal_cap_sets)
            .admit_growth(outstanding_sets, self.quantity)
            .map_err(|_| Error::PrincipalCapacity)
    }

    /// The Hoard scalar this act must leave behind, in complete-set units.
    ///
    /// This is the aggregate side of the same conservation the Custody transfer
    /// executes on the token side. An adapter that commits one without the
    /// other has produced the hole this crate exists to close.
    pub const fn conserved_hoard_sets_after(self, pre_hoard_sets: u64) -> Result<u64> {
        match self.direction {
            ClaimsConservationDirectionV1::Split => match pre_hoard_sets.checked_add(self.quantity)
            {
                Some(value) => Ok(value),
                None => Err(Error::CollateralOverflow),
            },
            ClaimsConservationDirectionV1::Merge => match pre_hoard_sets.checked_sub(self.quantity)
            {
                Some(value) => Ok(value),
                None => Err(Error::CollateralOverflow),
            },
        }
    }
}

// ------------------------------------------------------------------ decoding

fn read_byte(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    let slice = input.get(offset..end).ok_or(Error::InvalidLength)?;
    let mut output = [0_u8; N];
    output.copy_from_slice(slice);
    Ok(output)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array::<2>(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array::<4>(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array::<SCALAR_BYTES>(
        input, offset,
    )?))
}

fn require_zero(input: &[u8], offset: usize, len: usize) -> Result<()> {
    let end = offset.checked_add(len).ok_or(Error::InvalidLength)?;
    let slice = input.get(offset..end).ok_or(Error::InvalidLength)?;
    if slice.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonical);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::InvalidLength)?;
    let slot = output.get_mut(offset..end).ok_or(Error::InvalidLength)?;
    slot.copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests;
