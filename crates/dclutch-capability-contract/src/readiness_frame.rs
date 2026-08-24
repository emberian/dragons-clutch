//! SDK-free physical frames and state transitions for Market-opening readiness.
//!
//! Account ownership, account bytes, PDA derivation, manifest hashing, Clock
//! access, native balances, System CPI execution, and persistence remain
//! explicit adapter obligations.  This module admits no caller declaration of
//! readiness or funding allocation.

use dclutch_core_contract::{ContentId, MarketRoot, Phase};

use crate::{
    CapabilityManifestV1, FundingCustodyObservationV1, FundingStateV1,
    MARKET_OPENING_READINESS_PDA_DOMAIN, MarketOpeningReadinessV1,
    readiness_instruction::{AdvanceMarketOpeningReadinessV1, BeginMarketOpeningReadinessV1},
};

/// Exact width of a Solana-compatible public-key byte string.
pub const READINESS_PUBKEY_BYTES: usize = 32;
/// Exact Begin account count.
pub const BEGIN_MARKET_OPENING_READINESS_ACCOUNTS: usize = 7;
/// Exact Advance account count.
pub const ADVANCE_MARKET_OPENING_READINESS_ACCOUNTS: usize = 4;
/// Canonical System Program key bytes.
pub const READINESS_SYSTEM_PROGRAM_ID: [u8; READINESS_PUBKEY_BYTES] = [0; READINESS_PUBKEY_BYTES];
/// Canonical Rent sysvar key bytes.
pub const READINESS_RENT_SYSVAR_ID: [u8; READINESS_PUBKEY_BYTES] = [
    6, 167, 213, 23, 25, 44, 92, 81, 33, 140, 201, 76, 61, 74, 241, 127, 88, 218, 238, 8, 155, 161,
    253, 68, 227, 219, 217, 138, 0, 0, 0, 0,
];

/// Refusal from a readiness frame or pure readiness transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadinessFrameError {
    /// An ordinary account key was the all-zero sentinel.
    ZeroAccountKey,
    /// A frame role did not have its required signer, writable, or executable bit.
    InvalidAccountPrivilege,
    /// Required distinct accounts aliased the same key.
    AccountAlias,
    /// The supplied System Program account was not the canonical executable Program.
    InvalidSystemProgram,
    /// The supplied Rent account was not the canonical nonexecutable Rent sysvar.
    InvalidRentSysvar,
    /// A readiness transition requires the Market to remain in Founding.
    MarketNotFounding,
    /// The immutable Market rent beneficiary did not match the Begin payer.
    PayerIsNotMarketBeneficiary,
    /// The authenticated immutable RentCredit beneficiary did not match the Begin payer.
    RentCreditBeneficiaryMismatch,
    /// An authenticated Market root refused the replay child-count transition.
    MarketRoot(dclutch_core_contract::Error),
    /// A canonical capability, funding, or readiness transition refused.
    Capability(crate::Error),
}

/// Result alias for readiness frames and pure transitions.
pub type Result<T> = core::result::Result<T, ReadinessFrameError>;

/// One runtime account projection used by a readiness physical frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadinessAccountMetaV1 {
    /// Exact account key bytes.
    pub key: [u8; READINESS_PUBKEY_BYTES],
    /// Whether the runtime exposed signer privilege.
    pub is_signer: bool,
    /// Whether the runtime exposed writable privilege.
    pub is_writable: bool,
    /// Whether the runtime exposed executable privilege.
    pub is_executable: bool,
}

/// Exact ordered Begin account role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BeginMarketOpeningReadinessRoleV1 {
    /// Signer and payer; must equal the immutable Market rent beneficiary.
    SponsorPayer,
    /// Writable authenticated Founding Market.
    Market,
    /// Vacant writable PDA for the new direct readiness child.
    Readiness,
    /// Immutable manifest committed by the authenticated Market identity.
    Manifest,
    /// Pre-existing readonly permanent RentCredit of the sponsor beneficiary.
    SponsorRentCredit,
    /// Canonical executable System Program.
    SystemProgram,
    /// Canonical nonexecutable Rent sysvar.
    RentSysvar,
}

/// Exact ordered Advance account role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdvanceMarketOpeningReadinessRoleV1 {
    /// Readonly authenticated Founding Market.
    Market,
    /// Writable transient readiness PDA.
    Readiness,
    /// Immutable manifest committed by the authenticated Market identity.
    Manifest,
    /// Readonly funding-state account for precisely the next manifest entry.
    FundingState,
}

/// Validated exact physical Begin frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginMarketOpeningReadinessFrameV1 {
    accounts: [ReadinessAccountMetaV1; BEGIN_MARKET_OPENING_READINESS_ACCOUNTS],
}

impl BeginMarketOpeningReadinessFrameV1 {
    /// Validate Begin ordering, privileges, fixed identities, and aliases.
    pub fn new(
        accounts: [ReadinessAccountMetaV1; BEGIN_MARKET_OPENING_READINESS_ACCOUNTS],
    ) -> Result<Self> {
        let payer = accounts[0];
        let market = accounts[1];
        let readiness = accounts[2];
        let manifest = accounts[3];
        let rent_credit = accounts[4];
        let system = accounts[5];
        let rent = accounts[6];
        require_ordinary(payer)?;
        require_ordinary(market)?;
        require_ordinary(readiness)?;
        require_ordinary(manifest)?;
        require_ordinary(rent_credit)?;
        require_role(payer, true, true, false)?;
        require_role(market, false, true, false)?;
        require_role(readiness, false, true, false)?;
        require_role(manifest, false, false, false)?;
        require_role(rent_credit, false, false, false)?;
        require_system_program(system)?;
        require_rent_sysvar(rent)?;
        require_distinct(&accounts)?;
        Ok(Self { accounts })
    }

    /// Return exact validated ordered account projections.
    pub const fn accounts(
        self,
    ) -> [ReadinessAccountMetaV1; BEGIN_MARKET_OPENING_READINESS_ACCOUNTS] {
        self.accounts
    }

    /// Return the sponsor/payer projection.
    pub const fn sponsor_payer(self) -> ReadinessAccountMetaV1 {
        self.accounts[0]
    }

    /// Return the authenticated Market projection.
    pub const fn market(self) -> ReadinessAccountMetaV1 {
        self.accounts[1]
    }

    /// Return the readiness child projection.
    pub const fn readiness(self) -> ReadinessAccountMetaV1 {
        self.accounts[2]
    }

    /// Return the immutable manifest projection.
    pub const fn manifest(self) -> ReadinessAccountMetaV1 {
        self.accounts[3]
    }

    /// Return the pre-existing sponsor RentCredit projection.
    pub const fn sponsor_rent_credit(self) -> ReadinessAccountMetaV1 {
        self.accounts[4]
    }
}

/// Validated exact physical Advance frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdvanceMarketOpeningReadinessFrameV1 {
    accounts: [ReadinessAccountMetaV1; ADVANCE_MARKET_OPENING_READINESS_ACCOUNTS],
}

impl AdvanceMarketOpeningReadinessFrameV1 {
    /// Validate Advance ordering, privileges, and aliases.
    pub fn new(
        accounts: [ReadinessAccountMetaV1; ADVANCE_MARKET_OPENING_READINESS_ACCOUNTS],
    ) -> Result<Self> {
        let market = accounts[0];
        let readiness = accounts[1];
        let manifest = accounts[2];
        let funding = accounts[3];
        require_ordinary(market)?;
        require_ordinary(readiness)?;
        require_ordinary(manifest)?;
        require_ordinary(funding)?;
        require_role(market, false, false, false)?;
        require_role(readiness, false, true, false)?;
        require_role(manifest, false, false, false)?;
        require_role(funding, false, false, false)?;
        require_distinct(&accounts)?;
        Ok(Self { accounts })
    }

    /// Return exact validated ordered account projections.
    pub const fn accounts(
        self,
    ) -> [ReadinessAccountMetaV1; ADVANCE_MARKET_OPENING_READINESS_ACCOUNTS] {
        self.accounts
    }

    /// Return the authenticated Market projection.
    pub const fn market(self) -> ReadinessAccountMetaV1 {
        self.accounts[0]
    }

    /// Return the readiness child projection.
    pub const fn readiness(self) -> ReadinessAccountMetaV1 {
        self.accounts[1]
    }

    /// Return the immutable manifest projection.
    pub const fn manifest(self) -> ReadinessAccountMetaV1 {
        self.accounts[2]
    }

    /// Return the selected funding-state projection.
    pub const fn funding_state(self) -> ReadinessAccountMetaV1 {
        self.accounts[3]
    }
}

/// PDA-seed projection for one canonical readiness child.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketOpeningReadinessPdaSeedsV1 {
    market: [u8; READINESS_PUBKEY_BYTES],
    generation: u64,
}

impl MarketOpeningReadinessPdaSeedsV1 {
    /// Construct the exact non-SDK seed projection.
    pub fn new(market: [u8; READINESS_PUBKEY_BYTES], generation: u64) -> Result<Self> {
        if is_zero(&market) {
            return Err(ReadinessFrameError::ZeroAccountKey);
        }
        Ok(Self { market, generation })
    }

    /// Return the fixed canonical PDA domain.
    pub const fn domain(self) -> &'static [u8] {
        MARKET_OPENING_READINESS_PDA_DOMAIN
    }

    /// Return the authenticated Market key seed.
    pub const fn market(self) -> [u8; READINESS_PUBKEY_BYTES] {
        self.market
    }

    /// Return the little-endian Market-generation seed.
    pub const fn generation_le_bytes(self) -> [u8; 8] {
        self.generation.to_le_bytes()
    }
}

/// Adapter obligation to authenticate a canonical manifest content commitment.
///
/// The adapter must SHA-256 the exact [`CapabilityManifestV1::as_bytes`]
/// preimage, construct the protocol content identity under its selected hash
/// policy, and prove it equals [`Self::content_id`].  This type contains no
/// caller-supplied attestation and performs no hashing in the pure contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManifestContentCommitmentV1<'a> {
    content_id: ContentId,
    manifest: CapabilityManifestV1<'a>,
}

impl<'a> ManifestContentCommitmentV1<'a> {
    fn from_market(root: MarketRoot, manifest: CapabilityManifestV1<'a>) -> Self {
        Self {
            content_id: root.identity().capability_manifest_id(),
            manifest,
        }
    }

    /// Return the immutable content identity committed by the Market root.
    pub const fn content_id(self) -> ContentId {
        self.content_id
    }

    /// Return the exact canonical manifest preimage the adapter must hash.
    pub const fn manifest(self) -> CapabilityManifestV1<'a> {
        self.manifest
    }
}

/// Adapter-authenticated immutable beneficiary of a pre-existing RentCredit.
///
/// Before Begin, the adapter must decode the canonical `RentCreditV1`, verify
/// its program ownership and PDA derivation from its own persisted bump, and
/// use the decoded refund authority to construct this value.  This contract
/// intentionally does not duplicate the rent-credit account codec or create a
/// fallback direct-refund path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRentCreditBeneficiaryV1([u8; READINESS_PUBKEY_BYTES]);

impl AuthenticatedRentCreditBeneficiaryV1 {
    /// Construct from the authority decoded from a canonical pre-existing credit.
    pub fn new(authority: [u8; READINESS_PUBKEY_BYTES]) -> Result<Self> {
        if is_zero(&authority) {
            return Err(ReadinessFrameError::ZeroAccountKey);
        }
        Ok(Self(authority))
    }

    /// Return the decoded immutable RentCredit beneficiary authority.
    pub const fn authority(self) -> [u8; READINESS_PUBKEY_BYTES] {
        self.0
    }
}

/// Successful Begin state, ready for atomic SVM account creation and persistence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BeginMarketOpeningReadinessPlanV1<'a> {
    root: MarketRoot,
    readiness: MarketOpeningReadinessV1,
    readiness_pda_seeds: MarketOpeningReadinessPdaSeedsV1,
    manifest_commitment: ManifestContentCommitmentV1<'a>,
}

impl<'a> BeginMarketOpeningReadinessPlanV1<'a> {
    /// Return the Market root after registration of the direct readiness child.
    pub const fn root(self) -> MarketRoot {
        self.root
    }

    /// Return the readiness record to persist in the newly created child account.
    pub const fn readiness(self) -> MarketOpeningReadinessV1 {
        self.readiness
    }

    /// Return the exact readiness PDA derivation obligation.
    pub const fn readiness_pda_seeds(self) -> MarketOpeningReadinessPdaSeedsV1 {
        self.readiness_pda_seeds
    }

    /// Return the immutable manifest content-authentication obligation.
    pub const fn manifest_commitment(self) -> ManifestContentCommitmentV1<'a> {
        self.manifest_commitment
    }
}

/// Successful Advance state, ready for atomic persistence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdvanceMarketOpeningReadinessPlanV1<'a> {
    readiness: MarketOpeningReadinessV1,
    manifest_commitment: ManifestContentCommitmentV1<'a>,
}

/// Authenticated observations consumed by one permissionless Advance.
///
/// `current_slot` must be read by the adapter from the trusted Clock sysvar;
/// it is intentionally not an account-frame role or client wire field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdvanceMarketOpeningReadinessObservationV1<'a> {
    root: MarketRoot,
    readiness: MarketOpeningReadinessV1,
    manifest: CapabilityManifestV1<'a>,
    funding: FundingStateV1,
    custody: FundingCustodyObservationV1,
    current_slot: u64,
}

impl<'a> AdvanceMarketOpeningReadinessObservationV1<'a> {
    /// Collect adapter-authenticated Market, manifest, funding, and Clock facts.
    pub const fn new(
        root: MarketRoot,
        readiness: MarketOpeningReadinessV1,
        manifest: CapabilityManifestV1<'a>,
        funding: FundingStateV1,
        custody: FundingCustodyObservationV1,
        current_slot: u64,
    ) -> Self {
        Self {
            root,
            readiness,
            manifest,
            funding,
            custody,
            current_slot,
        }
    }
}

impl<'a> AdvanceMarketOpeningReadinessPlanV1<'a> {
    /// Return the readiness record to persist after exactly one accepted entry.
    pub const fn readiness(self) -> MarketOpeningReadinessV1 {
        self.readiness
    }

    /// Return whether the persisted record is derived Ready and sealed.
    pub const fn is_sealed_ready(self) -> bool {
        self.readiness.is_ready()
    }

    /// Return the immutable manifest content-authentication obligation.
    pub const fn manifest_commitment(self) -> ManifestContentCommitmentV1<'a> {
        self.manifest_commitment
    }
}

/// Plan an atomic Begin after authenticated account and Market observations.
///
/// The adapter must additionally prove the Market/readiness owners and vacant
/// readiness account state, derive `frame.readiness()` from the returned seed
/// projection, hash the exact manifest preimage, derive the sponsor's
/// pre-existing RentCredit, calculate current Rent, create the account with
/// System, and persist both returned values atomically.  No sponsor receives a
/// direct refund from this transition.
pub fn begin_market_opening_readiness<'a>(
    root: MarketRoot,
    instruction: BeginMarketOpeningReadinessV1,
    frame: BeginMarketOpeningReadinessFrameV1,
    manifest: CapabilityManifestV1<'a>,
    rent_credit_beneficiary: AuthenticatedRentCreditBeneficiaryV1,
) -> Result<BeginMarketOpeningReadinessPlanV1<'a>> {
    require_founding(root)?;
    let payer = frame.sponsor_payer().key;
    if root.rent_refund() != payer {
        return Err(ReadinessFrameError::PayerIsNotMarketBeneficiary);
    }
    if rent_credit_beneficiary.authority() != payer {
        return Err(ReadinessFrameError::RentCreditBeneficiaryMismatch);
    }
    let manifest_commitment = ManifestContentCommitmentV1::from_market(root, manifest);
    let mut next_root = root;
    next_root
        .register_child(
            instruction.generation(),
            instruction.expected_market_child_count(),
        )
        .map_err(ReadinessFrameError::MarketRoot)?;
    let readiness_pda_seeds =
        MarketOpeningReadinessPdaSeedsV1::new(frame.market().key, instruction.generation())?;
    let readiness = MarketOpeningReadinessV1::begin(
        frame.market().key,
        instruction.generation(),
        manifest_commitment.content_id(),
        manifest,
        payer,
    )
    .map_err(ReadinessFrameError::Capability)?;
    Ok(BeginMarketOpeningReadinessPlanV1 {
        root: next_root,
        readiness,
        readiness_pda_seeds,
        manifest_commitment,
    })
}

/// Plan an atomic permissionless Advance using one actual selected FundingState.
///
/// `current_slot` is an adapter observation from trusted `Clock::get`; it is
/// intentionally absent from both the account frame and instruction wire.
/// The adapter must prove every supplied account's owner/PDA/content identity,
/// hash the exact manifest preimage, read the FundingState bytes, observe the
/// program-owned state's lamports and any authenticated Realm token vault, and
/// persist the returned readiness record only if all checks and the transaction
/// itself succeed.
pub fn advance_market_opening_readiness<'a>(
    instruction: AdvanceMarketOpeningReadinessV1,
    frame: AdvanceMarketOpeningReadinessFrameV1,
    observation: AdvanceMarketOpeningReadinessObservationV1<'a>,
) -> Result<AdvanceMarketOpeningReadinessPlanV1<'a>> {
    require_founding(observation.root)?;
    let manifest_commitment =
        ManifestContentCommitmentV1::from_market(observation.root, observation.manifest);
    let mut next_readiness = observation.readiness;
    next_readiness
        .advance(
            frame.market().key,
            instruction.generation(),
            manifest_commitment.content_id(),
            observation.manifest,
            instruction.expected_entry_index(),
            observation.funding,
            observation.custody,
            observation.current_slot,
        )
        .map_err(ReadinessFrameError::Capability)?;
    Ok(AdvanceMarketOpeningReadinessPlanV1 {
        readiness: next_readiness,
        manifest_commitment,
    })
}

fn require_founding(root: MarketRoot) -> Result<()> {
    if root.phase() != Phase::Founding {
        return Err(ReadinessFrameError::MarketNotFounding);
    }
    Ok(())
}

fn require_ordinary(account: ReadinessAccountMetaV1) -> Result<()> {
    if is_zero(&account.key) {
        return Err(ReadinessFrameError::ZeroAccountKey);
    }
    Ok(())
}

fn require_role(
    account: ReadinessAccountMetaV1,
    signer: bool,
    writable: bool,
    executable: bool,
) -> Result<()> {
    if account.is_signer != signer
        || account.is_writable != writable
        || account.is_executable != executable
    {
        return Err(ReadinessFrameError::InvalidAccountPrivilege);
    }
    Ok(())
}

fn require_system_program(account: ReadinessAccountMetaV1) -> Result<()> {
    if account.key != READINESS_SYSTEM_PROGRAM_ID || !account.is_executable {
        return Err(ReadinessFrameError::InvalidSystemProgram);
    }
    if account.is_signer || account.is_writable {
        return Err(ReadinessFrameError::InvalidAccountPrivilege);
    }
    Ok(())
}

fn require_rent_sysvar(account: ReadinessAccountMetaV1) -> Result<()> {
    if account.key != READINESS_RENT_SYSVAR_ID || account.is_executable {
        return Err(ReadinessFrameError::InvalidRentSysvar);
    }
    if account.is_signer || account.is_writable {
        return Err(ReadinessFrameError::InvalidAccountPrivilege);
    }
    Ok(())
}

fn require_distinct<const N: usize>(accounts: &[ReadinessAccountMetaV1; N]) -> Result<()> {
    for (index, account) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other.key == account.key)
        {
            return Err(ReadinessFrameError::AccountAlias);
        }
    }
    Ok(())
}

fn is_zero(bytes: &[u8; READINESS_PUBKEY_BYTES]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
        FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };

    fn id(value: u8) -> ContentId {
        ContentId::new([value; READINESS_PUBKEY_BYTES]).expect("nonzero test content")
    }

    fn account(key: u8, signer: bool, writable: bool, executable: bool) -> ReadinessAccountMetaV1 {
        ReadinessAccountMetaV1 {
            key: [key; READINESS_PUBKEY_BYTES],
            is_signer: signer,
            is_writable: writable,
            is_executable: executable,
        }
    }

    fn begin_frame() -> BeginMarketOpeningReadinessFrameV1 {
        BeginMarketOpeningReadinessFrameV1::new([
            account(9, true, true, false),
            account(10, false, true, false),
            account(11, false, true, false),
            account(12, false, false, false),
            account(13, false, false, false),
            ReadinessAccountMetaV1 {
                key: READINESS_SYSTEM_PROGRAM_ID,
                is_signer: false,
                is_writable: false,
                is_executable: true,
            },
            ReadinessAccountMetaV1 {
                key: READINESS_RENT_SYSVAR_ID,
                is_signer: false,
                is_writable: false,
                is_executable: false,
            },
        ])
        .expect("valid Begin frame")
    }

    fn advance_frame() -> AdvanceMarketOpeningReadinessFrameV1 {
        AdvanceMarketOpeningReadinessFrameV1::new([
            account(10, false, false, false),
            account(11, false, true, false),
            account(12, false, false, false),
            account(14, false, false, false),
        ])
        .expect("valid Advance frame")
    }

    fn root() -> MarketRoot {
        let identity =
            dclutch_core_contract::MarketIdentity::new(id(1), id(2), id(3), id(4), id(5), 7);
        MarketRoot::founding(identity, [9; READINESS_PUBKEY_BYTES]).expect("founding root")
    }

    fn manifest<'a>(
        storage: &'a mut [u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES],
    ) -> CapabilityManifestV1<'a> {
        let quote = FundingQuoteV1::new(
            FundingAmountsV1::new(
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::native_lamports(5).expect("work lamports"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            )
            .expect("amounts"),
            None,
        )
        .expect("quote");
        let entry = CapabilityEntryV1::new(
            id(20),
            id(21),
            id(22),
            id(23),
            id(24),
            id(25),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .expect("entry");
        CapabilityManifestV1::encode_into(&[entry], storage).expect("manifest")
    }

    #[test]
    fn frames_refuse_aliases_and_privilege_escalation() {
        let mut aliased = begin_frame().accounts();
        aliased[2].key = aliased[1].key;
        assert_eq!(
            BeginMarketOpeningReadinessFrameV1::new(aliased),
            Err(ReadinessFrameError::AccountAlias)
        );

        let mut writable_funding = advance_frame().accounts();
        writable_funding[3].is_writable = true;
        assert_eq!(
            AdvanceMarketOpeningReadinessFrameV1::new(writable_funding),
            Err(ReadinessFrameError::InvalidAccountPrivilege)
        );
    }

    #[test]
    fn begin_binds_market_beneficiary_manifest_and_exact_child_replay() {
        let mut storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = manifest(&mut storage);
        let plan = begin_market_opening_readiness(
            root(),
            BeginMarketOpeningReadinessV1::new(7, 0),
            begin_frame(),
            manifest,
            AuthenticatedRentCreditBeneficiaryV1::new([9; READINESS_PUBKEY_BYTES])
                .expect("credit beneficiary"),
        )
        .expect("begin plan");
        assert_eq!(plan.root().outstanding_children(), 1);
        assert_eq!(plan.readiness().entry_count(), 1);
        assert_eq!(
            plan.readiness_pda_seeds().domain(),
            MARKET_OPENING_READINESS_PDA_DOMAIN
        );

        assert_eq!(
            begin_market_opening_readiness(
                root(),
                BeginMarketOpeningReadinessV1::new(7, 1),
                begin_frame(),
                manifest,
                AuthenticatedRentCreditBeneficiaryV1::new([9; READINESS_PUBKEY_BYTES])
                    .expect("credit beneficiary"),
            ),
            Err(ReadinessFrameError::MarketRoot(
                dclutch_core_contract::Error::ChildCountMismatch
            ))
        );

        let mut wrong_payer = begin_frame().accounts();
        wrong_payer[0].key = [8; READINESS_PUBKEY_BYTES];
        let wrong_frame = BeginMarketOpeningReadinessFrameV1::new(wrong_payer).expect("frame");
        assert_eq!(
            begin_market_opening_readiness(
                root(),
                BeginMarketOpeningReadinessV1::new(7, 0),
                wrong_frame,
                manifest,
                AuthenticatedRentCreditBeneficiaryV1::new([8; READINESS_PUBKEY_BYTES])
                    .expect("credit beneficiary"),
            ),
            Err(ReadinessFrameError::PayerIsNotMarketBeneficiary)
        );

        let mut open_root = root();
        open_root
            .transition_phase(7, Phase::Open)
            .expect("admitted test edge");
        assert_eq!(
            begin_market_opening_readiness(
                open_root,
                BeginMarketOpeningReadinessV1::new(7, 0),
                begin_frame(),
                manifest,
                AuthenticatedRentCreditBeneficiaryV1::new([9; READINESS_PUBKEY_BYTES])
                    .expect("credit beneficiary"),
            ),
            Err(ReadinessFrameError::MarketNotFounding)
        );
    }

    #[test]
    fn advance_requires_actual_capitalized_active_next_funding_and_seals_derived_ready() {
        let mut storage = [0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        let manifest = manifest(&mut storage);
        let begin = begin_market_opening_readiness(
            root(),
            BeginMarketOpeningReadinessV1::new(7, 0),
            begin_frame(),
            manifest,
            AuthenticatedRentCreditBeneficiaryV1::new([9; READINESS_PUBKEY_BYTES])
                .expect("credit beneficiary"),
        )
        .expect("begin plan");
        let custody = FundingCustodyObservationV1::native_only(105, 100).expect("custody");
        let mut funding = FundingStateV1::new(id(5), manifest, 0, custody).expect("funding");
        assert_eq!(
            advance_market_opening_readiness(
                AdvanceMarketOpeningReadinessV1::new(7, 0),
                advance_frame(),
                AdvanceMarketOpeningReadinessObservationV1::new(
                    begin.root(),
                    begin.readiness(),
                    manifest,
                    funding,
                    custody,
                    10,
                ),
            ),
            Err(ReadinessFrameError::Capability(
                crate::Error::FoundingCapabilityInactive
            ))
        );
        funding
            .activate(id(5), manifest, custody, 10)
            .expect("activate founding capability");
        assert_eq!(
            advance_market_opening_readiness(
                AdvanceMarketOpeningReadinessV1::new(7, 0),
                advance_frame(),
                AdvanceMarketOpeningReadinessObservationV1::new(
                    begin.root(),
                    begin.readiness(),
                    manifest,
                    funding,
                    FundingCustodyObservationV1::native_only(104, 100).expect("short custody"),
                    10,
                ),
            ),
            Err(ReadinessFrameError::Capability(
                crate::Error::PresentNativeLamportsMismatch
            ))
        );
        let plan = advance_market_opening_readiness(
            AdvanceMarketOpeningReadinessV1::new(7, 0),
            advance_frame(),
            AdvanceMarketOpeningReadinessObservationV1::new(
                begin.root(),
                begin.readiness(),
                manifest,
                funding,
                custody,
                10,
            ),
        )
        .expect("advance plan");
        assert!(plan.is_sealed_ready());
        assert_eq!(plan.readiness().next_entry_index(), 1);
    }
}
