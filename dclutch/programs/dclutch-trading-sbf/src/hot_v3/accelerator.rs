//! The read-only accelerator's view of one Hot invocation: the frame, the sealed
//! artifacts, the activation cache and the input bank, authenticated from the
//! accelerator's side so an evaluator never trusts what Trading handed it.

use super::*;

/// Descriptor artifact class exposed by one authenticated accelerator view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceleratorArtifactClassV4 {
    /// AccountProfile selected by CapabilityProgramV4.
    AccountProfile,
    /// RequestProfile selected by CapabilityProgramV4.
    RequestProfile,
    /// LifecycleV5 policy selected by CapabilityProgramV4.
    Lifecycle,
    /// ExecutionStrategy selected by CapabilityProgramV4.
    Strategy,
    /// Transition program selected by CapabilityProgramV4/Strategy.
    Transition,
    /// EffectV4 program selected by CapabilityProgramV4.
    Effect,
}

/// Public read-only facts authenticated for one admitted accelerator callback.
///
/// This is an ephemeral adapter view, not a persisted DTO and not write/CPI
/// authority. The complete family request remains owned by the current
/// top-level Hot instruction; the view owns that loaded instruction only so an
/// external accelerator can borrow its exact request slice after this helper
/// has rejoined the caller PDA, activation, records, Product, runtime digest,
/// and the accelerator request invocation-context digest.
pub struct AuthenticatedAcceleratorInvocationV4<'request, 'accounts, 'info> {
    request: AdmittedAcceleratorRequestV2<'request>,
    output_page: Option<&'accounts AccountInfo<'info>>,
    envelope: HotExecutionEnvelopeV3,
    hot_instruction: Vec<u8>,
    descriptor: Box<CapabilityProgramV4>,
    selected_action: u32,
    context: Box<AdmittedInvocationContextV3>,
    product_runtime: Box<AuthenticatedProductRuntimeV3<'accounts, 'info>>,
    claims_program: ContentId,
    custody_program: ContentId,
    input_bank: Vec<u8>,
    scalars: Vec<u64>,
    identities: Vec<[u8; 32]>,
    artifact_raw_accounts: [&'accounts AccountInfo<'info>; 6],
    runtime_accounts: &'accounts [AccountInfo<'info>],
}

/// Ephemeral proof that Trading authenticated one exact accelerator CPI caller.
///
/// The type and its fields are crate-private, and only the private caller-PDA
/// boundary below constructs it. It is neither wire data nor a reusable
/// deployment credential: it binds the current release set, Market, root, full
/// accelerator request digest, and exact observed Program/ProgramData metadata
/// on this stack. The strategy adapter may spend it only for an immutable
/// admitted-AOT deployment after independently rejoining the finalized
/// ArtifactRelease and every structural Loader fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AuthenticatedAcceleratorCallerV4 {
    pub(super) release_set: [u8; 32],
    pub(super) market: [u8; 32],
    pub(super) root: [u8; 32],
    pub(super) role_request_digest: [u8; 32],
    pub(super) artifact_release: [u8; 32],
    pub(super) accelerator_program: [u8; 32],
    pub(super) accelerator_programdata: [u8; 32],
    pub(super) deployment_slot: u64,
    pub(super) upgrade_authority: Option<[u8; 32]>,
}

impl AuthenticatedAcceleratorCallerV4 {
    /// Rejoin the token to the one already-authenticated Trading root.
    pub(crate) fn binds_context(self, context: TradingFamilyContextV1) -> bool {
        self.binds_context_parts(
            context.release_set().to_bytes(),
            context.market(),
            context.child_root_key(),
        )
    }

    pub(super) fn binds_context_parts(self, release_set: [u8; 32], market: [u8; 32], root: [u8; 32]) -> bool {
        self.release_set == release_set
            && self.market == market
            && self.root == root
            && self.role_request_digest != [0; 32]
    }

    /// Rejoin the token to one immutable finalized release and live deployment.
    pub(crate) fn binds_immutable_deployment(
        self,
        artifact_release: dclutch_registry::release_set::ArtifactReleaseIdV1,
        release: ArtifactReleaseV1,
        program: &AccountInfo<'_>,
        programdata: &AccountInfo<'_>,
    ) -> bool {
        release.upgrade_policy() == ArtifactUpgradePolicyV1::Immutable
            && release.upgrade_authority().is_none()
            && self.upgrade_authority.is_none()
            && self.artifact_release == artifact_release.to_bytes()
            && self.accelerator_program == program.key.to_bytes()
            && self.accelerator_program == release.program().to_bytes()
            && self.accelerator_programdata == programdata.key.to_bytes()
            && self.accelerator_programdata == release.programdata()
            && self.deployment_slot == release.deployment_slot()
    }
}

impl<'request, 'accounts, 'info> AuthenticatedAcceleratorInvocationV4<'request, 'accounts, 'info> {
    /// Exact canonical accelerator request supplied by Trading, under its
    /// Strategy record's own output transport.
    pub const fn request(&self) -> AdmittedAcceleratorRequestV2<'request> {
        self.request
    }

    /// The accelerator's own output page, `Some` only under `OutputPageV3`.
    ///
    /// Resolved once, at the coordinate `admitted_v3` names, so a callback does
    /// not index the frame a second time with its own arithmetic. What the
    /// callback still owes is the part this helper deliberately does NOT do:
    /// it is Trading authenticating an invocation, not Trading vouching for an
    /// account another program owns, so the owner, the width and the aliasing
    /// are the accelerator's own refusals in its own band.
    pub const fn output_page(&self) -> Option<&'accounts AccountInfo<'info>> {
        self.output_page
    }

    /// Exact authenticated common Hot envelope.
    pub const fn envelope(&self) -> HotExecutionEnvelopeV3 {
        self.envelope
    }

    /// Borrow the complete family request from the authenticated top-level instruction.
    pub fn family_request(&self) -> &[u8] {
        self.hot_instruction
            .get(dclutch_market::capability_program::hot_v3::HOT_FAMILY_REQUEST_OFFSET_V3..)
            .unwrap_or(&[])
    }

    /// Action selector returned by the authenticated CapabilityProgramSetV2.
    pub const fn selected_action(&self) -> u32 {
        self.selected_action
    }

    /// Exact hostile-decoded CapabilityProgramV4 descriptor.
    ///
    /// Read through the seal rather than out of an authenticated strategy: the
    /// strategy chain this view used to carry was 37,061 CU of re-derivation
    /// whose only surviving outputs are four context identities the caller now
    /// signs for, and the descriptor body is the one thing a family evaluator
    /// still asks this view for.
    pub const fn descriptor(&self) -> CapabilityProgramV4 {
        *self.descriptor
    }

    /// Complete invocation-context preimage whose digest is in AcceleratorRequestV2.
    pub const fn context(&self) -> AdmittedInvocationContextV3 {
        *self.context
    }

    /// Product-authenticated runtime facts.
    pub const fn product_runtime(&self) -> &AuthenticatedProductRuntimeV3<'accounts, 'info> {
        &self.product_runtime
    }

    /// Independently authenticated Product-linked basis record coordinate.
    pub const fn linked_basis_record(
        &self,
    ) -> dclutch_product::svm_reader::AuthenticatedRecordV2 {
        self.product_runtime.linked_basis_record
    }

    /// Current Registry-selected Claims program identity.
    pub const fn claims_program(&self) -> ContentId {
        self.claims_program
    }

    /// Current Registry-selected Custody program identity.
    pub const fn custody_program(&self) -> ContentId {
        self.custody_program
    }

    /// Exact complete pre-Transition register bank committed by the request.
    pub fn input_bank(&self) -> &[u8] {
        &self.input_bank
    }

    /// Scalar prefix decoded without narrowing from the complete input bank.
    pub fn scalars(&self) -> &[u64] {
        &self.scalars
    }

    /// Identity suffix decoded from the complete input bank.
    pub fn identities(&self) -> &[[u8; 32]] {
        &self.identities
    }

    /// Exact finalized raw account for one descriptor artifact class.
    pub const fn artifact_raw_account(
        &self,
        class: AcceleratorArtifactClassV4,
    ) -> &'accounts AccountInfo<'info> {
        let [
            account_profile,
            request_profile,
            lifecycle,
            strategy,
            transition,
            effect,
        ] = self.artifact_raw_accounts;
        match class {
            AcceleratorArtifactClassV4::AccountProfile => account_profile,
            AcceleratorArtifactClassV4::RequestProfile => request_profile,
            AcceleratorArtifactClassV4::Lifecycle => lifecycle,
            AcceleratorArtifactClassV4::Strategy => strategy,
            AcceleratorArtifactClassV4::Transition => transition,
            AcceleratorArtifactClassV4::Effect => effect,
        }
    }

    /// Expanded logical AccountInfo sequence, downgraded read-only for the callback.
    pub const fn runtime_accounts(&self) -> &'accounts [AccountInfo<'info>] {
        self.runtime_accounts
    }
}

/// Authenticate one external admitted-accelerator invocation without lending
/// mutation or child-CPI authority.
#[inline(never)]
pub fn authenticate_accelerator_invocation_v4<'request, 'accounts, 'info>(
    accelerator_program: &Pubkey,
    accounts: &'accounts [AccountInfo<'info>],
    request_bytes: &'request [u8],
) -> Result<Box<AuthenticatedAcceleratorInvocationV4<'request, 'accounts, 'info>>, ProgramError> {
    let request = AdmittedAcceleratorRequestV2::decode(request_bytes)
        .map_err(|_| TradingSbfError::AcceleratorFrame)?;
    let caller_authority = account(accounts, 0)?;
    let fixed = accounts
        .get(
            ADMITTED_ACCELERATOR_HOT_FIXED_START_V4
                ..ADMITTED_ACCELERATOR_HOT_FIXED_START_V4 + ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4,
        )
        .ok_or(TradingSbfError::AcceleratorFrame)?;
    let strategy_evidence = accounts
        .get(
            ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_START_V4
                ..ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_START_V4
                    + ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4,
        )
        .ok_or(TradingSbfError::AcceleratorFrame)?;
    // The runtime slice begins one account later under the output-page profile,
    // because the page is APPENDED to the frame the chunked profile already
    // had. Both coordinates come from `admitted_v3`, so this is a lookup and
    // not a second table.
    let runtime_start = admitted_runtime_accounts_start_v4(request.profile())
        .ok_or(TradingSbfError::AcceleratorFrame)?;
    let output_page = match request.profile() {
        AcceleratorTransportProfileV2::OutputPageV3 => Some(account(
            accounts,
            ADMITTED_ACCELERATOR_OUTPUT_PAGE_ACCOUNT_V4,
        )?),
        AcceleratorTransportProfileV2::ChunkedBankV2
        | AcceleratorTransportProfileV2::ShadowTranscriptV3 => None,
    };
    let runtime_accounts = accounts
        .get(runtime_start..)
        .ok_or(TradingSbfError::AcceleratorFrame)?;
    hot_cu_checkpoint!("acc-enter");
    let trading_program = account(fixed, HOT_TRADING_PROGRAM_ACCOUNT_V3)?;
    let frame = HotFrameV3::parse_accelerator_readonly(trading_program.key, fixed)?;
    let hot_instruction = authenticate_accelerator_top_level_v4(
        frame,
        strategy_evidence,
        caller_authority,
        output_page,
        request,
    )?;
    let (envelope, family_request) = HotExecutionEnvelopeV3::split_instruction(&hot_instruction)
        .map_err(|_| TradingSbfError::AcceleratorFrame)?;
    let accelerator_program_evidence = account(
        strategy_evidence,
        ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4
            .checked_sub(2)
            .ok_or(TradingSbfError::AcceleratorFrame)?,
    )?;
    let accelerator_programdata_evidence = account(
        strategy_evidence,
        ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4
            .checked_sub(1)
            .ok_or(TradingSbfError::AcceleratorFrame)?,
    )?;
    let artifact_release_digest = {
        let artifact_release = account(
            strategy_evidence,
            ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4
                .checked_sub(4)
                .ok_or(TradingSbfError::AcceleratorFrame)?,
        )?;
        let data = artifact_release
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Release)?;
        hash(&data).to_bytes()
    };
    let accelerator_programdata_metadata = {
        let data = accelerator_programdata_evidence
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Release)?;
        ProgramDataMetadataV3View::parse(&data).map_err(|_| TradingSbfError::Release)?
    };
    hot_cu_checkpoint!("acc-toplevel");
    let accelerator_caller = authenticate_accelerator_caller_authority_v4(
        frame.trading_program.key,
        caller_authority,
        envelope,
        frame.root.key,
        // The digest of the SIGNED top-level family request, taken here from
        // the bytes the instructions sysvar just proved are the top-level
        // instruction's -- not from the request's `invocation_context`, which
        // is the caller's statement of it. The two are required equal later,
        // by `require_admitted_bank_matches_frame_v3`; this conjunct must not
        // rest on the value it is meant to check.
        family_request_digest_v3(family_request).map_err(|_| TradingSbfError::AcceleratorFrame)?,
        request.caller_authority_index(),
        artifact_release_digest,
        accelerator_program,
        accelerator_program_evidence,
        accelerator_programdata_evidence,
        accelerator_programdata_metadata,
    )?;
    let root_prestate = {
        let data = frame
            .root
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Root)?;
        hash(&data).to_bytes()
    };
    if root_prestate != envelope.root_prestate_digest() {
        return Err(TradingSbfError::Root.into());
    }
    hot_cu_checkpoint!("acc-caller-authority");
    let (trading_receipt, claims_program, custody_program) =
        authenticate_accelerator_activation_v4(frame, envelope)?;
    let trading_semantic_release = trading_receipt.semantic_release_id().to_bytes();
    hot_cu_checkpoint!("acc-activation");
    let market = authenticate_market_boxed_v3(&frame, envelope)
        .map_err(|_| TradingSbfError::AcceleratorRelease)?;
    hot_cu_checkpoint!("acc-market");
    let root_data = frame
        .root
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Root)?;
    let family_context = TradingFamilyContextV1::authenticate_at(
        frame.trading_program.key,
        frame.root.key,
        frame.root.owner,
        &root_data,
        trading_receipt,
        envelope.bump_hints().root,
    )?;
    drop(root_data);
    if family_context.market() != envelope.market()
        || family_context.release_set().to_bytes() != envelope.release_set()
        || family_context.generation() != envelope.generation()
    {
        return Err(TradingSbfError::Root.into());
    }
    hot_cu_checkpoint!("acc-release-waist");
    // The caller's record bumps are read AHEAD of the witness's own decode,
    // because the Product walk below is the first thing on this route that
    // spends them -- and reading them cannot refuse. See
    // `accelerator_record_bump_hints_v4`.
    let record_bumps = accelerator_record_bump_hints_v4(request.witness());
    let product_runtime =
        authenticate_product_runtime_hinted_boxed_v3(&frame, &market, record_bumps)
            .map_err(|_| TradingSbfError::AcceleratorArtifact)?;
    hot_cu_checkpoint!("acc-product-runtime");

    // THE PRELUDE'S CHAIN IS NOT WALKED HERE ANY MORE; ITS OUTPUTS ARRIVE IN
    // THE REQUEST, AND EVERY ONE THIS PROGRAM CAN SOURCE INDEPENDENTLY IS
    // REJOINED BELOW.
    //
    // Every value in that chain is something TRADING COMPUTED, in the same
    // instruction, before it built this CPI, and the caller-authority PDA
    // authenticated forty lines above proves the composing program is the
    // Trading at this release set, market and root, executing the family
    // request the top level signed -- a program-derived address has no private
    // key, so only that program could have signed it. What it does NOT pin, as
    // of 2026-09-03, is `hash(request_bytes)`: the request carries the register
    // bank and a window-gated bank carries the slot, so an address seeded off
    // it was unsignable. The request bytes are therefore admitted because
    // Trading composed them, not because an off-chain caller restated them. So
    // the request is a channel that costs nothing to widen, and the repair is a
    // MOVE: `admitted_composition_v3` writes the complete
    // `AdmittedInvocationContextV3` preimage and the two AccountProfile-derived
    // geometry banks into the request tail, and this program reads them.
    //
    // WHAT IS NOT TAKEN ON THE CALLER'S WORD, because the accelerator exists to
    // be a second opinion on the EVALUATION and a mirror of its caller would be
    // worth nothing:
    //
    // - every EVALUATION INPUT still comes from an account this program reads.
    //   The Product graph above (`acc-product-runtime`, 39,217) supplies the
    //   payout scale, the outcome count and the semantic basis; the input
    //   register bank below is read out of the runtime accounts and bound to
    //   the request's own digest; the root prestate is hashed from the root.
    // - the RUNTIME SLICE is bound by the observation digest this program
    //   computes over those accounts' bytes, at `acc-observations`. That is the
    //   one thing `acc-toplevel` does not cover -- it binds the forty-eight
    //   fixed accounts and the evidence suffix to the top-level instruction,
    //   and the runtime slice is neither.
    // - the ACTIVATION stays unconditional, as it does in Claims: the seeds
    //   name a role, not a key, and which program holds Trading in this release
    //   set is the one fact a signature under that program cannot state.
    // - the FIVE ARTIFACT IDENTITIES come from the SEAL, which is a
    //   Trading-owned write-once persisted verdict, not a caller assertion.
    //
    // WHAT IS ESTABLISHED BY THE SIGNATURE ALONE, named so it can be argued
    // with: `strategy`, `certificate`, `admission`, `artifact_release`,
    // `lifecycle`, `tail_count`, the dynamic span widths and the representative
    // coordinates. The first two are also request header fields and are
    // compared against them; the rest have no second source on this side of the
    // boundary, and the debt they carry is written in
    // `docs/design/DEALER_PARTIAL_REMOVE_COMPUTE_2026_09_02.md`.
    let context = decode_accelerator_prelude_context_v4(request.witness())?;
    let selected_action = context.selected_action;
    // The caller-authority token is spent here rather than in the strategy
    // chain that used to hold it: it binds the release set, the Market and the
    // root it was derived for to the family context this program read out of
    // the root account.
    if !accelerator_caller.binds_context(family_context) {
        return Err(TradingSbfError::Release.into());
    }
    hot_cu_checkpoint!("acc-witness");

    // Decision 0005's seal, and now it is the JOINT rather than a shortcut
    // through one. Its key is (descriptor schema, descriptor digest, action,
    // Trading semantic release, Registry), its account sits at the PDA that key
    // derives, and its body names the six artifact rows with their exact
    // widths. The descriptor digest is `request.capability_program()`, which
    // the caller signed; the action is the witness's; the semantic release is
    // the activation's. So a request naming an action this Trading release
    // never sealed for this descriptor has NO SEAL ACCOUNT AT ALL, and the
    // manifest and program-set walk that used to derive the pair is a walk to
    // the same answer through twelve accounts.
    let seal_data = frame
        .capability_seal
        .try_borrow_data()
        .map_err(|_| TradingSbfError::AcceleratorArtifact)?;
    let seal = authenticate_capability_seal_v3(
        frame.trading_program.key,
        frame,
        PROGRAM_SCHEMA_ID_V4,
        request.capability_program().to_bytes(),
        selected_action,
        trading_semantic_release,
        &seal_data,
    )
    .map_err(|_| TradingSbfError::AcceleratorArtifact)?;
    hot_cu_checkpoint!("acc-seal");
    let descriptor_data = borrow_sealed_record(
        frame,
        seal,
        SealedRoleV1::Descriptor,
        frame.descriptor_raw,
        frame.descriptor_staging,
        PROGRAM_SCHEMA_ID_V4,
        request.capability_program().to_bytes(),
    )
    .map_err(|_| TradingSbfError::AcceleratorArtifact)?;
    let descriptor = decode_capability_program_boxed_v3(&descriptor_data)?;
    drop(descriptor_data);
    hot_cu_checkpoint!("acc-descriptor");

    require_accelerator_projection_bindings_v4(
        family_context.selection().config().to_bytes(),
        &market,
        &product_runtime,
    )?;
    drop(market);
    hot_cu_checkpoint!("acc-records");
    let input_bank = authenticate_accelerator_input_bank_v4(
        request,
        runtime_accounts,
        frame.trading_program.key,
    )?;
    let (scalars, identities) = decode_accelerator_register_bank_v4(request, &input_bank)?;
    hot_cu_checkpoint!("acc-input-bank");
    // The two AccountProfile-derived banks are read rather than re-derived, and
    // the whole rejoin runs in a callee's frame rather than this one.
    // `authenticate_accelerator_invocation_v4` had 192 bytes of headroom at
    // 3,904 of 4,096 before this change, and the first version of it inlined
    // the banks, the eight-argument observation digest and the join here: the
    // frame went to 5,248 and the SBF linker emitted thirty-four
    // frame-overwrite diagnostics. The two boxed callees this move deleted were
    // load-bearing as FRAMES, not only as code.
    authenticate_accelerator_witness_v4(AcceleratorWitnessJoinV4 {
        accelerator_program,
        root: frame.root.key,
        registry: frame.registry.key,
        trading_program: frame.trading_program.key,
        release_set: envelope.release_set(),
        market: envelope.market(),
        selected_config: family_context.selection().config(),
        descriptor: &descriptor,
        seal,
        product_runtime: &product_runtime,
        request,
        family_request,
        runtime_accounts,
        root_prestate,
        context: &context,
    })?;
    hot_cu_checkpoint!("acc-context");
    Ok(Box::new(AuthenticatedAcceleratorInvocationV4 {
        request,
        output_page,
        envelope,
        hot_instruction,
        descriptor,
        selected_action,
        context,
        product_runtime,
        claims_program,
        custody_program,
        input_bank,
        scalars,
        identities,
        artifact_raw_accounts: [
            frame.account_profile_raw,
            frame.request_profile_raw,
            frame.lifecycle_raw,
            frame.strategy_raw,
            frame.transition_raw,
            frame.effect_raw,
        ],
        runtime_accounts,
    }))
}

/// Everything one accelerator invocation rejoins its caller's witness against.
///
/// A struct rather than fourteen positional arguments because the whole point
/// of the function is that each field has an INDEPENDENT SOURCE on this side of
/// the CPI boundary, and a positional list makes it possible to pass the
/// witness's own copy of a value as the thing it is being checked against.
struct AcceleratorWitnessJoinV4<'a, 'accounts, 'info> {
    accelerator_program: &'a Pubkey,
    // The three frame coordinates and the two envelope identities, passed as
    // the values they are rather than as the two aggregates that hold them:
    // `HotFrameV3` is thirty-nine account references and
    // `HotExecutionEnvelopeV3` is another hundred-odd bytes, and staging both
    // in `authenticate_accelerator_invocation_v4`'s frame to call this is what
    // put it 64 bytes over its 4,096-byte allowance.
    root: &'a Pubkey,
    registry: &'a Pubkey,
    trading_program: &'a Pubkey,
    release_set: [u8; 32],
    market: [u8; 32],
    selected_config: ContentId,
    descriptor: &'a CapabilityProgramV4,
    seal: SealedDescriptorClosureV1<'a>,
    product_runtime: &'a AuthenticatedProductRuntimeV3<'accounts, 'info>,
    request: AdmittedAcceleratorRequestV2<'a>,
    family_request: &'a [u8],
    runtime_accounts: &'accounts [AccountInfo<'info>],
    root_prestate: [u8; 32],
    context: &'a AdmittedInvocationContextV3,
}

/// Build and take the common projection bindings in a callee's frame.
///
/// `CommonProjectionBindingsV3` is eight thirty-two-byte identities, and
/// building it in `authenticate_accelerator_invocation_v4` put 256 bytes of it
/// in a frame with 192 bytes of headroom.
#[inline(never)]
fn require_accelerator_projection_bindings_v4(
    selected_config: [u8; 32],
    market: &CoreState,
    product_runtime: &AuthenticatedProductRuntimeV3<'_, '_>,
) -> Result<(), ProgramError> {
    require_common_projection_bindings_v3(CommonProjectionBindingsV3 {
        selected_config,
        selected_product_record: market.identity.product_record.to_bytes(),
        authenticated_product_record: product_runtime
            .runtime
            .product_record
            .content_digest
            .to_bytes(),
        market_product: market.identity.product_id.to_bytes(),
        runtime_product: product_runtime.runtime.product_id.to_bytes(),
        product_semantic_basis: product_runtime.runtime.liability_basis_id.to_bytes(),
        authenticated_semantic_basis: product_runtime.semantic_basis_id.to_bytes(),
        authenticated_linked_basis: product_runtime
            .linked_basis_record
            .content_digest
            .to_bytes(),
    })
}

/// Read the caller's eight Product graph record bumps, or none at all.
///
/// TOTAL, AND THAT IS THE POINT. These are search hints: each is fed to a
/// `create_program_address` over seeds this program derives for itself, and
/// the address it produces is compared against the account the frame supplied,
/// by the equality that was always there. So an unreadable witness is not an
/// accusation HERE -- it yields the absent bank, the walk searches exactly as
/// it did before the bank existed, and the witness's own decode below still
/// refuses by name for every field it owns.
///
/// Making it fallible instead moved the accelerator's frontier marker from
/// `AcceleratorArtifact` back to `AcceleratorRuntimeView`, measured
/// 2026-09-03: a hint reader had become the first thing on the route that
/// could refuse, and it reported a conjunct it does not own.
#[inline(never)]
fn accelerator_record_bump_hints_v4(witness: &[u8]) -> ProductRecordBumpsV3 {
    AdmittedPreludeWitnessV1::decode(witness)
        .map(|witness| ProductRecordBumpsV3(witness.record_bumps()))
        .unwrap_or(ProductRecordBumpsV3::ABSENT)
}

/// Decode the caller's prelude witness into its boxed context preimage.
///
/// Boxed and `#[inline(never)]` for the frame, not for the code: 756 bytes of
/// context is a fifth of the caller's whole stack allowance.
#[inline(never)]
fn decode_accelerator_prelude_context_v4(
    witness: &[u8],
) -> Result<Box<AdmittedInvocationContextV3>, ProgramError> {
    Ok(Box::new(
        AdmittedPreludeWitnessV1::decode(witness)
            .map_err(|_| TradingSbfError::AcceleratorRuntimeView)?
            .context(),
    ))
}

/// Refuse unless every witness field with a second source agrees with it.
///
/// THIS IS THE HOSTILE'S TARGET, and it is why the widened request is a repair
/// and not a relaxation: a tampered field in the witness must refuse by name
/// rather than be believed. Five groups, each with its own accusation:
///
/// - `AcceleratorRuntimeView` for the request's own header and the three
///   digests this program takes over bytes it read itself;
/// - `AcceleratorRelease` for the six coordinates this frame names;
/// - `AcceleratorArtifact` for the four artifact identities the seal fixes, the
///   descriptor's derivation policy, and the Product graph this program
///   authenticated.
///
/// The context digest is compared LAST-BUT-FIRST on purpose: it is checked at
/// the top so a witness whose preimage does not reproduce the digest the
/// request header carries is refused before any field of it is read as if it
/// meant something. The header itself is admitted because the caller authority
/// at account 0 is a PDA of the composing Trading program and signed this CPI,
/// not because an off-chain producer restated the request bytes: since
/// 2026-09-03 the authority's seed is the SIGNED FAMILY REQUEST and the chunk
/// ordinal, so that the address does not move with the executing slot.
#[inline(never)]
fn authenticate_accelerator_witness_v4(
    join: AcceleratorWitnessJoinV4<'_, '_, '_>,
) -> Result<(), ProgramError> {
    let context = join.context;
    let request = join.request;
    if admitted_invocation_context_digest_v3(*context)
        .map_err(|_| TradingSbfError::AcceleratorRuntimeView)?
        != request.invocation_context()
        || context.capability_program != request.capability_program()
        || context.strategy != request.strategy_program()
        || context.certificate != request.certificate_program()
        || context.tail_count != request.tail_count()
        || context.scalar_count != request.scalar_count()
        || context.identity_count != request.identity_count()
        || usize::try_from(context.account_count)
            .map_err(|_| TradingSbfError::AcceleratorRuntimeView)?
            != join.runtime_accounts.len()
    {
        return Err(TradingSbfError::AcceleratorRuntimeView.into());
    }
    if context.release_set.to_bytes() != join.release_set
        || context.market.to_bytes() != join.market
        || context.root.to_bytes() != join.root.to_bytes()
        || context.registry_program.to_bytes() != join.registry.to_bytes()
        || context.trading_program.to_bytes() != join.trading_program.to_bytes()
        || context.accelerator_program.to_bytes() != join.accelerator_program.to_bytes()
        || context.config != join.selected_config
    {
        return Err(TradingSbfError::AcceleratorRelease.into());
    }
    for (role, identity) in [
        (SealedRoleV1::AccountProfile, context.account_profile),
        (SealedRoleV1::RequestProfile, context.request_profile),
        (SealedRoleV1::TransitionProgram, context.transition),
        (SealedRoleV1::EffectProgram, context.effect),
    ] {
        if join
            .seal
            .row(role)
            .map_err(|_| TradingSbfError::AcceleratorArtifact)?
            .content_digest()
            != identity.to_bytes()
        {
            return Err(TradingSbfError::AcceleratorArtifact.into());
        }
    }
    if context.lifecycle != join.descriptor.derivation_policy()
        || context.product.to_bytes()
            != join
                .product_runtime
                .runtime
                .product_record
                .content_digest
                .to_bytes()
        || context.portfolio.to_bytes()
            != join
                .product_runtime
                .runtime
                .portfolio_record
                .content_digest
                .to_bytes()
        || context.linked_basis.to_bytes()
            != join
                .product_runtime
                .linked_basis_record
                .content_digest
                .to_bytes()
    {
        return Err(TradingSbfError::AcceleratorArtifact.into());
    }
    if context.root_prestate_digest.to_bytes() != join.root_prestate
        || context.family_request_digest
            != family_request_digest_v3(join.family_request)
                .map_err(|_| TradingSbfError::AcceleratorRuntimeView)?
    {
        return Err(TradingSbfError::AcceleratorRuntimeView.into());
    }
    // The banks are read here, where the observation digest that consumes the
    // representative map is also taken. A witness naming a coordinate outside
    // this frame is refused by the decoder before it can be indexed with.
    let witness = AdmittedPreludeWitnessV1::decode(request.witness())
        .map_err(|_| TradingSbfError::AcceleratorRuntimeView)?;
    // The witness carries no span bank: every family derives its span widths
    // on its own side of the boundary, and `AdmittedPreludeWitnessV1` keeps the
    // header word as a canonical zero.
    let mut representatives = Vec::with_capacity(join.runtime_accounts.len());
    for index in 0..join.runtime_accounts.len() {
        representatives.push(
            witness
                .representative(index)
                .map_err(|_| TradingSbfError::AcceleratorRuntimeView)?,
        );
    }
    hot_cu_checkpoint!("acc-artifacts");
    // THE RUNTIME SLICE'S OWN BINDING, and the one thing `acc-toplevel` does
    // not cover. It is taken over the bytes THIS program reads out of the
    // accounts it was handed, and compared against the digest the caller
    // signed for.
    let runtime = join.product_runtime;
    if context.runtime_observations_digest
        != accelerator_runtime_observations_digest_v4(
            join.runtime_accounts,
            &representatives,
            request,
            join.trading_program,
            join.selected_config.to_bytes(),
            runtime.runtime.product_record.content_digest.to_bytes(),
            runtime.runtime.portfolio_record.content_digest.to_bytes(),
            runtime.linked_basis_record.content_digest.to_bytes(),
        )?
    {
        return Err(TradingSbfError::AcceleratorRuntimeView.into());
    }
    Ok(())
}

/// Bind the accelerator's read-only frame to the top-level Hot instruction.
///
/// Every one of the [`HOT_FIXED_ACCOUNT_COUNT_V3`] common fixed accounts, the
/// eight admitted-AOT strategy evidence accounts and this chunk's caller
/// authority must be the account the top-level instruction named, at the
/// position it named it, with the privileges it named -- so an accelerator
/// cannot be handed a frame that differs from the one the Trading invocation
/// was authorized against.
///
/// # The capability seal is bound here by address, and its body elsewhere
///
/// `frame.capability_seal` is compared against its meta below, the same as the
/// other thirty-eight, and that is all this function does with it. It is
/// present in the comparison because the accelerator carries the common Hot
/// fixed frame ENTIRE, and "entire" is exactly what the comparison below has to
/// mean -- the array it is compared through held thirty-eight entries against
/// thirty-nine metas once, and this account was the one silently skipped.
pub(super) fn authenticate_accelerator_top_level_v4(
    frame: HotFrameV3<'_, '_>,
    strategy_evidence: &[AccountInfo<'_>],
    caller_authority: &AccountInfo<'_>,
    output_page: Option<&AccountInfo<'_>>,
    request: AdmittedAcceleratorRequestV2<'_>,
) -> Result<Vec<u8>, ProgramError> {
    let (current_index, sysvar) = borrow_authenticated_instructions_v1(frame.instructions)?;
    let observed = SysvarInstructionV1::read(current_index, &sysvar)?;
    let (hot_instruction, fixed_start, strategy_start, caller_start, registry_mode) = if observed
        .program_id()
        == frame.trading_program.key.as_array()
    {
        (
            observed.data().to_vec(),
            0_usize,
            HOT_FIXED_ACCOUNT_COUNT_V3,
            HOT_FIXED_ACCOUNT_COUNT_V3
                .checked_add(ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4)
                .ok_or(TradingSbfError::Content)?,
            false,
        )
    } else if observed.program_id() == frame.registry.key.as_array() {
        let (envelope, _) = HotExecutionEnvelopeV3::split_instruction(observed.data())
            .map_err(|_| TradingSbfError::NativeSignature)?;
        let activation_digest = {
            let data = frame
                .activation_cache
                .try_borrow_data()
                .map_err(|_| TradingSbfError::NativeSignature)?;
            ContentId::new(hash(&data).to_bytes()).map_err(|_| TradingSbfError::NativeSignature)?
        };
        let continuation = RegistryContinuationRequestV1::new_core_trading_hot(
            ContentId::new(envelope.release_set()).map_err(|_| TradingSbfError::NativeSignature)?,
            activation_digest,
            ContentId::new(hash(observed.data()).to_bytes())
                .map_err(|_| TradingSbfError::NativeSignature)?,
            u32::try_from(observed.data().len()).map_err(|_| TradingSbfError::NativeSignature)?,
        )
        .map_err(|_| TradingSbfError::NativeSignature)?;
        // `metas_range` REQUIRES all six outer-prefix metas to exist; the
        // explicit `.take(5)` below then compares the five this frame can name.
        // The sixth is the Registry continuation admission, which the read-only
        // accelerator frame does not carry, so there is nothing here to compare
        // it against -- `authenticate_hot_invocation_v3` binds it, against the
        // admission account it derived. Deliberate, and spelled `.take(...)`
        // rather than left to `zip`'s truncation, which is the shape that hid
        // the missing capability-seal comparison in this same function.
        let outer = observed.metas_range(0, REGISTRY_CONTINUATION_OUTER_PREFIX_ACCOUNTS_V1)?;
        let expected_outer = [
            frame.activation_cache.key,
            frame.core_program.key,
            frame.core_programdata.key,
            frame.trading_program.key,
            frame.trading_programdata.key,
        ];
        if outer
            .iter()
            .take(expected_outer.len())
            .zip(expected_outer)
            .any(|(meta, key)| meta.pubkey != key.as_array() || meta.is_signer || meta.is_writable)
        {
            return Err(TradingSbfError::NativeSignature.into());
        }
        let batch = continuation
            .role_batch_request()
            .map_err(|_| TradingSbfError::NativeSignature)?;
        let batch_digest = ContentId::new(hash(&batch.to_bytes()).to_bytes())
            .map_err(|_| TradingSbfError::NativeSignature)?;
        let seeds = RegistryContinuationAdmissionSeedsV1::new(
            continuation,
            frame.activation_cache.key.to_bytes(),
            batch_digest,
        )
        .map_err(|_| TradingSbfError::NativeSignature)?;
        let release = seeds.release_set();
        let cache = seeds.activation_cache();
        let batch = seeds.batch_request_digest();
        let mask = seeds.role_mask();
        let role = seeds.continuation_role();
        let digest = seeds.continuation_digest();
        let expected_admission = Pubkey::find_program_address(
            &[
                seeds.domain(),
                release.as_slice(),
                cache.as_slice(),
                batch.as_slice(),
                mask.as_slice(),
                role.as_slice(),
                digest.as_slice(),
            ],
            frame.registry.key,
        )
        .0;
        let admission_meta = outer.get(5).ok_or(TradingSbfError::NativeSignature)?;
        if admission_meta.pubkey != expected_admission.as_array()
            || admission_meta.is_signer
            || admission_meta.is_writable
        {
            return Err(TradingSbfError::NativeSignature.into());
        }
        let fixed_start = REGISTRY_CONTINUATION_OUTER_PREFIX_ACCOUNTS_V1;
        let strategy_start = fixed_start
            .checked_add(HOT_FIXED_ACCOUNT_COUNT_V3)
            .and_then(|value| value.checked_add(1))
            .ok_or(TradingSbfError::Content)?;
        let caller_start = strategy_start
            .checked_add(ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4)
            .ok_or(TradingSbfError::Content)?;
        (
            observed.data().to_vec(),
            fixed_start,
            strategy_start,
            caller_start,
            true,
        )
    } else {
        return Err(TradingSbfError::NativeSignature.into());
    };
    let (envelope, _) = HotExecutionEnvelopeV3::split_instruction(&hot_instruction)
        .map_err(|_| TradingSbfError::NativeSignature)?;
    if strategy_evidence.len() != ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4 {
        return Err(TradingSbfError::Content.into());
    }
    let fixed_metas = observed.metas_range(fixed_start, HOT_FIXED_ACCOUNT_COUNT_V3)?;
    // Typed at the contract's own width, and NOT a bare array literal, because
    // the loop below is a `zip` and `zip` truncates to the shorter side without
    // a diagnostic. This array held thirty-eight entries against thirty-nine
    // metas: `frame.capability_seal` was absent, so the meta at
    // `HOT_CAPABILITY_SEAL_ACCOUNT_V3` was silently never compared, and every
    // future account appended to the common fixed frame would have been skipped
    // the same silent way. `ADMITTED_ACCELERATOR_HOT_FIXED_COUNT_V4`'s note
    // records the 38 -> 39 drift that already closed this path from both ends
    // once; the annotation is what makes the third occurrence a compile error
    // instead of a fourth authentication hole.
    let fixed_accounts: [&AccountInfo<'_>; HOT_FIXED_ACCOUNT_COUNT_V3] = [
        frame.market,
        frame.root,
        frame.manifest_raw,
        frame.manifest_staging,
        frame.program_set_raw,
        frame.program_set_staging,
        frame.descriptor_raw,
        frame.descriptor_staging,
        frame.config_raw,
        frame.config_staging,
        frame.account_profile_raw,
        frame.account_profile_staging,
        frame.request_profile_raw,
        frame.request_profile_staging,
        frame.transition_raw,
        frame.transition_staging,
        frame.effect_raw,
        frame.effect_staging,
        frame.lifecycle_raw,
        frame.lifecycle_staging,
        frame.strategy_raw,
        frame.strategy_staging,
        frame.activation_cache,
        frame.core_program,
        frame.core_programdata,
        frame.trading_program,
        frame.trading_programdata,
        frame.registry,
        frame.rent,
        frame.instructions,
        frame.product_raw,
        frame.product_staging,
        frame.result_domain_raw,
        frame.result_domain_staging,
        frame.portfolio_raw,
        frame.portfolio_staging,
        frame.linked_basis_raw,
        frame.linked_basis_staging,
        frame.capability_seal,
    ];
    for (index, (meta, info)) in fixed_metas.iter().zip(fixed_accounts).enumerate() {
        let expected_writable =
            index == HOT_ROOT_ACCOUNT_V3 || (registry_mode && index == HOT_MARKET_ACCOUNT_V3);
        if meta.pubkey != info.key.as_array()
            || meta.is_signer
            || meta.is_writable != expected_writable
        {
            return Err(TradingSbfError::NativeSignature.into());
        }
    }
    let strategy_metas = observed.metas_range(
        strategy_start,
        ADMITTED_ACCELERATOR_STRATEGY_EVIDENCE_COUNT_V4,
    )?;
    if strategy_metas
        .iter()
        .zip(strategy_evidence)
        .any(|(meta, info)| {
            meta.pubkey != info.key.as_array() || meta.is_signer || meta.is_writable
        })
    {
        return Err(TradingSbfError::NativeSignature.into());
    }
    let caller_count = admitted_caller_authority_count_v3(
        request.profile(),
        request.scalar_count(),
        request.identity_count(),
    )?;
    if caller_count
        != usize::try_from(
            request
                .caller_authority_count()
                .map_err(|_| TradingSbfError::Content)?,
        )
        .map_err(|_| TradingSbfError::Content)?
    {
        return Err(TradingSbfError::Content.into());
    }
    let caller_index = caller_start
        .checked_add(
            usize::try_from(request.caller_authority_index())
                .map_err(|_| TradingSbfError::Content)?,
        )
        .ok_or(TradingSbfError::Content)?;
    let caller_meta = observed
        .metas()
        .get(caller_index)
        .ok_or(TradingSbfError::NativeSignature)?;
    let page_start = caller_start
        .checked_add(caller_count)
        .ok_or(TradingSbfError::Content)?;
    if caller_meta.pubkey != caller_authority.key.as_array()
        || caller_meta.is_signer
        || caller_meta.is_writable
        || page_start > observed.account_count()
        || envelope.market() == [0; 32]
    {
        return Err(TradingSbfError::NativeSignature.into());
    }
    // THE TOP-LEVEL FRAME AND THE CPI FRAME ARE TWO SHAPES, joined here. This
    // helper exists because an accelerator that trusts the CPI frame it was
    // handed trusts the caller to have built it; under the output-page profile
    // that frame carries a WRITABLE account, so the one meta that grants write
    // authority is exactly the one worth re-reading out of the runtime's own
    // serialization of Trading's top-level instruction. It must be the page
    // this accelerator was handed, writable and unsigned, immediately after the
    // caller-authority span -- which is where `ADMITTED_OUTPUT_PAGE_ACCOUNT_V3`
    // puts it in the CPI frame.
    if let Some(page) = output_page {
        let page_meta = observed
            .metas()
            .get(page_start)
            .ok_or(TradingSbfError::NativeSignature)?;
        if page_meta.pubkey != page.key.as_array() || page_meta.is_signer || !page_meta.is_writable
        {
            return Err(TradingSbfError::NativeSignature.into());
        }
    }
    Ok(hot_instruction)
}

/// Proof that this execution has already held the activation cache account to
/// its address, its owner, and its privileges.
///
/// Only [`require_activation_cache_account_v3`] produces one, so a reader of
/// the cache either takes the check or takes this — there is no third way to
/// get at the bytes, and "someone earlier already checked it" stops being a
/// claim in a comment that a later edit can quietly falsify.
///
/// # Why the witness rather than just calling the check again
///
/// The check contains a `find_program_address`, and that search costs 1,500 CU
/// per attempt it has to make. Its seeds are `[domain, release_set]` under the
/// Registry, and the release set is a property of the DEPLOYED ELVES, not of
/// the caller — so every execution against one deployment pays the same depth,
/// and repeating the search repeats that cost exactly.
///
/// # This is now the CONTINUATION arm's device only
///
/// Decision 0017's option B moved the top-level arm onto
/// `dclutch-registry::activation_auth_v1`'s
/// `authenticate_activation_cache_identity_v1`, which is the same conjunction
/// written once and shared with the Registry's own `Reauthenticate` handler. So
/// the "no third way" rule still holds for both arms, with two producers rather
/// than one: the continuation takes the inlined check below, the top level takes
/// the crate's. [`require_activation_cache_account_v3`] says why the continuation
/// cannot simply take the crate's too, and it is a measured reason, not a
/// preference.
#[derive(Clone, Copy)]
pub(super) struct ActivationCacheAuthenticatedV1(());

/// Hold the activation cache account to its address, its owner, and its
/// privileges, before anything reads a byte out of it.
///
/// Shared by every reader of the cache so the checks cannot drift apart: the
/// account must be the Registry-owned PDA for THIS market's release set, and
/// must arrive neither signing nor writable nor executable.
///
/// # Why this is a SECOND spelling of a conjunction the crate owns, and stays
///
/// `dclutch_registry::activation_auth_v1::authenticate_activation_cache_identity_v1`
/// checks exactly this -- Registry ownership, non-executability, the one exact
/// width, and the address reproduced from the body's carried bump -- and the
/// top-level arm takes it there since decision 0017's option B. This copy
/// survives because it is the CONTINUATION's, and the continuation is the route
/// with no compute to spare: the measurement above is what happens when this
/// stops being inlined at its one call site, and an out-of-line call into
/// another crate cannot be inlined at all across the SBF codegen boundary.
///
/// That makes this a real drift seam and it should be named as one rather than
/// left to be discovered. What holds it closed is that both arms authenticate
/// the SAME account under the same rule and any divergence shows up as a route
/// that admits a cache the other refuses; the tripwire in
/// `tests/registry_hot_continuation.rs` and the top-level cases in
/// `tests/direct_hot_top_level.rs` exercise both. The clean fix is for the
/// continuation to afford the crate call, and that is a compute problem, not a
/// design one.
#[inline(always)]
pub(super) fn require_activation_cache_account_v3(
    frame: HotFrameV3<'_, '_>,
    release_set: [u8; 32],
) -> Result<ActivationCacheAuthenticatedV1, ProgramError> {
    if frame.activation_cache.owner != frame.registry.key
        || frame.activation_cache.is_signer
        || frame.activation_cache.is_writable
        || frame.activation_cache.executable
        || frame.activation_cache.data_len() != ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
    {
        return Err(TradingSbfError::Release.into());
    }
    // The cache carries its own bump, so this REPRODUCES the address instead of
    // walking down from 255. Only the seed the account contributes is that one
    // byte; the release set is still the envelope's, so the address this checks
    // against is still the one THIS market selected and not one the account
    // named for itself. A wrong byte reproduces a different address and refuses
    // at the equality below, and only the Registry writes the byte -- see
    // `ACTIVATION_CACHE_BUMP_OFFSET_V1`. Zero means a cache written before the
    // byte existed, and falls back to the search this used to always do.
    let carried = {
        let bytes = frame
            .activation_cache
            .try_borrow_data()
            .map_err(|_| TradingSbfError::Release)?;
        let bump = *bytes
            .get(ACTIVATION_CACHE_BUMP_OFFSET_V1)
            .ok_or(TradingSbfError::Release)?;
        (bump != 0).then_some(bump)
    };
    let expected_cache = match carried {
        Some(bump) => {
            let bump_seed = [bump];
            Pubkey::create_program_address(
                &[ACTIVATION_PDA_DOMAIN_V1, &release_set, &bump_seed],
                frame.registry.key,
            )
            .map_err(|_| TradingSbfError::Release)?
        }
        None => {
            Pubkey::find_program_address(
                &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
                frame.registry.key,
            )
            .0
        }
    };
    if frame.activation_cache.key != &expected_cache {
        return Err(TradingSbfError::Release.into());
    }
    Ok(ActivationCacheAuthenticatedV1(()))
}

/// The Claims and Custody programs this execution may CPI, read from a cache
/// view whose identity is already established.
///
/// Both arms now read the children out of the SAME decode they authenticate the
/// root roles from: the continuation in `authenticate_accelerator_activation_v4`,
/// the top level in [`authenticate_top_level_root_roles_from_cache_v3`]. This
/// takes the view rather than the account for exactly that reason -- it used to
/// take the account, and paid a third full `decode` of a 1,288-byte immutable
/// buffer to learn two program ids that the caller's own decode already held.
///
/// The children are not authenticated here and are not meant to be: this reads
/// the identities the Direct crosscheck names in the ack it commits, out of a
/// projection the caller has already held to its address, its owner, its width
/// and this Market's release set. Nothing downstream lends them authority
/// without a caller-authority derivation of its own.
pub(super) fn read_activated_child_programs_v3(
    activated: ActivatedExecutionReleaseSetViewV1<'_>,
) -> Result<AuthenticatedChildProgramsV3, ProgramError> {
    let claims = activated
        .role(ExecutionRoleV1::Claims)
        .map_err(|_| TradingSbfError::Release)?
        .release()
        .program()
        .to_bytes();
    let custody = activated
        .role(ExecutionRoleV1::Custody)
        .map_err(|_| TradingSbfError::Release)?
        .release()
        .program()
        .to_bytes();
    Ok(AuthenticatedChildProgramsV3 { claims, custody })
}

pub(super) fn authenticate_accelerator_activation_v4(
    frame: HotFrameV3<'_, '_>,
    envelope: HotExecutionEnvelopeV3,
) -> Result<(AuthenticatedRoleReceiptV1, ContentId, ContentId), ProgramError> {
    require_activation_cache_account_v3(frame, envelope.release_set())?;
    let data = frame
        .activation_cache
        .try_borrow_data()
        .map_err(|_| TradingSbfError::Release)?;
    let activated =
        ActivatedExecutionReleaseSetViewV1::decode(&data).map_err(|_| TradingSbfError::Release)?;
    if activated
        .execution_release_set_id()
        .map_err(|_| TradingSbfError::Release)?
        .to_bytes()
        != envelope.release_set()
    {
        return Err(TradingSbfError::Release.into());
    }
    let core = activated
        .role(ExecutionRoleV1::Core)
        .map_err(|_| TradingSbfError::Release)?;
    let trading = activated
        .role(ExecutionRoleV1::Trading)
        .map_err(|_| TradingSbfError::Release)?;
    let claims = activated
        .role(ExecutionRoleV1::Claims)
        .map_err(|_| TradingSbfError::Release)?;
    let custody = activated
        .role(ExecutionRoleV1::Custody)
        .map_err(|_| TradingSbfError::Release)?;
    drop(data);
    // Both releases come from the Registry activation cache, whose activation
    // already authenticated a chain-observed complete-ELF digest for each role.
    authenticate_activated_current_deployment(
        core.release(),
        frame.core_program,
        frame.core_programdata,
    )
    .map_err(ProgramError::from)?;
    authenticate_activated_current_deployment(
        trading.release(),
        frame.trading_program,
        frame.trading_programdata,
    )
    .map_err(ProgramError::from)?;
    Ok((
        AuthenticatedRoleReceiptV1::new(
            ExecutionRoleV1::Trading,
            ContentId::new(envelope.release_set()).map_err(|_| TradingSbfError::Release)?,
            trading.release().program(),
            trading.artifact_release_id(),
            trading.release().semantic_release_id(),
        ),
        ContentId::new(claims.release().program().to_bytes())
            .map_err(|_| TradingSbfError::Release)?,
        ContentId::new(custody.release().program().to_bytes())
            .map_err(|_| TradingSbfError::Release)?,
    ))
}

pub(super) fn authenticate_accelerator_input_bank_v4(
    request: AdmittedAcceleratorRequestV2<'_>,
    runtime_accounts: &[AccountInfo<'_>],
    trading_program: &Pubkey,
) -> Result<Vec<u8>, ProgramError> {
    let bank = match request.transport() {
        RequestTransportV2::Inline => request.inline_bank().to_vec(),
        RequestTransportV2::ScratchPages => {
            let page_count = usize::try_from(
                request
                    .input_page_count()
                    .map_err(|_| TradingSbfError::Content)?,
            )
            .map_err(|_| TradingSbfError::Content)?;
            let mut pages = vec![None; page_count];
            for account in runtime_accounts {
                if account.owner != trading_program
                    || account.is_signer
                    || account.is_writable
                    || account.executable
                    || runtime_accounts
                        .iter()
                        .filter(|runtime| runtime.key == account.key)
                        .count()
                        != 1
                {
                    continue;
                }
                let data = account
                    .try_borrow_data()
                    .map_err(|_| TradingSbfError::Content)?;
                let Ok(page) = AuthenticatedScratchPageV2::decode(&data) else {
                    continue;
                };
                page.validate_request_input(
                    ContentId::new(trading_program.to_bytes())
                        .map_err(|_| TradingSbfError::Content)?,
                    request,
                )
                .map_err(|_| TradingSbfError::Content)?;
                let index =
                    usize::try_from(page.chunk_index()).map_err(|_| TradingSbfError::Content)?;
                let slot = pages.get_mut(index).ok_or(TradingSbfError::Content)?;
                if slot.is_some() {
                    return Err(TradingSbfError::Content.into());
                }
                *slot = Some((page.chunk_offset(), page.payload().to_vec()));
            }
            let mut bank = Vec::with_capacity(
                usize::try_from(request.total_bank_bytes())
                    .map_err(|_| TradingSbfError::Content)?,
            );
            for (index, page) in pages.into_iter().enumerate() {
                let (offset, payload) = page.ok_or(TradingSbfError::Content)?;
                if usize::try_from(offset).map_err(|_| TradingSbfError::Content)? != bank.len()
                    || index >= page_count
                {
                    return Err(TradingSbfError::Content.into());
                }
                bank.extend_from_slice(&payload);
            }
            bank
        }
    };
    if u64::try_from(bank.len()).map_err(|_| TradingSbfError::Content)?
        != request.total_bank_bytes()
        || ContentId::new(hash(&bank).to_bytes()).map_err(|_| TradingSbfError::Content)?
            != request.input_bank_digest()
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok(bank)
}

pub(super) fn decode_accelerator_register_bank_v4(
    request: AdmittedAcceleratorRequestV2<'_>,
    bank: &[u8],
) -> Result<(Vec<u64>, Vec<[u8; 32]>), ProgramError> {
    let expected = register_bank_bytes_v2(request.scalar_count(), request.identity_count())
        .map_err(|_| TradingSbfError::Content)?;
    if usize::try_from(expected).map_err(|_| TradingSbfError::Content)? != bank.len() {
        return Err(TradingSbfError::Content.into());
    }
    let scalar_bytes = usize::try_from(request.scalar_count())
        .map_err(|_| TradingSbfError::Content)?
        .checked_mul(8)
        .ok_or(TradingSbfError::Content)?;
    let mut scalars = Vec::with_capacity(
        usize::try_from(request.scalar_count()).map_err(|_| TradingSbfError::Content)?,
    );
    for bytes in bank
        .get(..scalar_bytes)
        .ok_or(TradingSbfError::Content)?
        .chunks_exact(8)
    {
        scalars.push(u64::from_le_bytes(
            bytes.try_into().map_err(|_| TradingSbfError::Content)?,
        ));
    }
    let identities = bank
        .get(scalar_bytes..)
        .ok_or(TradingSbfError::Content)?
        .chunks_exact(32)
        .map(|bytes| bytes.try_into().map_err(|_| TradingSbfError::Content))
        .collect::<Result<Vec<[u8; 32]>, _>>()?;
    if scalars.len()
        != usize::try_from(request.scalar_count()).map_err(|_| TradingSbfError::Content)?
        || identities.len()
            != usize::try_from(request.identity_count()).map_err(|_| TradingSbfError::Content)?
    {
        return Err(TradingSbfError::Content.into());
    }
    Ok((scalars, identities))
}

/// The accelerator's copy of the observation walk, keyed the way Trading keys it.
///
/// `representatives` is not optional and not a refinement. Trading keys an
/// aliased coordinate by its representative
/// (`logical_projection_key_v3(*aliases.get(coordinate)…)` in
/// `execute_hot_v3`), and `logical_projection_key_v3` substitutes a *content
/// digest* for coordinates 1..=4 and the *physical key* for everything else.
/// Walking the raw `enumerate()` index here therefore produced a different
/// transcript for every profile that route-aliases a later coordinate onto one
/// of those four. The
/// context digests could not agree, so the admitted-AOT lane refused every
/// well-formed invocation, in the same way and for the same reason as the bare
/// `family_request` hash beside it.
///
/// The expression below is deliberately identical to Trading's rather than
/// merely equivalent to it: this is the third copy of this walk in this file,
/// and copies two and three had already been observed to agree only by
/// inspection.
pub(super) fn accelerator_runtime_observations_digest_v4(
    runtime_accounts: &[AccountInfo<'_>],
    representatives: &[usize],
    request: AdmittedAcceleratorRequestV2<'_>,
    trading_program: &Pubkey,
    selected_config: [u8; 32],
    product_root: [u8; 32],
    portfolio: [u8; 32],
    linked_basis: [u8; 32],
) -> Result<ContentId, ProgramError> {
    if representatives.len() != runtime_accounts.len() {
        return Err(TradingSbfError::AcceleratorRuntimeView.into());
    }
    let projected = LogicalProjectionKeysV3 {
        selected_config,
        product_root,
        portfolio,
        linked_basis,
    };
    let trading_program_id = ContentId::new(trading_program.to_bytes())
        .map_err(|_| TradingSbfError::AcceleratorRuntimeView)?;
    // Exact capacity, not `collect::<Result<Vec<_>, _>>()`. A fallible collect
    // reports a zero lower bound, so the SBF bump allocator - which never frees
    // - is walked through the whole doubling ladder and charges several times
    // the live width for every fallible bank on this path.
    let mut runtime_data = Vec::with_capacity(runtime_accounts.len());
    for account in runtime_accounts.iter() {
        runtime_data.push(
            account
                .try_borrow_data()
                .map_err(|_| TradingSbfError::AcceleratorRuntimeView)?,
        );
    }
    let observations = runtime_accounts
        .iter()
        .zip(&runtime_data)
        .enumerate()
        .map(|(coordinate, (account, data))| {
            // A scratch input page embeds the invocation-context digest this
            // transcript is constructing. Its exact bytes therefore cannot
            // also be an input to that digest. Canonicalize only a page whose
            // account facts and page header authenticate against this exact
            // request; the input-bank digest independently commits its bytes.
            let canonical_scratch = request.transport() == RequestTransportV2::ScratchPages
                && account.owner == trading_program
                && !account.is_signer
                && !account.is_writable
                && !account.executable
                && AuthenticatedScratchPageV2::decode(data.as_ref()).is_ok_and(|page| {
                    page.validate_request_input(trading_program_id, request)
                        .is_ok()
                });
            ShadowRuntimeObservationV3 {
                key: *logical_projection_key_v3(
                    *representatives.get(coordinate).unwrap_or(&coordinate),
                    account.key,
                    &projected,
                ),
                owner: account.owner.to_bytes(),
                lamports: account.lamports(),
                data: if canonical_scratch
                    || transcript_omits_loader_bytes_v3(account.owner, account.executable)
                {
                    &[]
                } else {
                    data.as_ref()
                },
                signer: false,
                writable: false,
                executable: account.executable,
            }
        })
        .collect::<Vec<_>>();
    runtime_observations_digest_v3(&observations)
        .map_err(|_| TradingSbfError::AcceleratorRuntimeView.into())
}

/// Re-derive the caller-authority address the composing Trading signed with.
///
/// THE SEED IS THE SIGNED FAMILY REQUEST, NOT THE ACCELERATOR REQUEST. It was
/// the latter until 2026-09-03, and an accelerator request carries the register
/// bank, which for a window-gated action carries `Clock::get().slot`; a PDA
/// whose address moves every slot cannot be named in an account list fixed at
/// signing time. `admitted_caller_authority_digest_v1` is the one author for
/// what replaced it, and this side and `admitted_composition_v3`'s side both
/// read it rather than each spelling a preimage.
pub(super) fn authenticate_accelerator_caller_authority_v4(
    trading_program: &Pubkey,
    caller_authority: &AccountInfo<'_>,
    envelope: HotExecutionEnvelopeV3,
    root: &Pubkey,
    parent_request_digest: ContentId,
    chunk_index: u32,
    artifact_release: [u8; 32],
    expected_accelerator_program: &Pubkey,
    accelerator_program: &AccountInfo<'_>,
    accelerator_programdata: &AccountInfo<'_>,
    programdata_metadata: ProgramDataMetadataV3View,
) -> Result<AuthenticatedAcceleratorCallerV4, ProgramError> {
    let role_request_digest = accelerator_caller_authority_digest_v1(
        AcceleratorCallerKindV1::Admitted,
        parent_request_digest,
        chunk_index,
    )
    .map_err(|_| TradingSbfError::Release)?;
    let seeds = CallerAuthoritySeedsV1::new(
        ContentId::new(envelope.release_set()).map_err(|_| TradingSbfError::Release)?,
        envelope.market(),
        ExecutionRoleV1::Trading,
        root.to_bytes(),
        role_request_digest.to_bytes(),
    )
    .map_err(|_| TradingSbfError::Release)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), trading_program).0;
    if caller_authority.key != &expected
        || !caller_authority.is_signer
        || caller_authority.is_writable
        || caller_authority.executable
        || accelerator_program.key != expected_accelerator_program
    {
        Err(TradingSbfError::Release.into())
    } else {
        Ok(AuthenticatedAcceleratorCallerV4 {
            release_set: envelope.release_set(),
            market: envelope.market(),
            root: root.to_bytes(),
            role_request_digest: role_request_digest.to_bytes(),
            artifact_release,
            accelerator_program: accelerator_program.key.to_bytes(),
            accelerator_programdata: accelerator_programdata.key.to_bytes(),
            deployment_slot: programdata_metadata.deployment_slot(),
            upgrade_authority: programdata_metadata.upgrade_authority(),
        })
    }
}
