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

/// One exact asset row in the request tail.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AssetV2 {
    /// Token-owned shard Mint.
    pub shard_mint: [u8; 32],
    /// Actor's shard Token Account.
    pub actor_shard_account: [u8; 32],
    /// Structured shard-custody Token Account.
    pub structured_custody_account: [u8; 32],
    /// Canonical Claims Position owner holding materialized native backing.
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
    fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != ASSET_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        let value = Self {
            shard_mint: require_nonzero(array_at(input, ASSET_SHARD_MINT_OFFSET)?)?,
            actor_shard_account: require_nonzero(array_at(
                input,
                ASSET_ACTOR_SHARD_ACCOUNT_OFFSET,
            )?)?,
            structured_custody_account: require_nonzero(array_at(
                input,
                ASSET_STRUCTURED_CUSTODY_ACCOUNT_OFFSET,
            )?)?,
            claims_custody_owner: require_nonzero(array_at(
                input,
                ASSET_CLAIMS_CUSTODY_OWNER_OFFSET,
            )?)?,
            coefficient: u64_at(input, ASSET_COEFFICIENT_OFFSET)?,
            expected_shard_supply: u64_at(input, ASSET_EXPECTED_SHARD_SUPPLY_OFFSET)?,
            expected_actor_shards: u64_at(input, ASSET_EXPECTED_ACTOR_SHARDS_OFFSET)?,
            expected_structured_shards: u64_at(input, ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET)?,
        };
        if value.shard_mint == value.actor_shard_account
            || value.shard_mint == value.structured_custody_account
            || value.actor_shard_account == value.structured_custody_account
        {
            return Err(Error::AccountAlias);
        }
        Ok(value)
    }

    /// Encode one canonical row.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() != ASSET_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        require_nonzero(self.shard_mint)?;
        require_nonzero(self.actor_shard_account)?;
        require_nonzero(self.structured_custody_account)?;
        require_nonzero(self.claims_custody_owner)?;
        output.fill(0);
        put(output, ASSET_SHARD_MINT_OFFSET, &self.shard_mint)?;
        put(
            output,
            ASSET_ACTOR_SHARD_ACCOUNT_OFFSET,
            &self.actor_shard_account,
        )?;
        put(
            output,
            ASSET_STRUCTURED_CUSTODY_ACCOUNT_OFFSET,
            &self.structured_custody_account,
        )?;
        put(
            output,
            ASSET_CLAIMS_CUSTODY_OWNER_OFFSET,
            &self.claims_custody_owner,
        )?;
        for (offset, value) in [
            (ASSET_COEFFICIENT_OFFSET, self.coefficient),
            (
                ASSET_EXPECTED_SHARD_SUPPLY_OFFSET,
                self.expected_shard_supply,
            ),
            (
                ASSET_EXPECTED_ACTOR_SHARDS_OFFSET,
                self.expected_actor_shards,
            ),
            (
                ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET,
                self.expected_structured_shards,
            ),
        ] {
            put(output, offset, &value.to_le_bytes())?;
        }
        Self::decode(output).map(|_| ())
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
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        if input.len() < REQUEST_HEADER_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if array_at::<8>(input, REQUEST_MAGIC_OFFSET)? != REQUEST_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if u16_at(input, REQUEST_VERSION_OFFSET)? != PHYSICAL_ABI_VERSION_V2 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, REQUEST_RESERVED_HEADER_OFFSET, 4)?;
        require_zero(input, REQUEST_RESERVED_TAIL_OFFSET, 4)?;
        let asset_count = u32_at(input, REQUEST_ASSET_COUNT_OFFSET)?;
        let tail = usize::try_from(asset_count)
            .map_err(|_| Error::InvalidWidth)?
            .checked_mul(ASSET_BYTES_V2)
            .ok_or(Error::InvalidLength)?;
        if input.len()
            != REQUEST_HEADER_BYTES_V2
                .checked_add(tail)
                .ok_or(Error::InvalidLength)?
        {
            return Err(Error::InvalidLength);
        }
        let value = Self {
            header: RepresentationRequestHeaderV2 {
                action: RepresentationActionV2::decode(byte_at(input, REQUEST_ACTION_OFFSET)?)?,
                caller_role: CallerRoleV2::decode(byte_at(input, REQUEST_CALLER_ROLE_OFFSET)?)?,
                release_set: require_nonzero(array_at(input, REQUEST_RELEASE_SET_OFFSET)?)?,
                market: require_nonzero(array_at(input, REQUEST_MARKET_OFFSET)?)?,
                graph_id: require_nonzero(array_at(input, REQUEST_GRAPH_ID_OFFSET)?)?,
                descriptor_id: require_nonzero(array_at(input, REQUEST_DESCRIPTOR_ID_OFFSET)?)?,
                parent_context: require_nonzero(array_at(input, REQUEST_PARENT_CONTEXT_OFFSET)?)?,
                actor: require_nonzero(array_at(input, REQUEST_ACTOR_OFFSET)?)?,
                receipt_mint: require_nonzero(array_at(input, REQUEST_RECEIPT_MINT_OFFSET)?)?,
                receipt_account: array_at(input, REQUEST_RECEIPT_ACCOUNT_OFFSET)?,
                representation_authority: require_nonzero(array_at(
                    input,
                    REQUEST_REPRESENTATION_AUTHORITY_OFFSET,
                )?)?,
                token_program: require_nonzero(array_at(input, REQUEST_TOKEN_PROGRAM_OFFSET)?)?,
                realm: array_at(input, REQUEST_REALM_OFFSET)?,
                collateral_recipient: array_at(input, REQUEST_COLLATERAL_RECIPIENT_OFFSET)?,
                expected_representation_revision: u64_at(
                    input,
                    REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET,
                )?,
                expected_claims_market_revision: u64_at(
                    input,
                    REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET,
                )?,
                expected_actor_position_revision: u64_at(
                    input,
                    REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET,
                )?,
                expected_custody_position_revision: u64_at(
                    input,
                    REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET,
                )?,
                expected_custody_replay_revision: u64_at(
                    input,
                    REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET,
                )?,
                generation: u64_at(input, REQUEST_GENERATION_OFFSET)?,
                quantity: u64_at(input, REQUEST_QUANTITY_OFFSET)?,
                denominator: u64_at(input, REQUEST_DENOMINATOR_OFFSET)?,
                expected_receipt_supply: u64_at(input, REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET)?,
                outcome_count: u32_at(input, REQUEST_OUTCOME_COUNT_OFFSET)?,
                selected_outcome: u32_at(input, REQUEST_SELECTED_OUTCOME_OFFSET)?,
                asset_count,
            },
            assets: subslice(input, REQUEST_HEADER_BYTES_V2, tail)?,
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

    /// Encode into the exact caller-owned variable-width output.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        self.validate()?;
        if output.len()
            != REQUEST_HEADER_BYTES_V2
                .checked_add(self.assets.len())
                .ok_or(Error::InvalidLength)?
        {
            return Err(Error::InvalidLength);
        }
        output.fill(0);
        put(output, REQUEST_MAGIC_OFFSET, &REQUEST_MAGIC_V2)?;
        put(
            output,
            REQUEST_VERSION_OFFSET,
            &PHYSICAL_ABI_VERSION_V2.to_le_bytes(),
        )?;
        put_byte(output, REQUEST_ACTION_OFFSET, self.header.action as u8)?;
        put_byte(
            output,
            REQUEST_CALLER_ROLE_OFFSET,
            self.header.caller_role as u8,
        )?;
        for (offset, value) in [
            (REQUEST_RELEASE_SET_OFFSET, self.header.release_set),
            (REQUEST_MARKET_OFFSET, self.header.market),
            (REQUEST_GRAPH_ID_OFFSET, self.header.graph_id),
            (REQUEST_DESCRIPTOR_ID_OFFSET, self.header.descriptor_id),
            (REQUEST_PARENT_CONTEXT_OFFSET, self.header.parent_context),
            (REQUEST_ACTOR_OFFSET, self.header.actor),
            (REQUEST_RECEIPT_MINT_OFFSET, self.header.receipt_mint),
            (REQUEST_RECEIPT_ACCOUNT_OFFSET, self.header.receipt_account),
            (
                REQUEST_REPRESENTATION_AUTHORITY_OFFSET,
                self.header.representation_authority,
            ),
            (REQUEST_TOKEN_PROGRAM_OFFSET, self.header.token_program),
            (REQUEST_REALM_OFFSET, self.header.realm),
            (
                REQUEST_COLLATERAL_RECIPIENT_OFFSET,
                self.header.collateral_recipient,
            ),
        ] {
            put(output, offset, &value)?;
        }
        for (offset, value) in [
            (
                REQUEST_EXPECTED_REPRESENTATION_REVISION_OFFSET,
                self.header.expected_representation_revision,
            ),
            (
                REQUEST_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET,
                self.header.expected_claims_market_revision,
            ),
            (
                REQUEST_EXPECTED_ACTOR_POSITION_REVISION_OFFSET,
                self.header.expected_actor_position_revision,
            ),
            (
                REQUEST_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET,
                self.header.expected_custody_position_revision,
            ),
            (
                REQUEST_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET,
                self.header.expected_custody_replay_revision,
            ),
            (REQUEST_GENERATION_OFFSET, self.header.generation),
            (REQUEST_QUANTITY_OFFSET, self.header.quantity),
            (REQUEST_DENOMINATOR_OFFSET, self.header.denominator),
            (
                REQUEST_EXPECTED_RECEIPT_SUPPLY_OFFSET,
                self.header.expected_receipt_supply,
            ),
        ] {
            put(output, offset, &value.to_le_bytes())?;
        }
        for (offset, value) in [
            (REQUEST_OUTCOME_COUNT_OFFSET, self.header.outcome_count),
            (
                REQUEST_SELECTED_OUTCOME_OFFSET,
                self.header.selected_outcome,
            ),
            (REQUEST_ASSET_COUNT_OFFSET, self.header.asset_count),
        ] {
            put(output, offset, &value.to_le_bytes())?;
        }
        put(output, REQUEST_HEADER_BYTES_V2, self.assets)
    }

    /// Return the fixed semantic header.
    pub const fn header(self) -> RepresentationRequestHeaderV2 {
        self.header
    }

    /// Read one canonical asset row.
    pub fn asset(self, index: u32) -> Result<AssetV2> {
        if index >= self.header.asset_count {
            return Err(Error::InvalidWidth);
        }
        let offset = usize::try_from(index)
            .map_err(|_| Error::InvalidWidth)?
            .checked_mul(ASSET_BYTES_V2)
            .ok_or(Error::InvalidLength)?;
        AssetV2::decode(subslice(self.assets, offset, ASSET_BYTES_V2)?)
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
                .checked_mul(ASSET_BYTES_V2)
                .ok_or(Error::InvalidLength)?
        {
            return Err(Error::InvalidLength);
        }
        let mut index = 0_u32;
        while index < self.header.asset_count {
            let asset = self.asset(index)?;
            if asset.actor_shard_account == self.header.receipt_account
                || asset.structured_custody_account == self.header.receipt_account
                || asset.shard_mint == self.header.receipt_mint
                || asset.actor_shard_account == self.header.receipt_mint
                || asset.structured_custody_account == self.header.receipt_mint
            {
                return Err(Error::AccountAlias);
            }
            let mut prior = 0_u32;
            while prior < index {
                let left = self.asset(prior)?;
                if asset.shard_mint == left.shard_mint
                    || asset.actor_shard_account == left.actor_shard_account
                    || asset.structured_custody_account == left.structured_custody_account
                    || asset.claims_custody_owner == left.claims_custody_owner
                {
                    return Err(Error::AccountAlias);
                }
                prior = prior.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
            }
            index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }
}
