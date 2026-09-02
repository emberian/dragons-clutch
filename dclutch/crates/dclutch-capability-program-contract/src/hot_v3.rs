//! Family-neutral Trading V3 hot execution ABI.
//!
//! The fixed prefix authenticates the Market, immutable capability root, and
//! every content-selection authority before any family runtime account. A
//! disposition-derived ExecutionStrategy suffix follows the prefix; the
//! remaining accounts are the exact AccountProfile/EffectProgram address
//! space. The adapter injects the already-present root, immutable config raw,
//! Product graph-root raw, Product-selected portfolio raw, and the exact
//! Product-linked basis raw accounts as logical coordinates zero through four
//! without duplicating physical metas. Config and Product records are
//! read-only projection evidence and no child route may borrow any injected
//! coordinate. No family discriminator or dummy account is part of the ABI.

/// Canonical hot instruction magic.
pub const HOT_EXECUTION_MAGIC_V3: [u8; 8] = *b"DCLTHOT3";

/// Heap frame a TOP-LEVEL Hot submission must request, in bytes.
///
/// A caller who invokes Trading directly makes two Registry reauthentication
/// CPIs that a Registry continuation never makes, and holds their frames and
/// receipts against a bump allocator that never frees. That route's peak does
/// not fit the protocol default 32,768, so it declares an extended heap
/// profile and the transaction must carry a ComputeBudget `RequestHeapFrame`
/// for this many bytes; Trading refuses by name -- not by running out of
/// memory -- if the grant did not arrive.
///
/// A continuation submission carries NO grant and must not: it fits the
/// default, and its packet has four spare bytes of the v0 ceiling anyway.
///
/// The value is a measured requirement plus margin, not a guess, and it must
/// stay a multiple of 1,024 and at most 262,144 for the runtime to honour it.
/// See `docs/evidence/` for the measurement and the margin it leaves.
pub const DIRECT_HOT_HEAP_FRAME_BYTES_V1: u32 = 65_536;
/// Canonical hot instruction schema version.
pub const HOT_EXECUTION_VERSION_V3: u16 = 3;
/// Canonical family-neutral hot instruction physical profile.
pub const HOT_EXECUTION_PROFILE_V3: u16 = 1;
/// Exact fixed hot envelope width before the family request.
pub const HOT_EXECUTION_ENVELOPE_BYTES_V3: usize = 128;
/// Absolute byte offset of the exact family request in the current instruction.
///
/// RequestProfile V2 native-evidence ranges are absolute, so generators add
/// this constant to family-relative signed-message coordinates.
pub const HOT_FAMILY_REQUEST_OFFSET_V3: usize = HOT_EXECUTION_ENVELOPE_BYTES_V3;

/// Canonical hot acknowledgment magic.
pub const HOT_EXECUTION_ACK_MAGIC_V3: [u8; 8] = *b"DCLTHAK3";
/// Exact hot acknowledgment width.
pub const HOT_EXECUTION_ACK_BYTES_V3: usize = 280;

/// Core Market account.
pub const HOT_MARKET_ACCOUNT_V3: usize = 0;
/// Mutable Trading capability root, committed after every effect/child route.
pub const HOT_ROOT_ACCOUNT_V3: usize = 1;
/// Finalized CapabilityManifest raw record.
pub const HOT_MANIFEST_RAW_ACCOUNT_V3: usize = 2;
/// Vacant CapabilityManifest staging cursor.
pub const HOT_MANIFEST_STAGING_ACCOUNT_V3: usize = 3;
/// Finalized CapabilityProgramSet raw record.
pub const HOT_PROGRAM_SET_RAW_ACCOUNT_V3: usize = 4;
/// Vacant CapabilityProgramSet staging cursor.
pub const HOT_PROGRAM_SET_STAGING_ACCOUNT_V3: usize = 5;
/// Action-selected finalized CapabilityProgramV3 raw record.
pub const HOT_DESCRIPTOR_RAW_ACCOUNT_V3: usize = 6;
/// Vacant CapabilityProgramV3 staging cursor.
pub const HOT_DESCRIPTOR_STAGING_ACCOUNT_V3: usize = 7;
/// Manifest-selected finalized immutable config raw record.
pub const HOT_CONFIG_RAW_ACCOUNT_V3: usize = 8;
/// Vacant immutable config staging cursor.
pub const HOT_CONFIG_STAGING_ACCOUNT_V3: usize = 9;
/// Finalized AccountProfile raw record.
pub const HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3: usize = 10;
/// Vacant AccountProfile staging cursor.
pub const HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3: usize = 11;
/// Finalized RequestProfile raw record.
pub const HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3: usize = 12;
/// Vacant RequestProfile staging cursor.
pub const HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3: usize = 13;
/// Finalized interpreted TransitionVM raw record.
pub const HOT_TRANSITION_RAW_ACCOUNT_V3: usize = 14;
/// Vacant TransitionVM staging cursor.
pub const HOT_TRANSITION_STAGING_ACCOUNT_V3: usize = 15;
/// Finalized EffectProgram raw record.
pub const HOT_EFFECT_RAW_ACCOUNT_V3: usize = 16;
/// Vacant EffectProgram staging cursor.
pub const HOT_EFFECT_STAGING_ACCOUNT_V3: usize = 17;
/// Finalized descriptor-selected StateLifecyclePolicy raw record.
pub const HOT_LIFECYCLE_RAW_ACCOUNT_V3: usize = 18;
/// Vacant StateLifecyclePolicy staging cursor.
pub const HOT_LIFECYCLE_STAGING_ACCOUNT_V3: usize = 19;
/// Finalized ExecutionStrategy raw record.
pub const HOT_STRATEGY_RAW_ACCOUNT_V3: usize = 20;
/// Vacant ExecutionStrategy staging cursor.
pub const HOT_STRATEGY_STAGING_ACCOUNT_V3: usize = 21;
/// Registry activation cache for the Market-selected release set.
pub const HOT_ACTIVATION_CACHE_ACCOUNT_V3: usize = 22;
/// Current Registry-selected Core Program.
pub const HOT_CORE_PROGRAM_ACCOUNT_V3: usize = 23;
/// Current Core ProgramData.
pub const HOT_CORE_PROGRAMDATA_ACCOUNT_V3: usize = 24;
/// Current Registry-selected Trading Program.
pub const HOT_TRADING_PROGRAM_ACCOUNT_V3: usize = 25;
/// Current Trading ProgramData.
pub const HOT_TRADING_PROGRAMDATA_ACCOUNT_V3: usize = 26;
/// Immutable Registry Program selected by the Market.
pub const HOT_REGISTRY_PROGRAM_ACCOUNT_V3: usize = 27;
/// Rent sysvar used for finalized-record and root observations.
pub const HOT_RENT_SYSVAR_ACCOUNT_V3: usize = 28;
/// Instructions sysvar used only when RequestProfile V2 requires native evidence.
pub const HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3: usize = 29;
/// Registry-finalized Product Runtime V2 graph-root raw record selected by Market.
pub const HOT_PRODUCT_RAW_ACCOUNT_V3: usize = 30;
/// Vacant Product Runtime V2 graph-root staging cursor.
pub const HOT_PRODUCT_STAGING_ACCOUNT_V3: usize = 31;
/// Registry-finalized Product-selected result-domain raw record.
pub const HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3: usize = 32;
/// Vacant Product-selected result-domain staging cursor.
pub const HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3: usize = 33;
/// Registry-finalized Product-selected portfolio raw record.
pub const HOT_PORTFOLIO_RAW_ACCOUNT_V3: usize = 34;
/// Vacant Product-selected portfolio staging cursor.
pub const HOT_PORTFOLIO_STAGING_ACCOUNT_V3: usize = 35;
/// Registry-finalized Product-linked basis raw record.
pub const HOT_LINKED_BASIS_RAW_ACCOUNT_V3: usize = 36;
/// Vacant Product-linked basis staging cursor.
pub const HOT_LINKED_BASIS_STAGING_ACCOUNT_V3: usize = 37;
/// Read-only Trading validated-artifact seal for the selected descriptor.
///
/// Decision 0005. The seal is content-addressed under the Trading Program from
/// the selected descriptor identity, the selected action, the authenticated
/// Trading interpreter semantic release, and the Market-selected Registry. It
/// is never writable on a hot action and is authenticated before any artifact
/// it names is decoded.
pub const HOT_CAPABILITY_SEAL_ACCOUNT_V3: usize = 38;
/// Exact family-neutral account prefix width.
pub const HOT_FIXED_ACCOUNT_COUNT_V3: usize = 39;
/// First disposition-derived ExecutionStrategy account.
pub const HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3: usize = HOT_FIXED_ACCOUNT_COUNT_V3;
/// Runtime AccountProfile coordinate occupied by the prefix root account.
pub const HOT_RUNTIME_ROOT_COORDINATE_V3: usize = 0;
/// Runtime AccountProfile coordinate occupied by authenticated immutable config.
pub const HOT_RUNTIME_CONFIG_COORDINATE_V3: usize = 1;
/// Runtime AccountProfile coordinate occupied by the Product graph-root body.
pub const HOT_RUNTIME_PRODUCT_COORDINATE_V3: usize = 2;
/// Runtime AccountProfile coordinate occupied by the Product portfolio body.
pub const HOT_RUNTIME_PORTFOLIO_COORDINATE_V3: usize = 3;
/// Runtime AccountProfile coordinate occupied by the Product-linked basis body.
pub const HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3: usize = 4;
/// Number of fixed-prefix accounts injected into the logical runtime vector.
pub const HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3: usize = 5;
/// Common identity-register input containing SHA-256 of the exact family request.
///
/// This is the sole adapter-computed register fact. AccountProfile and
/// RequestProfile own every other register coordinate, while EffectProgram
/// may copy this digest into canonical child `parent_request_digest` fields.
pub const HOT_PARENT_REQUEST_DIGEST_IDENTITY_V3: usize = 0;

const ENVELOPE_REQUEST_BYTES_OFFSET: usize = 12;
const ENVELOPE_RELEASE_SET_OFFSET: usize = 16;
const ENVELOPE_MARKET_OFFSET: usize = 48;
const ENVELOPE_GENERATION_OFFSET: usize = 80;
const ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET: usize = 88;

const ACK_RELEASE_SET_OFFSET: usize = 16;
const ACK_MARKET_OFFSET: usize = 48;
const ACK_GENERATION_OFFSET: usize = 80;
const ACK_ROOT_OFFSET: usize = 88;
const ACK_REQUEST_DIGEST_OFFSET: usize = 120;
const ACK_SELECTED_PROGRAM_OFFSET: usize = 152;
const ACK_ROOT_PRESTATE_DIGEST_OFFSET: usize = 184;
const ACK_ROOT_POSTSTATE_DIGEST_OFFSET: usize = 216;
const ACK_EXECUTION_DIGEST_OFFSET: usize = 248;

/// Stable hot-envelope or acknowledgment refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HotExecutionErrorV3 {
    /// The exact fixed width or count-derived instruction width differed.
    InvalidLength,
    /// Magic selected another instruction or receipt.
    InvalidMagic,
    /// Schema version, physical profile, or reserved bytes were noncanonical.
    UnsupportedProfile,
    /// A required content/account/digest identity was zero.
    ZeroIdentity,
}

/// Result alias for the common hot ABI.
pub type HotExecutionResultV3<T> = core::result::Result<T, HotExecutionErrorV3>;

/// Number of caller-mined bump hints the hot envelope carries.
pub const HOT_BUMP_HINT_COUNT_V1: usize = 8;
/// Offset of the caller-mined bump hints. Formerly the reserved block.
pub const HOT_BUMP_HINTS_OFFSET_V1: usize = 120;

/// The eight bump hints a caller mines off chain so the route never searches.
///
/// # Why a hot route may not search at all
///
/// A PDA bump is `Geometric(1/2)` in the participant key, and
/// `find_program_address` costs 1,500 CU per rejected candidate. A route with
/// surviving searches therefore has no compute ceiling: its cost is a property
/// of whose key is trading, and some fraction of strangers draw deep enough to
/// exceed the transaction limit and are refused for no reason they can see or
/// fix. That fraction is a product defect, not a tail statistic. These eight
/// bytes exist so the number of searches on the public hot path is zero and
/// per-key cost is a property of the code.
///
/// # Why a hint is not the wire naming an address
///
/// `dispatch.rs` refuses a caller-supplied `SelectedRecordBumpsV1` outright --
/// "a selection arriving on the wire asserts identities, never addresses" --
/// and that refusal stands unchanged. It is about a bump written into an
/// account that later readers must TRUST, where a non-canonical value names a
/// different valid address and nothing re-checks it.
///
/// A hint is the opposite shape, and it is the shape
/// `dclutch_custody_sbf::split_caller_authority_bump_v1` and
/// `dclutch_claims_sbf::sparse_native_transfer_v1::split_instruction` already
/// ship: the program rebuilds the seeds ITSELF, reproduces the address with
/// `create_program_address`, and compares the result against the account the
/// frame supplied. A wrong hint reproduces a different address and refuses. The
/// derivation IS the check, so the hint is a memo about a search the caller
/// already paid for and can never be an authority.
///
/// # Why the envelope and not a suffix
///
/// The seeds of several of these addresses end in a digest over the family
/// request, so a hint written INSIDE the request has no fixed point: it changes
/// the digest, which changes the address, which changes the bump. The two
/// shipped precedents put their byte after the request for exactly that reason.
///
/// This block goes one better and rides in the envelope, BEFORE the request, at
/// the eight bytes the V3 wire already reserved. Three consequences, all of
/// them the point:
///
/// * `hash(family_request)` cannot see it, so no parent request digest, child
///   caller authority or acknowledgment moves -- pinned by
///   `the_hot_request_digest_covers_the_family_request_only`;
/// * the maker Ed25519 windows are absolute offsets rebased on
///   `HOT_FAMILY_REQUEST_OFFSET_V3`, which does not move, so no signed message
///   moves either;
/// * and the packet does not grow by one byte. That is not an aesthetic
///   preference. A Registry continuation submission has four spare bytes of the
///   v0 packet ceiling, so a trailing suffix wide enough to carry eight hints
///   would not fit on that route at all.
///
/// # Zero means absent
///
/// An unset hint is zero and the reader searches exactly as it used to, so the
/// all-zero wire every current caller emits keeps working unchanged and no
/// account and no market needs migrating. Zero is not a value any derivation
/// hands back in practice, which is the same reading `split_caller_authority_bump_v1`
/// already gives it.
///
/// # The slots are roles, not addresses
///
/// This envelope is family-neutral, so the slots name the ROLE a bump plays in
/// any hot execution -- never a Direct account. `lifecycle` is indexed in the
/// order the StateLifecyclePolicy materializes created accounts; `child_caller`
/// and `child_relay` in child-route order. A family that creates more than two
/// accounts or drives more than two child routes gets the search back for the
/// ones past the end, and says so, rather than silently reusing a neighbour's
/// slot.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HotBumpHintsV1 {
    /// Core Market state PDA bump. Relayed to every child that reads the Market.
    pub market: u8,
    /// Trading capability root PDA bump.
    pub root: u8,
    /// Lifecycle-created account bumps, in lifecycle materialization order.
    pub lifecycle: [u8; 2],
    /// Trading caller-authority bumps, in child-route order.
    pub child_caller: [u8; 2],
    /// Bumps a child derives internally, relayed in its request, in route order.
    pub child_relay: [u8; 2],
}

impl HotBumpHintsV1 {
    /// The all-zero block: every reader searches, exactly as it used to.
    pub const ABSENT: Self = Self {
        market: 0,
        root: 0,
        lifecycle: [0; 2],
        child_caller: [0; 2],
        child_relay: [0; 2],
    };

    /// Read the block in canonical slot order.
    const fn from_bytes(bytes: [u8; HOT_BUMP_HINT_COUNT_V1]) -> Self {
        Self {
            market: bytes[0],
            root: bytes[1],
            lifecycle: [bytes[2], bytes[3]],
            child_caller: [bytes[4], bytes[5]],
            child_relay: [bytes[6], bytes[7]],
        }
    }

    /// Write the block in canonical slot order.
    pub const fn to_bytes(self) -> [u8; HOT_BUMP_HINT_COUNT_V1] {
        [
            self.market,
            self.root,
            self.lifecycle[0],
            self.lifecycle[1],
            self.child_caller[0],
            self.child_caller[1],
            self.child_relay[0],
            self.child_relay[1],
        ]
    }

    /// Whether no hint is set, so every reader on this execution searches.
    pub fn is_absent(self) -> bool {
        self == Self::ABSENT
    }
}

/// Read one hint as the `Option<u8>` every reproducing reader takes.
///
/// Zero is absent. Every consumer in the tree already spells its two arms as
/// `Some(bump) => create_program_address(..)` and `None => find_program_address(..)`,
/// so a hint reaches them in the shape they already have.
pub const fn hot_bump_hint_v1(hint: u8) -> Option<u8> {
    if hint == 0 { None } else { Some(hint) }
}

/// Exact immutable envelope preceding one family request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HotExecutionEnvelopeV3 {
    request_bytes: u32,
    release_set: [u8; 32],
    market: [u8; 32],
    generation: u64,
    root_prestate_digest: [u8; 32],
    bump_hints: HotBumpHintsV1,
}

impl HotExecutionEnvelopeV3 {
    /// Construct an exact envelope for one nonempty family request.
    pub fn new(
        request_bytes: u32,
        release_set: [u8; 32],
        market: [u8; 32],
        generation: u64,
        root_prestate_digest: [u8; 32],
    ) -> HotExecutionResultV3<Self> {
        if request_bytes == 0 {
            return Err(HotExecutionErrorV3::InvalidLength);
        }
        for identity in [release_set, market, root_prestate_digest] {
            require_nonzero(identity)?;
        }
        Ok(Self {
            request_bytes,
            release_set,
            market,
            generation,
            root_prestate_digest,
            bump_hints: HotBumpHintsV1::ABSENT,
        })
    }

    /// Carry the eight bumps the caller mined off chain.
    ///
    /// Separate from [`Self::new`] because a hint is not part of the envelope's
    /// identity: it names no fact, and an envelope carrying none is the same
    /// execution, more expensively. See [`HotBumpHintsV1`].
    #[must_use]
    pub const fn with_bump_hints(mut self, hints: HotBumpHintsV1) -> Self {
        self.bump_hints = hints;
        self
    }

    /// Hostile-decode the exact fixed envelope without accepting request bytes.
    pub fn decode(bytes: &[u8]) -> HotExecutionResultV3<Self> {
        if bytes.len() != HOT_EXECUTION_ENVELOPE_BYTES_V3 {
            return Err(HotExecutionErrorV3::InvalidLength);
        }
        if bytes.get(..8) != Some(HOT_EXECUTION_MAGIC_V3.as_slice()) {
            return Err(HotExecutionErrorV3::InvalidMagic);
        }
        if read_u16(bytes, 8)? != HOT_EXECUTION_VERSION_V3
            || read_u16(bytes, 10)? != HOT_EXECUTION_PROFILE_V3
        {
            return Err(HotExecutionErrorV3::UnsupportedProfile);
        }
        Ok(Self::new(
            read_u32(bytes, ENVELOPE_REQUEST_BYTES_OFFSET)?,
            read_array(bytes, ENVELOPE_RELEASE_SET_OFFSET)?,
            read_array(bytes, ENVELOPE_MARKET_OFFSET)?,
            read_u64(bytes, ENVELOPE_GENERATION_OFFSET)?,
            read_array(bytes, ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET)?,
        )?
        .with_bump_hints(HotBumpHintsV1::from_bytes(
            slice(bytes, HOT_BUMP_HINTS_OFFSET_V1, HOT_BUMP_HINT_COUNT_V1)?
                .try_into()
                .map_err(|_| HotExecutionErrorV3::InvalidLength)?,
        )))
    }

    /// Split a complete instruction into its exact envelope and family request.
    pub fn split_instruction(bytes: &[u8]) -> HotExecutionResultV3<(Self, &[u8])> {
        let envelope = Self::decode(
            bytes
                .get(..HOT_EXECUTION_ENVELOPE_BYTES_V3)
                .ok_or(HotExecutionErrorV3::InvalidLength)?,
        )?;
        let request_bytes = usize::try_from(envelope.request_bytes)
            .map_err(|_| HotExecutionErrorV3::InvalidLength)?;
        let expected = HOT_EXECUTION_ENVELOPE_BYTES_V3
            .checked_add(request_bytes)
            .ok_or(HotExecutionErrorV3::InvalidLength)?;
        if bytes.len() != expected {
            return Err(HotExecutionErrorV3::InvalidLength);
        }
        Ok((
            envelope,
            bytes
                .get(HOT_FAMILY_REQUEST_OFFSET_V3..)
                .ok_or(HotExecutionErrorV3::InvalidLength)?,
        ))
    }

    /// Exact nonzero family request width.
    pub const fn request_bytes(self) -> u32 {
        self.request_bytes
    }

    /// Current immutable execution release set.
    pub const fn release_set(self) -> [u8; 32] {
        self.release_set
    }

    /// Exact Core Market identity.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }

    /// Exact Core Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }

    /// Optimistic prestate commitment of the complete capability root.
    pub const fn root_prestate_digest(self) -> [u8; 32] {
        self.root_prestate_digest
    }

    /// The eight caller-mined bumps this execution may reproduce addresses from.
    ///
    /// Never an authority. Every consumer rebuilds the seeds itself and refuses
    /// unless the hint reproduces the account it was handed. See
    /// [`HotBumpHintsV1`].
    pub const fn bump_hints(self) -> HotBumpHintsV1 {
        self.bump_hints
    }

    /// Encode the exact canonical fixed envelope.
    pub fn to_bytes(self) -> [u8; HOT_EXECUTION_ENVELOPE_BYTES_V3] {
        let mut output = [0_u8; HOT_EXECUTION_ENVELOPE_BYTES_V3];
        put(&mut output, 0, &HOT_EXECUTION_MAGIC_V3);
        put(&mut output, 8, &HOT_EXECUTION_VERSION_V3.to_le_bytes());
        put(&mut output, 10, &HOT_EXECUTION_PROFILE_V3.to_le_bytes());
        put(
            &mut output,
            ENVELOPE_REQUEST_BYTES_OFFSET,
            &self.request_bytes.to_le_bytes(),
        );
        put(&mut output, ENVELOPE_RELEASE_SET_OFFSET, &self.release_set);
        put(&mut output, ENVELOPE_MARKET_OFFSET, &self.market);
        put(
            &mut output,
            ENVELOPE_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        put(
            &mut output,
            ENVELOPE_ROOT_PRESTATE_DIGEST_OFFSET,
            &self.root_prestate_digest,
        );
        put(
            &mut output,
            HOT_BUMP_HINTS_OFFSET_V1,
            &self.bump_hints.to_bytes(),
        );
        output
    }
}

/// Exact commit-last evidence returned by the common hot outer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HotExecutionAckV3 {
    /// Current immutable execution release set.
    pub release_set: [u8; 32],
    /// Exact Core Market identity.
    pub market: [u8; 32],
    /// Exact Core Market generation.
    pub generation: u64,
    /// Exact mutable capability-root account identity.
    pub root: [u8; 32],
    /// SHA-256 of the complete exact family request.
    pub request_digest: [u8; 32],
    /// Action-selected exact CapabilityProgramV3 content identity.
    pub selected_program: [u8; 32],
    /// Complete root commitment before execution.
    pub root_prestate_digest: [u8; 32],
    /// Complete root commitment after every accepted effect/child route.
    pub root_poststate_digest: [u8; 32],
    /// Domain-separated commitment to selected artifacts and child receipts.
    pub execution_digest: [u8; 32],
}

impl HotExecutionAckV3 {
    /// Construct exact nonzero commit-last evidence.
    pub fn new(value: Self) -> HotExecutionResultV3<Self> {
        for identity in [
            value.release_set,
            value.market,
            value.root,
            value.request_digest,
            value.selected_program,
            value.root_prestate_digest,
            value.root_poststate_digest,
            value.execution_digest,
        ] {
            require_nonzero(identity)?;
        }
        Ok(value)
    }

    /// Hostile-decode one exact acknowledgment.
    pub fn decode(bytes: &[u8]) -> HotExecutionResultV3<Self> {
        if bytes.len() != HOT_EXECUTION_ACK_BYTES_V3 {
            return Err(HotExecutionErrorV3::InvalidLength);
        }
        if bytes.get(..8) != Some(HOT_EXECUTION_ACK_MAGIC_V3.as_slice()) {
            return Err(HotExecutionErrorV3::InvalidMagic);
        }
        if read_u16(bytes, 8)? != HOT_EXECUTION_VERSION_V3
            || read_u16(bytes, 10)? != HOT_EXECUTION_PROFILE_V3
            || !all_zero(slice(bytes, 12, 4)?)
        {
            return Err(HotExecutionErrorV3::UnsupportedProfile);
        }
        Self::new(Self {
            release_set: read_array(bytes, ACK_RELEASE_SET_OFFSET)?,
            market: read_array(bytes, ACK_MARKET_OFFSET)?,
            generation: read_u64(bytes, ACK_GENERATION_OFFSET)?,
            root: read_array(bytes, ACK_ROOT_OFFSET)?,
            request_digest: read_array(bytes, ACK_REQUEST_DIGEST_OFFSET)?,
            selected_program: read_array(bytes, ACK_SELECTED_PROGRAM_OFFSET)?,
            root_prestate_digest: read_array(bytes, ACK_ROOT_PRESTATE_DIGEST_OFFSET)?,
            root_poststate_digest: read_array(bytes, ACK_ROOT_POSTSTATE_DIGEST_OFFSET)?,
            execution_digest: read_array(bytes, ACK_EXECUTION_DIGEST_OFFSET)?,
        })
    }

    /// Encode exact canonical acknowledgment bytes.
    pub fn to_bytes(self) -> [u8; HOT_EXECUTION_ACK_BYTES_V3] {
        let mut output = [0_u8; HOT_EXECUTION_ACK_BYTES_V3];
        put(&mut output, 0, &HOT_EXECUTION_ACK_MAGIC_V3);
        put(&mut output, 8, &HOT_EXECUTION_VERSION_V3.to_le_bytes());
        put(&mut output, 10, &HOT_EXECUTION_PROFILE_V3.to_le_bytes());
        for (offset, value) in [
            (ACK_RELEASE_SET_OFFSET, self.release_set),
            (ACK_MARKET_OFFSET, self.market),
            (ACK_ROOT_OFFSET, self.root),
            (ACK_REQUEST_DIGEST_OFFSET, self.request_digest),
            (ACK_SELECTED_PROGRAM_OFFSET, self.selected_program),
            (ACK_ROOT_PRESTATE_DIGEST_OFFSET, self.root_prestate_digest),
            (ACK_ROOT_POSTSTATE_DIGEST_OFFSET, self.root_poststate_digest),
            (ACK_EXECUTION_DIGEST_OFFSET, self.execution_digest),
        ] {
            put(&mut output, offset, &value);
        }
        put(
            &mut output,
            ACK_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        );
        output
    }
}

fn require_nonzero(value: [u8; 32]) -> HotExecutionResultV3<()> {
    if value == [0; 32] {
        Err(HotExecutionErrorV3::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn slice(bytes: &[u8], offset: usize, width: usize) -> HotExecutionResultV3<&[u8]> {
    bytes
        .get(
            offset
                ..offset
                    .checked_add(width)
                    .ok_or(HotExecutionErrorV3::InvalidLength)?,
        )
        .ok_or(HotExecutionErrorV3::InvalidLength)
}

fn read_u16(bytes: &[u8], offset: usize) -> HotExecutionResultV3<u16> {
    Ok(u16::from_le_bytes(
        slice(bytes, offset, 2)?
            .try_into()
            .map_err(|_| HotExecutionErrorV3::InvalidLength)?,
    ))
}

fn read_u32(bytes: &[u8], offset: usize) -> HotExecutionResultV3<u32> {
    Ok(u32::from_le_bytes(
        slice(bytes, offset, 4)?
            .try_into()
            .map_err(|_| HotExecutionErrorV3::InvalidLength)?,
    ))
}

fn read_u64(bytes: &[u8], offset: usize) -> HotExecutionResultV3<u64> {
    Ok(u64::from_le_bytes(
        slice(bytes, offset, 8)?
            .try_into()
            .map_err(|_| HotExecutionErrorV3::InvalidLength)?,
    ))
}

fn read_array(bytes: &[u8], offset: usize) -> HotExecutionResultV3<[u8; 32]> {
    slice(bytes, offset, 32)?
        .try_into()
        .map_err(|_| HotExecutionErrorV3::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(value) {
        *destination = *source;
    }
}

fn all_zero(bytes: &[u8]) -> bool {
    bytes.iter().all(|value| *value == 0)
}

/// The six `(raw, staging)` fixed coordinate pairs the seal-backed shape
/// collapses.
///
/// These are the per-ACTION records -- descriptor, account profile, request
/// profile, transition, effect, lifecycle -- and they are exactly the six
/// `process_capability_seal_v1` calls `borrow_finalized_record` on to mint a
/// seal, hence exactly the six whose staging cursors the seal durably witnessed
/// vacant. The per-ROOT (manifest, program set, config) and per-STRATEGY
/// (strategy, product, result domain, portfolio, linked basis) pairs are not
/// the seal's and stay distinct.
///
/// This table lived TWICE -- privately in `dclutch-trading-sbf`'s executor and
/// again in `dclutch-operator`'s Direct route projector -- and a third copy was
/// about to be written for the Dealer producer. It belongs beside
/// [`SEALED_EXECUTION_ALIAS_FAMILIES_V3`]: one declaration says WHICH families
/// submit the shape, one says WHAT the shape is, and every executor, builder
/// and operator reads both from here.
pub const SEALED_EXECUTION_FIXED_ALIASES_V3: [(usize, usize); 6] = [
    (
        HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
        HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
    ),
    (
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
        HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
    ),
    (
        HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3,
    ),
    (
        HOT_TRANSITION_RAW_ACCOUNT_V3,
        HOT_TRANSITION_STAGING_ACCOUNT_V3,
    ),
    (HOT_EFFECT_RAW_ACCOUNT_V3, HOT_EFFECT_STAGING_ACCOUNT_V3),
    (
        HOT_LIFECYCLE_RAW_ACCOUNT_V3,
        HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
    ),
];

/// Which capability families submit the seal-backed ALIASED fixed frame.
///
/// Six of the thirty-nine fixed coordinates are per-ACTION staging cursors --
/// descriptor, lifecycle, account profile, request profile, transition, effect
/// -- and a family submits them one of exactly two ways. In the DISTINCT shape
/// each staging coordinate carries the real cursor PDA, System-owned and
/// zero-length, and the hot route observes its non-existence live. In the
/// ALIASED shape each staging coordinate repeats its own raw record, so the
/// transaction locks one account per record instead of two.
///
/// The aliased shape is sound because the fact it stops re-observing is
/// already persisted: `hot_v3/seal.rs` mints one write-once Trading-owned seal
/// per (descriptor, action, Trading semantic release, Registry) and can only
/// mint it by calling `borrow_finalized_record` on exactly these six records,
/// each of which requires the cursor vacant and each of which is recorded as a
/// `SealedRecordRowV1`. Registry finalization is monotone -- `Begin` refuses a
/// non-vacant raw record and `Abort` refuses a closed cursor -- so a record
/// finalized once is finalized for the life of the chain, and the seal's seeds
/// join the Trading semantic release, so an interpreter upgrade mints afresh
/// rather than inheriting a stale verdict. See
/// `docs/design/REGISTRY_FINALIZATION_OBSERVATION_2026_09_02.md`.
///
/// The shape is a property of the FAMILY and not of the submitter: Trading
/// requires the frame to match this table exactly, so a client can neither opt
/// in to save locks nor opt out to look ordinary. This table is the sole place
/// the choice is declared, and the executor, every bundle builder, and every
/// operator read it here rather than each spelling the rule again.
///
/// `None` in the second position means every action of that kind; `Some(a)`
/// means that one action.
///
/// The kind identities are SHA-256 of the capability-kind labels their owning
/// crates define, restated here because those crates depend on this one and
/// cannot be depended on back. Each owner pins its own literal against this
/// one in a test, so a drift is red rather than silent.
/// ADDING A FAMILY IS ONE ROW HERE AND THREE PRODUCERS. Measured for the
/// Dealer kind (`a768…d98a`, SHA-256 of `b"dclutch/capability/dealer-v2"`,
/// `None` for every action) on real ELFs, 2026-09-02: LP-hot 54 -> 48 unique
/// locks and the equity Add 70 -> 64, the campaign unchanged at 30/1. That row
/// is NOT here, because a row without its producers is a family whose
/// transactions the chain refuses: `apps/dclutch-web`'s `dealerEquityChain.ts`
/// THROWS on a staging coordinate that is not the derived cursor, the operator
/// has no projector for the shape, and the sealed-execution hostile does not
/// cover the kind. See the appendix of
/// `docs/design/REGISTRY_FINALIZATION_OBSERVATION_2026_09_02.md`.
pub const SEALED_EXECUTION_ALIAS_FAMILIES_V3: [([u8; 32], Option<u32>); 1] = [
    // SHA-256 of `b"dclutch/capability/direct-successor-v3"`, InlineOrdinary.
    (
        [
            0x2f, 0x9c, 0xf5, 0x05, 0xbd, 0x6a, 0x41, 0x7e, 0x88, 0x22, 0xce, 0xe2, 0xb4, 0x27,
            0x24, 0x6d, 0x7b, 0x2a, 0xd8, 0x25, 0x7a, 0x9a, 0xbe, 0xf5, 0xa8, 0x52, 0xc7, 0x24,
            0x8f, 0x53, 0x31, 0x47,
        ],
        Some(1),
    ),
];

/// Whether this family's fixed frame aliases its six per-action staging
/// coordinates onto their raw records.
///
/// See [`SEALED_EXECUTION_ALIAS_FAMILIES_V3`]. Trading compares the answer
/// against the frame it was handed with `!=`, not `||`: the wrong shape for
/// the family refuses in either direction.
#[must_use]
pub fn hot_frame_uses_sealed_execution_aliases_v3(kind: [u8; 32], action: u32) -> bool {
    SEALED_EXECUTION_ALIAS_FAMILIES_V3
        .iter()
        .any(|(family, selected)| *family == kind && selected.is_none_or(|value| value == action))
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::vec::Vec;

    use super::*;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    #[test]
    fn envelope_splits_one_exact_family_request() {
        let envelope = HotExecutionEnvelopeV3::new(3, id(1), id(2), 7, id(3)).expect("envelope");
        let mut instruction = Vec::from(envelope.to_bytes());
        instruction.extend_from_slice(b"hot");
        assert_eq!(
            HotExecutionEnvelopeV3::split_instruction(&instruction),
            Ok((envelope, b"hot".as_slice()))
        );
        instruction.push(0);
        assert_eq!(
            HotExecutionEnvelopeV3::split_instruction(&instruction),
            Err(HotExecutionErrorV3::InvalidLength)
        );
    }

    #[test]
    fn reserved_zero_identities_and_width_substitution_refuse() {
        let envelope = HotExecutionEnvelopeV3::new(1, id(1), id(2), 0, id(3)).expect("envelope");
        let bytes = envelope.to_bytes();
        for offset in [0, 8, 10] {
            let mut hostile = bytes;
            *hostile.get_mut(offset).expect("hostile offset") ^= 1;
            assert!(HotExecutionEnvelopeV3::decode(&hostile).is_err());
        }
        assert_eq!(
            HotExecutionEnvelopeV3::new(1, [0; 32], id(2), 0, id(3)),
            Err(HotExecutionErrorV3::ZeroIdentity)
        );
        assert_eq!(
            HotExecutionEnvelopeV3::decode(
                bytes
                    .get(..bytes.len() - 1)
                    .expect("one-byte-short envelope"),
            ),
            Err(HotExecutionErrorV3::InvalidLength)
        );
    }

    fn hints() -> HotBumpHintsV1 {
        HotBumpHintsV1 {
            market: 254,
            root: 253,
            lifecycle: [252, 251],
            child_caller: [250, 249],
            child_relay: [248, 247],
        }
    }

    #[test]
    fn bump_hints_ride_the_envelope_and_leave_the_family_request_untouched() {
        let envelope = HotExecutionEnvelopeV3::new(3, id(1), id(2), 7, id(3)).expect("envelope");
        let hinted = envelope.with_bump_hints(hints());
        assert!(envelope.bump_hints().is_absent());
        assert_eq!(hinted.bump_hints(), hints());

        // The whole difference between the two wires is the eight hint bytes,
        // and every one of them is BEFORE the family request. That is the
        // property the parent request digest, the child caller authorities and
        // the maker Ed25519 windows all rest on: a hint cannot reach any of
        // them, so adding one moves no digest and no signed message.
        let plain = envelope.to_bytes();
        let carried = hinted.to_bytes();
        for (offset, (left, right)) in plain.iter().zip(carried.iter()).enumerate() {
            assert_eq!(
                left == right,
                !(HOT_BUMP_HINTS_OFFSET_V1..HOT_BUMP_HINTS_OFFSET_V1 + HOT_BUMP_HINT_COUNT_V1)
                    .contains(&offset),
                "byte {offset} moved outside the hint block",
            );
        }
        assert_eq!(
            HOT_BUMP_HINTS_OFFSET_V1 + HOT_BUMP_HINT_COUNT_V1,
            HOT_EXECUTION_ENVELOPE_BYTES_V3,
        );
        assert_eq!(
            HOT_FAMILY_REQUEST_OFFSET_V3,
            HOT_EXECUTION_ENVELOPE_BYTES_V3
        );

        let mut instruction = Vec::from(carried);
        instruction.extend_from_slice(b"hot");
        assert_eq!(
            HotExecutionEnvelopeV3::split_instruction(&instruction),
            Ok((hinted, b"hot".as_slice())),
        );
    }

    #[test]
    fn every_hint_slot_round_trips_at_its_own_canonical_offset() {
        let envelope = HotExecutionEnvelopeV3::new(1, id(1), id(2), 0, id(3)).expect("envelope");
        let bytes = envelope.with_bump_hints(hints()).to_bytes();
        assert_eq!(
            bytes.get(HOT_BUMP_HINTS_OFFSET_V1..).expect("hint block"),
            &[254, 253, 252, 251, 250, 249, 248, 247],
        );
        assert_eq!(
            HotExecutionEnvelopeV3::decode(&bytes)
                .expect("hinted envelope")
                .bump_hints(),
            hints(),
        );
    }

    #[test]
    fn the_all_zero_hint_block_is_absent_and_still_decodes() {
        // Every wire emitted before this block existed is all-zero here, and
        // must keep decoding to an execution that searches exactly as it did.
        // No market, no account and no caller needs migrating.
        let envelope = HotExecutionEnvelopeV3::new(1, id(1), id(2), 0, id(3)).expect("envelope");
        let bytes = envelope.to_bytes();
        assert!(all_zero(
            bytes.get(HOT_BUMP_HINTS_OFFSET_V1..).expect("hint block"),
        ));
        let decoded = HotExecutionEnvelopeV3::decode(&bytes).expect("absent hints decode");
        assert_eq!(decoded, envelope);
        assert!(decoded.bump_hints().is_absent());
        assert_eq!(decoded.bump_hints(), HotBumpHintsV1::default());
        for hint in HotBumpHintsV1::ABSENT.to_bytes() {
            assert_eq!(hot_bump_hint_v1(hint), None);
        }
        assert_eq!(hot_bump_hint_v1(255), Some(255));
    }

    #[test]
    fn commit_last_ack_binds_program_request_root_and_execution() {
        let ack = HotExecutionAckV3::new(HotExecutionAckV3 {
            release_set: id(1),
            market: id(2),
            generation: 3,
            root: id(4),
            request_digest: id(5),
            selected_program: id(6),
            root_prestate_digest: id(7),
            root_poststate_digest: id(8),
            execution_digest: id(9),
        })
        .expect("ack");
        let bytes = ack.to_bytes();
        assert_eq!(HotExecutionAckV3::decode(&bytes), Ok(ack));
        let mut bad_magic = bytes;
        bad_magic[0] = 0;
        assert!(HotExecutionAckV3::decode(&bad_magic).is_err());
        let mut bad_reserved = bytes;
        bad_reserved[12] = 1;
        assert!(HotExecutionAckV3::decode(&bad_reserved).is_err());
        for offset in [ACK_SELECTED_PROGRAM_OFFSET, ACK_EXECUTION_DIGEST_OFFSET] {
            let mut zero_identity = bytes;
            zero_identity
                .get_mut(offset..offset + 32)
                .expect("identity field")
                .fill(0);
            assert!(HotExecutionAckV3::decode(&zero_identity).is_err());
        }
    }

    #[test]
    fn frame_prefix_is_contiguous_and_runtime_root_is_unique() {
        assert_eq!(HOT_MARKET_ACCOUNT_V3, 0);
        assert_eq!(HOT_ROOT_ACCOUNT_V3, 1);
        assert_eq!(
            HOT_EFFECT_STAGING_ACCOUNT_V3 + 1,
            HOT_LIFECYCLE_RAW_ACCOUNT_V3
        );
        assert_eq!(
            HOT_LIFECYCLE_STAGING_ACCOUNT_V3 + 1,
            HOT_STRATEGY_RAW_ACCOUNT_V3
        );
        assert_eq!(
            HOT_STRATEGY_STAGING_ACCOUNT_V3 + 1,
            HOT_ACTIVATION_CACHE_ACCOUNT_V3
        );
        assert_eq!(
            HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3 + 1,
            HOT_PRODUCT_RAW_ACCOUNT_V3
        );
        assert_eq!(
            HOT_PRODUCT_RAW_ACCOUNT_V3 + 1,
            HOT_PRODUCT_STAGING_ACCOUNT_V3
        );
        assert_eq!(
            HOT_PRODUCT_STAGING_ACCOUNT_V3 + 1,
            HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3
        );
        assert_eq!(
            HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3 + 1,
            HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3
        );
        assert_eq!(
            HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3 + 1,
            HOT_PORTFOLIO_RAW_ACCOUNT_V3
        );
        assert_eq!(
            HOT_PORTFOLIO_RAW_ACCOUNT_V3 + 1,
            HOT_PORTFOLIO_STAGING_ACCOUNT_V3
        );
        assert_eq!(
            HOT_PORTFOLIO_STAGING_ACCOUNT_V3 + 1,
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3
        );
        assert_eq!(
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3 + 1,
            HOT_LINKED_BASIS_STAGING_ACCOUNT_V3
        );
        assert_eq!(
            HOT_LINKED_BASIS_STAGING_ACCOUNT_V3 + 1,
            HOT_CAPABILITY_SEAL_ACCOUNT_V3
        );
        assert_eq!(
            HOT_CAPABILITY_SEAL_ACCOUNT_V3 + 1,
            HOT_FIXED_ACCOUNT_COUNT_V3
        );
        assert_eq!(
            HOT_STRATEGY_EXTRA_ACCOUNTS_START_V3,
            HOT_FIXED_ACCOUNT_COUNT_V3
        );
        assert_eq!(HOT_RUNTIME_ROOT_COORDINATE_V3, 0);
        assert_eq!(HOT_RUNTIME_CONFIG_COORDINATE_V3, 1);
        assert_eq!(HOT_RUNTIME_PRODUCT_COORDINATE_V3, 2);
        assert_eq!(HOT_RUNTIME_PORTFOLIO_COORDINATE_V3, 3);
        assert_eq!(HOT_RUNTIME_LINKED_BASIS_COORDINATE_V3, 4);
        assert_eq!(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, 5);
        assert_eq!(
            HOT_FAMILY_REQUEST_OFFSET_V3,
            HOT_EXECUTION_ENVELOPE_BYTES_V3
        );
    }
}
