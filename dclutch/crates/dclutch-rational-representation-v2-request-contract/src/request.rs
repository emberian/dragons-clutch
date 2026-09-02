//! Lean-owned physical request and dynamic asset rows.

use dclutch_token_svm::TokenProgram;

use crate::{
    ABSENT_REVISION, Error, Result, array_at, byte_at, generated::*, is_zero, put, put_byte,
    require_nonzero, require_zero, subslice, u16_at, u32_at, u64_at,
};

/// Claims release role authorized by the upstream caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CallerRoleV2 {
    /// Market Core orchestration.
    Core = CALLER_ROLE_CORE,
    /// Trading capability orchestration.
    Trading = CALLER_ROLE_TRADING,
}

impl CallerRoleV2 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            CALLER_ROLE_CORE => Ok(Self::Core),
            CALLER_ROLE_TRADING => Ok(Self::Trading),
            _ => Err(Error::NonCanonical),
        }
    }
}

/// Physical rational representation action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RepresentationActionV2 {
    /// Materialize native claims and mint exact free shard atoms.
    Denominate = ACTION_DENOMINATE,
    /// Burn exact free shard atoms and dematerialize native claims.
    Reconstitute = ACTION_RECONSTITUTE,
    /// Transfer coefficient shards into custody and mint receipt atoms.
    IssueStructured = ACTION_ISSUE_STRUCTURED,
    /// Burn receipt atoms and return coefficient shards from custody.
    UnwrapStructured = ACTION_UNWRAP_STRUCTURED,
    /// Burn one exact denominator of terminal shards, redeem Claims, and pay Custody.
    RedeemTerminal = ACTION_REDEEM_TERMINAL,
}

impl RepresentationActionV2 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            ACTION_DENOMINATE => Ok(Self::Denominate),
            ACTION_RECONSTITUTE => Ok(Self::Reconstitute),
            ACTION_ISSUE_STRUCTURED => Ok(Self::IssueStructured),
            ACTION_UNWRAP_STRUCTURED => Ok(Self::UnwrapStructured),
            ACTION_REDEEM_TERMINAL => Ok(Self::RedeemTerminal),
            _ => Err(Error::NonCanonical),
        }
    }

    /// Whether this action selects exactly one Product outcome.
    pub const fn selected_outcome(self) -> bool {
        matches!(
            self,
            Self::Denominate | Self::Reconstitute | Self::RedeemTerminal
        )
    }

    /// Whether this action executes one canonical Claims economic plan.
    pub const fn uses_claims(self) -> bool {
        self.selected_outcome()
    }

    /// Header class this action's request is encoded in.
    pub const fn class(self) -> RequestClassV3 {
        match self {
            Self::IssueStructured | Self::UnwrapStructured => RequestClassV3::Structured,
            Self::Denominate | Self::Reconstitute => RequestClassV3::Selected,
            Self::RedeemTerminal => RequestClassV3::Terminal,
        }
    }
}

/// Action class owning one physical request header layout.
///
/// Version three sends only the fields an action can vary.  A field this class
/// omits is forced to a constant by [`RepresentationRequestV2::validate`], so
/// omitting it removes no information: the decoder restores the constant and
/// every relation is still checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestClassV3 {
    /// Structured issue and unwrap: carries the actor receipt Account.
    Structured,
    /// Denominate and Reconstitute: carries the Claims and Position revisions.
    Selected,
    /// Terminal redemption: carries the Realm, recipient and Custody revisions.
    Terminal,
}

impl RequestClassV3 {
    /// Exact header width on the wire for this class.
    pub const fn header_bytes(self) -> usize {
        match self {
            Self::Structured => REQUEST_STRUCTURED_HEADER_BYTES_V3,
            Self::Selected => REQUEST_SELECTED_HEADER_BYTES_V3,
            Self::Terminal => REQUEST_TERMINAL_HEADER_BYTES_V3,
        }
    }

    /// Offset of this class's canonically zero reserved tail.
    const fn reserved_tail_offset(self) -> usize {
        match self {
            Self::Structured => STRUCTURED_REQUEST_RESERVED_TAIL_OFFSET_V3,
            Self::Selected => SELECTED_REQUEST_RESERVED_TAIL_OFFSET_V3,
            Self::Terminal => TERMINAL_REQUEST_RESERVED_TAIL_OFFSET_V3,
        }
    }
}

/// Fixed semantic header used to construct one request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepresentationRequestHeaderV2 {
    /// Action selected by the caller.
    pub action: RepresentationActionV2,
    /// Registry-authenticated caller role.
    pub caller_role: CallerRoleV2,
    /// Immutable execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Finalized representation graph content identity.
    pub graph_id: [u8; 32],
    /// Immutable rational representation descriptor.
    pub descriptor_id: [u8; 32],
    /// Complete upstream packet digest/replay context.
    pub parent_context: [u8; 32],
    /// Token holder and Claims Position owner.
    pub actor: [u8; 32],
    /// Structured receipt Mint, even for a selected shard action.
    pub receipt_mint: [u8; 32],
    /// Actor receipt Account for Structured actions, otherwise zero.
    pub receipt_account: [u8; 32],
    /// Claims-owned representation/token authority.
    pub representation_authority: [u8; 32],
    /// Exact Token or Token-2022 program.
    pub token_program: [u8; 32],
    /// Immutable Realm identity for terminal action, otherwise zero.
    pub realm: [u8; 32],
    /// Actor collateral recipient for terminal action, otherwise zero.
    pub collateral_recipient: [u8; 32],
    /// Exact representation replay revision.
    pub expected_representation_revision: u64,
    /// Claims aggregate revision or absent sentinel.
    pub expected_claims_market_revision: u64,
    /// Actor Claims Position revision or absent sentinel.
    pub expected_actor_position_revision: u64,
    /// Shard-custody Claims Position revision or absent sentinel.
    pub expected_custody_position_revision: u64,
    /// Custody replay revision for positive terminal payout, otherwise absent.
    pub expected_custody_replay_revision: u64,
    /// Market generation.
    pub generation: u64,
    /// Exact native claims or receipt atoms.
    pub quantity: u64,
    /// Exact shard denominator.
    pub denominator: u64,
    /// Token-owned receipt Mint supply before execution.
    pub expected_receipt_supply: u64,
    /// Product-owned outcome width.
    pub outcome_count: u32,
    /// Selected Product outcome or `u32::MAX` for Structured actions.
    pub selected_outcome: u32,
    /// Dynamic asset rows, one or the exact Product width.
    pub asset_count: u32,
}

/// The three per-coordinate program addresses the Claims adapter DERIVES from
/// `(program_id, descriptor_id, outcome)`.
///
/// Version two inlined all three in every asset row and then required them to
/// equal the derivation, so the wire copy authenticated nothing the chain had
/// not already computed.  Version three sends none of them and the adapter's
/// derivation is their only author; this struct is how a resolved row receives
/// them.  Their pairwise distinctness across rows, which version two checked
/// on the wire, is now a consequence of `find_program_address` injectivity
/// over distinct outcome seeds.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CoordinateIdentitiesV3 {
    /// Token-owned shard Mint.
    pub shard_mint: [u8; 32],
    /// Structured shard-custody Token Account.
    pub structured_custody_account: [u8; 32],
    /// Canonical Claims Position owner holding materialized native backing.
    pub claims_custody_owner: [u8; 32],
}

/// Exactly what one asset row puts ON THE WIRE in version three.
///
/// The actor's shard Account is the one key a caller chooses, so it is the one
/// key still sent; everything else in the row is a balance or a coefficient.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssetRowV2 {
    /// Actor's shard Token Account.
    pub actor_shard_account: [u8; 32],
    /// Shard atoms required per Structured receipt atom.
    pub coefficient: u64,
    /// Token shard Mint supply before execution.
    pub expected_shard_supply: u64,
    /// Actor shard balance before execution.
    pub expected_actor_shards: u64,
    /// Structured custody shard balance before execution.
    pub expected_structured_shards: u64,
}

impl AssetRowV2 {
    fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != ASSET_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        Ok(Self {
            actor_shard_account: require_nonzero(array_at(
                input,
                ASSET_ACTOR_SHARD_ACCOUNT_OFFSET_V3,
            )?)?,
            coefficient: u64_at(input, ASSET_COEFFICIENT_OFFSET_V3)?,
            expected_shard_supply: u64_at(input, ASSET_EXPECTED_SHARD_SUPPLY_OFFSET_V3)?,
            expected_actor_shards: u64_at(input, ASSET_EXPECTED_ACTOR_SHARDS_OFFSET_V3)?,
            expected_structured_shards: u64_at(input, ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET_V3)?,
        })
    }

    /// Restore a complete row by joining the adapter's derived identities.
    pub const fn resolve(self, identities: CoordinateIdentitiesV3) -> AssetV2 {
        AssetV2 {
            shard_mint: identities.shard_mint,
            actor_shard_account: self.actor_shard_account,
            structured_custody_account: identities.structured_custody_account,
            claims_custody_owner: identities.claims_custody_owner,
            coefficient: self.coefficient,
            expected_shard_supply: self.expected_shard_supply,
            expected_actor_shards: self.expected_actor_shards,
            expected_structured_shards: self.expected_structured_shards,
        }
    }
}

/// One complete asset row in memory: the wire row joined to the identities the
/// adapter derived for its coordinate.
///
/// This is the shape every consumer of a representation request reads and every
/// operator builds.  Only [`AssetRowV2`] crosses the wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssetV2 {
    /// Token-owned shard Mint. Derived; not sent.
    pub shard_mint: [u8; 32],
    /// Actor's shard Token Account.
    pub actor_shard_account: [u8; 32],
    /// Structured shard-custody Token Account. Derived; not sent.
    pub structured_custody_account: [u8; 32],
    /// Canonical Claims Position owner holding materialized native backing.
    /// Derived; not sent.
    pub claims_custody_owner: [u8; 32],
    /// Shard atoms required per Structured receipt atom.
    pub coefficient: u64,
    /// Token shard Mint supply before execution.
    pub expected_shard_supply: u64,
    /// Actor shard balance before execution.
    pub expected_actor_shards: u64,
    /// Structured custody shard balance before execution.
    pub expected_structured_shards: u64,
}

impl AssetV2 {
    /// The bytes this row contributes to the request tail.
    pub const fn row(self) -> AssetRowV2 {
        AssetRowV2 {
            actor_shard_account: self.actor_shard_account,
            coefficient: self.coefficient,
            expected_shard_supply: self.expected_shard_supply,
            expected_actor_shards: self.expected_actor_shards,
            expected_structured_shards: self.expected_structured_shards,
        }
    }

    /// Encode one canonical row.
    ///
    /// The three derived identities are still REQUIRED to be present and
    /// distinct in the caller's row -- an operator that cannot name them has
    /// not derived the coordinate -- and are then not written, because the
    /// adapter derives them again and would only compare them to itself.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() != ASSET_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        require_nonzero(self.shard_mint)?;
        require_nonzero(self.actor_shard_account)?;
        require_nonzero(self.structured_custody_account)?;
        require_nonzero(self.claims_custody_owner)?;
        if self.shard_mint == self.actor_shard_account
            || self.shard_mint == self.structured_custody_account
            || self.actor_shard_account == self.structured_custody_account
        {
            return Err(Error::AccountAlias);
        }
        output.fill(0);
        put(
            output,
            ASSET_ACTOR_SHARD_ACCOUNT_OFFSET_V3,
            &self.actor_shard_account,
        )?;
        for (offset, value) in [
            (ASSET_COEFFICIENT_OFFSET_V3, self.coefficient),
            (
                ASSET_EXPECTED_SHARD_SUPPLY_OFFSET_V3,
                self.expected_shard_supply,
            ),
            (
                ASSET_EXPECTED_ACTOR_SHARDS_OFFSET_V3,
                self.expected_actor_shards,
            ),
            (
                ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET_V3,
                self.expected_structured_shards,
            ),
        ] {
            put(output, offset, &value.to_le_bytes())?;
        }
        AssetRowV2::decode(output).map(|_| ())
    }
}

/// Borrowed exact variable-width request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepresentationRequestV2<'a> {
    header: RepresentationRequestHeaderV2,
    assets: &'a [u8],
}

impl<'a> RepresentationRequestV2<'a> {
    /// Decode and fully canonicalize one exact request.
    ///
    /// The action selects the header class, so the wire length is a function
    /// of the action and the outcome count rather than of a sent `assetCount`.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        if input.len() < REQUEST_COMMON_PREFIX_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if array_at::<8>(input, REQUEST_MAGIC_OFFSET_V3)? != REQUEST_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if u16_at(input, REQUEST_VERSION_OFFSET_V3)? != PHYSICAL_ABI_VERSION_V3 {
            return Err(Error::UnsupportedVersion);
        }
        let action = RepresentationActionV2::decode(byte_at(input, REQUEST_ACTION_OFFSET_V3)?)?;
        let class = action.class();
        let header_bytes = class.header_bytes();
        if input.len() < header_bytes {
            return Err(Error::InvalidLength);
        }
        require_zero(input, REQUEST_RESERVED_HEADER_OFFSET_V3, 4)?;
        require_zero(input, class.reserved_tail_offset(), 4)?;
        let outcome_count = u32_at(input, REQUEST_OUTCOME_COUNT_OFFSET_V3)?;
        // DERIVED, not sent: Structured actions carry the complete outcome set
        // (`asset_count == outcome_count`, the exhaustiveness rule that lets
        // issuance mint), every selected-outcome action carries exactly one.
        let asset_count = if action.selected_outcome() { 1 } else { outcome_count };
        let tail = usize::try_from(asset_count)
            .map_err(|_| Error::InvalidWidth)?
            .checked_mul(ASSET_BYTES_V3)
            .ok_or(Error::InvalidLength)?;
        if input.len() != header_bytes.checked_add(tail).ok_or(Error::InvalidLength)? {
            return Err(Error::InvalidLength);
        }
        let receipt_account = match class {
            RequestClassV3::Structured => require_nonzero(array_at(
                input,
                STRUCTURED_REQUEST_RECEIPT_ACCOUNT_OFFSET_V3,
            )?)?,
            RequestClassV3::Selected | RequestClassV3::Terminal => [0_u8; 32],
        };
        let (realm, collateral_recipient) = match class {
            RequestClassV3::Terminal => (
                require_nonzero(array_at(input, TERMINAL_REQUEST_REALM_OFFSET_V3)?)?,
                require_nonzero(array_at(
                    input,
                    TERMINAL_REQUEST_COLLATERAL_RECIPIENT_OFFSET_V3,
                )?)?,
            ),
            RequestClassV3::Structured | RequestClassV3::Selected => ([0_u8; 32], [0_u8; 32]),
        };
        let (claims_market, actor_position, custody_position, custody_replay, selected_outcome) =
            match class {
                RequestClassV3::Structured => (
                    ABSENT_REVISION,
                    ABSENT_REVISION,
                    ABSENT_REVISION,
                    ABSENT_REVISION,
                    u32::MAX,
                ),
                RequestClassV3::Selected => (
                    u64_at(
                        input,
                        SELECTED_REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET_V3,
                    )?,
                    u64_at(
                        input,
                        SELECTED_REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET_V3,
                    )?,
                    u64_at(
                        input,
                        SELECTED_REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET_V3,
                    )?,
                    ABSENT_REVISION,
                    u32_at(input, SELECTED_REQUEST_SELECTED_OUTCOME_OFFSET_V3)?,
                ),
                RequestClassV3::Terminal => (
                    u64_at(
                        input,
                        TERMINAL_REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET_V3,
                    )?,
                    ABSENT_REVISION,
                    u64_at(
                        input,
                        TERMINAL_REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET_V3,
                    )?,
                    u64_at(
                        input,
                        TERMINAL_REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET_V3,
                    )?,
                    u32_at(input, TERMINAL_REQUEST_SELECTED_OUTCOME_OFFSET_V3)?,
                ),
            };
        let value = Self {
            header: RepresentationRequestHeaderV2 {
                action,
                caller_role: CallerRoleV2::decode(byte_at(input, REQUEST_CALLER_ROLE_OFFSET_V3)?)?,
                release_set: require_nonzero(array_at(input, REQUEST_RELEASE_SET_OFFSET_V3)?)?,
                market: require_nonzero(array_at(input, REQUEST_MARKET_OFFSET_V3)?)?,
                graph_id: require_nonzero(array_at(input, REQUEST_GRAPH_ID_OFFSET_V3)?)?,
                descriptor_id: require_nonzero(array_at(input, REQUEST_DESCRIPTOR_ID_OFFSET_V3)?)?,
                parent_context: require_nonzero(array_at(
                    input,
                    REQUEST_PARENT_CONTEXT_OFFSET_V3,
                )?)?,
                actor: require_nonzero(array_at(input, REQUEST_ACTOR_OFFSET_V3)?)?,
                receipt_mint: require_nonzero(array_at(input, REQUEST_RECEIPT_MINT_OFFSET_V3)?)?,
                receipt_account,
                representation_authority: require_nonzero(array_at(
                    input,
                    REQUEST_REPRESENTATION_AUTHORITY_OFFSET_V3,
                )?)?,
                token_program: require_nonzero(array_at(input, REQUEST_TOKEN_PROGRAM_OFFSET_V3)?)?,
                realm,
                collateral_recipient,
                expected_representation_revision: u64_at(
                    input,
                    REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET_V3,
                )?,
                expected_claims_market_revision: claims_market,
                expected_actor_position_revision: actor_position,
                expected_custody_position_revision: custody_position,
                expected_custody_replay_revision: custody_replay,
                generation: u64_at(input, REQUEST_GENERATION_OFFSET_V3)?,
                quantity: u64_at(input, REQUEST_QUANTITY_OFFSET_V3)?,
                denominator: u64_at(input, REQUEST_DENOMINATOR_OFFSET_V3)?,
                expected_receipt_supply: u64_at(
                    input,
                    REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET_V3,
                )?,
                outcome_count,
                selected_outcome,
                asset_count,
            },
            assets: subslice(input, header_bytes, tail)?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Construct one borrowed request and validate every action shape.
    pub fn new(header: RepresentationRequestHeaderV2, assets: &'a [u8]) -> Result<Self> {
        let value = Self { header, assets };
        value.validate()?;
        Ok(value)
    }

    /// Exact wire width of this request.
    pub fn wire_len(self) -> Result<usize> {
        self.header
            .action
            .class()
            .header_bytes()
            .checked_add(self.assets.len())
            .ok_or(Error::InvalidLength)
    }

    /// Encode into the exact caller-owned variable-width output.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        let class = self.header.action.class();
        let header_bytes = class.header_bytes();
        if output.len() != self.wire_len()? {
            return Err(Error::InvalidLength);
        }
        output.fill(0);
        put(output, REQUEST_MAGIC_OFFSET_V3, &REQUEST_MAGIC_V2)?;
        put(
            output,
            REQUEST_VERSION_OFFSET_V3,
            &PHYSICAL_ABI_VERSION_V3.to_le_bytes(),
        )?;
        put_byte(output, REQUEST_ACTION_OFFSET_V3, self.header.action as u8)?;
        put_byte(
            output,
            REQUEST_CALLER_ROLE_OFFSET_V3,
            self.header.caller_role as u8,
        )?;
        for (offset, value) in [
            (REQUEST_RELEASE_SET_OFFSET_V3, self.header.release_set),
            (REQUEST_MARKET_OFFSET_V3, self.header.market),
            (REQUEST_GRAPH_ID_OFFSET_V3, self.header.graph_id),
            (REQUEST_DESCRIPTOR_ID_OFFSET_V3, self.header.descriptor_id),
            (REQUEST_PARENT_CONTEXT_OFFSET_V3, self.header.parent_context),
            (REQUEST_ACTOR_OFFSET_V3, self.header.actor),
            (REQUEST_RECEIPT_MINT_OFFSET_V3, self.header.receipt_mint),
            (
                REQUEST_REPRESENTATION_AUTHORITY_OFFSET_V3,
                self.header.representation_authority,
            ),
            (REQUEST_TOKEN_PROGRAM_OFFSET_V3, self.header.token_program),
        ] {
            put(output, offset, &value)?;
        }
        for (offset, value) in [
            (
                REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET_V3,
                self.header.expected_representation_revision,
            ),
            (REQUEST_GENERATION_OFFSET_V3, self.header.generation),
            (REQUEST_QUANTITY_OFFSET_V3, self.header.quantity),
            (REQUEST_DENOMINATOR_OFFSET_V3, self.header.denominator),
            (
                REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET_V3,
                self.header.expected_receipt_supply,
            ),
        ] {
            put(output, offset, &value.to_le_bytes())?;
        }
        put(
            output,
            REQUEST_OUTCOME_COUNT_OFFSET_V3,
            &self.header.outcome_count.to_le_bytes(),
        )?;
        // The class tail. Every field this class omits is forced to a constant
        // by `validate`, which ran above, so nothing written here is lost and
        // nothing omitted here was free to differ.
        match class {
            RequestClassV3::Structured => {
                put(
                    output,
                    STRUCTURED_REQUEST_RECEIPT_ACCOUNT_OFFSET_V3,
                    &self.header.receipt_account,
                )?;
            }
            RequestClassV3::Selected => {
                for (offset, value) in [
                    (
                        SELECTED_REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET_V3,
                        self.header.expected_claims_market_revision,
                    ),
                    (
                        SELECTED_REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET_V3,
                        self.header.expected_actor_position_revision,
                    ),
                    (
                        SELECTED_REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET_V3,
                        self.header.expected_custody_position_revision,
                    ),
                ] {
                    put(output, offset, &value.to_le_bytes())?;
                }
                put(
                    output,
                    SELECTED_REQUEST_SELECTED_OUTCOME_OFFSET_V3,
                    &self.header.selected_outcome.to_le_bytes(),
                )?;
            }
            RequestClassV3::Terminal => {
                put(output, TERMINAL_REQUEST_REALM_OFFSET_V3, &self.header.realm)?;
                put(
                    output,
                    TERMINAL_REQUEST_COLLATERAL_RECIPIENT_OFFSET_V3,
                    &self.header.collateral_recipient,
                )?;
                for (offset, value) in [
                    (
                        TERMINAL_REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET_V3,
                        self.header.expected_claims_market_revision,
                    ),
                    (
                        TERMINAL_REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET_V3,
                        self.header.expected_custody_position_revision,
                    ),
                    (
                        TERMINAL_REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET_V3,
                        self.header.expected_custody_replay_revision,
                    ),
                ] {
                    put(output, offset, &value.to_le_bytes())?;
                }
                put(
                    output,
                    TERMINAL_REQUEST_SELECTED_OUTCOME_OFFSET_V3,
                    &self.header.selected_outcome.to_le_bytes(),
                )?;
            }
        }
        put(output, header_bytes, self.assets)
    }

    /// Return the fixed semantic header.
    pub const fn header(self) -> RepresentationRequestHeaderV2 {
        self.header
    }

    /// Read one canonical asset row exactly as it arrived on the wire.
    pub fn asset_row(self, index: u32) -> Result<AssetRowV2> {
        if index >= self.header.asset_count {
            return Err(Error::InvalidWidth);
        }
        let offset = usize::try_from(index)
            .map_err(|_| Error::InvalidWidth)?
            .checked_mul(ASSET_BYTES_V3)
            .ok_or(Error::InvalidLength)?;
        AssetRowV2::decode(subslice(self.assets, offset, ASSET_BYTES_V3)?)
    }

    /// Borrow the exact dynamic asset rows.
    pub const fn asset_bytes(self) -> &'a [u8] {
        self.assets
    }

    /// Return the exact Claims adapter account-frame width for this request.
    ///
    /// Selected actions carry one physical asset row regardless of Product
    /// width. Structured issue and unwrap carry their full `N` rows. Positive
    /// terminal redemption appends the canonical Custody suffix.
    pub fn physical_account_count(self) -> Result<usize> {
        crate::REPRESENTATION_FRAME_SPEC_V2.account_count(self)
    }

    fn validate(self) -> Result<()> {
        for identity in [
            self.header.release_set,
            self.header.market,
            self.header.graph_id,
            self.header.descriptor_id,
            self.header.parent_context,
            self.header.actor,
            self.header.receipt_mint,
            self.header.representation_authority,
            self.header.token_program,
        ] {
            require_nonzero(identity)?;
        }
        TokenProgram::parse(self.header.token_program).map_err(|_| Error::InvalidActionShape)?;
        if self.header.quantity == 0
            || self.header.denominator == 0
            || self.header.outcome_count == 0
            || self.header.asset_count == 0
            || self.header.expected_representation_revision == u64::MAX
        {
            return Err(Error::InvalidActionShape);
        }
        self.header
            .expected_representation_revision
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)?;
        let selected = self.header.action.selected_outcome();
        if selected {
            if self.header.asset_count != 1
                || self.header.selected_outcome >= self.header.outcome_count
            {
                return Err(Error::InvalidActionShape);
            }
        } else if self.header.asset_count != self.header.outcome_count
            || self.header.selected_outcome != u32::MAX
        {
            return Err(Error::InvalidActionShape);
        }
        let terminal = self.header.action == RepresentationActionV2::RedeemTerminal;
        let structured = matches!(
            self.header.action,
            RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
        );
        if structured != !is_zero(self.header.receipt_account)
            || terminal != !is_zero(self.header.realm)
            || terminal != !is_zero(self.header.collateral_recipient)
        {
            return Err(Error::InvalidActionShape);
        }
        let claims = self.header.action.uses_claims();
        let claims_market = self.header.expected_claims_market_revision != ABSENT_REVISION;
        let actor_position = self.header.expected_actor_position_revision != ABSENT_REVISION;
        let custody_position = self.header.expected_custody_position_revision != ABSENT_REVISION;
        let custody_replay = self.header.expected_custody_replay_revision != ABSENT_REVISION;
        let revisions_valid = match self.header.action {
            RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute => {
                claims_market && actor_position && custody_position && !custody_replay
            }
            RepresentationActionV2::RedeemTerminal => {
                // Exact payout is derived only after Product/basis terminal
                // evaluation. Custody replay is therefore present for a
                // positive payout and absent for a zero payout; completion
                // evidence closes that shape.
                claims_market && !actor_position && custody_position
            }
            RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured => {
                !claims_market && !actor_position && !custody_position && !custody_replay
            }
        };
        if !revisions_valid || claims != claims_market {
            return Err(Error::InvalidActionShape);
        }
        if self.assets.len()
            != usize::try_from(self.header.asset_count)
                .map_err(|_| Error::InvalidWidth)?
                .checked_mul(ASSET_BYTES_V3)
                .ok_or(Error::InvalidLength)?
        {
            return Err(Error::InvalidLength);
        }
        let mut index = 0_u32;
        while index < self.header.asset_count {
            let asset = self.asset_row(index)?;
            // The three derived keys left the wire, and with them the alias
            // checks that read them. The actor Account did not: it is the one
            // key a caller chooses, so it stays checked against the receipt
            // pair and pairwise across rows. Distinctness of the derived
            // triple is now `find_program_address` injectivity over distinct
            // outcome seeds, discharged where the adapter derives them.
            if asset.actor_shard_account == self.header.receipt_account
                || asset.actor_shard_account == self.header.receipt_mint
            {
                return Err(Error::AccountAlias);
            }
            let mut prior = 0_u32;
            while prior < index {
                let left = self.asset_row(prior)?;
                if asset.actor_shard_account == left.actor_shard_account {
                    return Err(Error::AccountAlias);
                }
                prior = prior.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }
}

/// One decoded request joined to the identities its adapter derived.
///
/// Version three does not send the three per-coordinate program addresses, so
/// a bare [`RepresentationRequestV2`] cannot answer what a coordinate's shard
/// Mint is -- only the adapter that derived it can.  Every consumer that reads
/// a complete asset row takes this type instead, which makes "I read an
/// identity nobody derived" a type error rather than a zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedRequestV2<'a> {
    request: RepresentationRequestV2<'a>,
    identities: IdentitySourceV3<'a>,
}

/// Where a resolved request reads its derived identities.
///
/// Every selected-outcome action carries exactly one coordinate, and its
/// caller already holds that coordinate's identities by value; borrowing a
/// slice for one element would force each such caller to own a buffer whose
/// only purpose is to be borrowed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdentitySourceV3<'a> {
    One(CoordinateIdentitiesV3),
    Many(&'a [CoordinateIdentitiesV3]),
}

impl IdentitySourceV3<'_> {
    fn len(self) -> usize {
        match self {
            Self::One(_) => 1,
            Self::Many(values) => values.len(),
        }
    }

    fn get(self, index: usize) -> Option<CoordinateIdentitiesV3> {
        match self {
            Self::One(value) => (index == 0).then_some(value),
            Self::Many(values) => values.get(index).copied(),
        }
    }
}

impl<'a> ResolvedRequestV2<'a> {
    /// Join derived identities to a decoded request, one per asset row.
    pub fn new(
        request: RepresentationRequestV2<'a>,
        identities: &'a [CoordinateIdentitiesV3],
    ) -> Result<Self> {
        Self::join(request, IdentitySourceV3::Many(identities))
    }

    /// Join the single coordinate of a selected-outcome action.
    pub fn selected(
        request: RepresentationRequestV2<'a>,
        identity: CoordinateIdentitiesV3,
    ) -> Result<Self> {
        Self::join(request, IdentitySourceV3::One(identity))
    }

    fn join(
        request: RepresentationRequestV2<'a>,
        identities: IdentitySourceV3<'a>,
    ) -> Result<Self> {
        if identities.len()
            != usize::try_from(request.header().asset_count).map_err(|_| Error::InvalidWidth)?
        {
            return Err(Error::InvalidWidth);
        }
        let mut index = 0;
        while index < identities.len() {
            let identity = identities.get(index).ok_or(Error::InvalidWidth)?;
            require_nonzero(identity.shard_mint)?;
            require_nonzero(identity.structured_custody_account)?;
            require_nonzero(identity.claims_custody_owner)?;
            // The wire no longer carries these, so the wire-level distinctness
            // check went with them; the cheap explicit one survives here, where
            // the derived set is finally in one place. It is redundant against
            // `find_program_address` injectivity over distinct outcome seeds
            // and costs one pass over K.
            let mut prior = 0;
            while prior < index {
                let left = identities.get(prior).ok_or(Error::InvalidWidth)?;
                if identity.shard_mint == left.shard_mint
                    || identity.structured_custody_account == left.structured_custody_account
                    || identity.claims_custody_owner == left.claims_custody_owner
                {
                    return Err(Error::AccountAlias);
                }
                prior += 1;
            }
            index += 1;
        }
        Ok(Self {
            request,
            identities,
        })
    }

    /// The underlying wire request.
    pub const fn request(self) -> RepresentationRequestV2<'a> {
        self.request
    }

    /// Fixed semantic header.
    pub const fn header(self) -> RepresentationRequestHeaderV2 {
        self.request.header()
    }

    /// Borrow the exact dynamic asset row bytes.
    pub const fn asset_bytes(self) -> &'a [u8] {
        self.request.asset_bytes()
    }

    /// Exact Claims adapter account-frame width for this request.
    pub fn physical_account_count(self) -> Result<usize> {
        self.request.physical_account_count()
    }

    /// One complete asset row: the wire row joined to its derived identities.
    pub fn asset(self, index: u32) -> Result<AssetV2> {
        let row = self.request.asset_row(index)?;
        let identity = self
            .identities
            .get(usize::try_from(index).map_err(|_| Error::InvalidWidth)?)
            .ok_or(Error::InvalidWidth)?;
        Ok(row.resolve(identity))
    }
}

#[cfg(test)]
mod physical_abi_v3_width {
    extern crate alloc;
    use crate::*;
    use crate::generated::REQUEST_VERSION_OFFSET_V3;

    fn id(seed: u8) -> [u8; 32] {
        [seed; 32]
    }

    fn header(action: RepresentationActionV2, assets: u32) -> RepresentationRequestHeaderV2 {
        let terminal = action == RepresentationActionV2::RedeemTerminal;
        let structured = matches!(
            action,
            RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
        );
        RepresentationRequestHeaderV2 {
            action,
            caller_role: CallerRoleV2::Trading,
            release_set: id(1),
            market: id(2),
            graph_id: id(3),
            descriptor_id: id(4),
            parent_context: id(5),
            actor: id(6),
            receipt_mint: id(7),
            receipt_account: if structured { id(8) } else { [0; 32] },
            representation_authority: id(9),
            token_program: dclutch_token_svm::TokenProgram::Token2022.program_id(),
            realm: if terminal { id(11) } else { [0; 32] },
            collateral_recipient: if terminal { id(12) } else { [0; 32] },
            expected_representation_revision: 4,
            expected_claims_market_revision: if structured { ABSENT_REVISION } else { 5 },
            expected_actor_position_revision: if structured || terminal {
                ABSENT_REVISION
            } else {
                6
            },
            expected_custody_position_revision: if structured { ABSENT_REVISION } else { 7 },
            expected_custody_replay_revision: if terminal { 8 } else { ABSENT_REVISION },
            generation: 1,
            quantity: 2,
            denominator: 3,
            expected_receipt_supply: 9,
            outcome_count: 3,
            selected_outcome: if structured { u32::MAX } else { 0 },
            asset_count: assets,
        }
    }

    fn rows(count: u32) -> alloc::vec::Vec<u8> {
        let mut bytes = alloc::vec![0_u8; count as usize * ASSET_BYTES_V3];
        for index in 0..count {
            AssetV2 {
                shard_mint: id(100 + index as u8),
                actor_shard_account: id(140 + index as u8),
                structured_custody_account: id(180 + index as u8),
                claims_custody_owner: id(220 + index as u8),
                coefficient: 1,
                expected_shard_supply: 10,
                expected_actor_shards: 10,
                expected_structured_shards: 10,
            }
            .encode_into(
                &mut bytes[index as usize * ASSET_BYTES_V3..(index as usize + 1) * ASSET_BYTES_V3],
            )
            .expect("row");
        }
        bytes
    }

    fn encoded(action: RepresentationActionV2, assets: u32) -> alloc::vec::Vec<u8> {
        let tail = rows(assets);
        let request = RepresentationRequestV2::new(header(action, assets), &tail).expect("request");
        let mut out = alloc::vec![0_u8; request.wire_len().expect("len")];
        request.encode_into(&mut out).expect("encode");
        RepresentationRequestV2::decode(&out).expect("round trip");
        out
    }

    // THE MEASUREMENT the packet ruling rests on. Version two sent one 488-byte
    // header and 160-byte rows for every action.
    #[test]
    fn structured_full_width_request_is_three_hundred_ninety_two_bytes_smaller_at_k_three() {
        let v2 = 488 + 3 * 160;
        let v3 = encoded(RepresentationActionV2::IssueStructured, 3).len();
        assert_eq!((v3, v2 - v3), (576, 392));
    }

    #[test]
    fn selected_request_is_two_hundred_four_bytes_smaller() {
        let v2 = 488 + 160;
        let v3 = encoded(RepresentationActionV2::Denominate, 1).len();
        assert_eq!((v3, v2 - v3), (444, 204));
    }

    #[test]
    fn terminal_request_is_one_hundred_forty_bytes_smaller() {
        let v2 = 488 + 160;
        let v3 = encoded(RepresentationActionV2::RedeemTerminal, 1).len();
        assert_eq!((v3, v2 - v3), (508, 140));
    }

    // The negative control: an old-shaped request refuses by a NAMED code, and
    // by the version rather than by an accidental length coincidence.
    #[test]
    fn a_version_two_request_refuses_by_unsupported_version() {
        let mut bytes = encoded(RepresentationActionV2::IssueStructured, 3);
        bytes[REQUEST_VERSION_OFFSET_V3] = 2;
        assert_eq!(
            RepresentationRequestV2::decode(&bytes).unwrap_err(),
            Error::UnsupportedVersion
        );
    }

    // And a request carrying the version-two 160-byte asset shape refuses at
    // decode on length, which is what a stale producer actually sends.
    #[test]
    fn a_version_two_asset_shape_refuses_by_length() {
        let good = encoded(RepresentationActionV2::IssueStructured, 3);
        let mut stale = good.clone();
        stale.extend_from_slice(&[0_u8; 3 * (160 - 64)]);
        assert_eq!(
            RepresentationRequestV2::decode(&stale).unwrap_err(),
            Error::InvalidLength
        );
    }
}
