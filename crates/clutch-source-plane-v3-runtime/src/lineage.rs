use clutch_source_plane_v3::ContentId;

use crate::auth::{
    account_data_id, domain_id, live_id, AuthenticatedSourceRouteV1, RuntimeAccountViewV1,
    RuntimeDerivedPdaV1, RuntimeKey,
};
use crate::{Error, Result};

const LINEAGE_MAGIC: [u8; 8] = *b"DCSLIN01";
const LINEAGE_DOMAIN: &[u8] = b"dragons-clutch/source-reopen-lineage/v1";
const REOPEN_AUTH_DOMAIN: &[u8] = b"dragons-clutch/source-reopen-authorization/v1";
const LINEAGE_RECIPE_DOMAIN: &[u8] = b"dragons-clutch/source-lineage-pda-recipe/v1";
const LINEAGE_ACCOUNT_AUTH_DOMAIN: &[u8] = b"dragons-clutch/authenticated-source-lineage/v1";
const SCHEMA_V1: u16 = 1;

/// Exact canonical bytes in [`ReopenLineageV1`].
pub const REOPEN_LINEAGE_BYTES: usize = 288;

/// Mutable SourcePlane account families that require durable close/reopen history.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LineageFamilyV1 {
    /// Source-only generation head.
    SourceHead = 1,
    /// Mutable raw-page ingestion prefix.
    OpenRawPage = 2,
    /// Resumable Window page-fold cursor.
    WindowWork = 3,
    /// Resumable statistic evaluator cursor.
    EvaluationWork = 4,
    /// Predictable immutable StatisticResult occupancy slot.
    StatisticResult = 5,
}

impl LineageFamilyV1 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::SourceHead),
            2 => Ok(Self::OpenRawPage),
            3 => Ok(Self::WindowWork),
            4 => Ok(Self::EvaluationWork),
            5 => Ok(Self::StatisticResult),
            _ => Err(Error::InvalidCodec),
        }
    }
}

/// Durable one-open-generation-at-a-time lineage state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReopenLineageV1 {
    /// Exact deployed adapter owning this lineage.
    pub adapter_program: RuntimeKey,
    /// Semantic account coordinate: source generation, page cursor, Window, or key.
    pub semantic_binding_id: ContentId,
    /// Physical lineage/tombstone account.
    pub lineage_account: RuntimeKey,
    /// Mutable account family governed by this history.
    pub family: LineageFamilyV1,
    /// Highest generation ever opened; zero means never created.
    pub latest_generation: u64,
    /// Whether `latest_generation` remains live.
    pub is_open: bool,
    /// Physical account of the current generation, or zero while closed.
    pub active_account: RuntimeKey,
    /// Canonical state digest observed at most recent open/transition.
    pub last_opened_state_id: ContentId,
    /// Exact terminal receipt for latest generation, or zero while open/never-created.
    pub last_close_receipt_id: ContentId,
    /// Source work schedule governing every generation.
    pub source_work_schedule_id: ContentId,
    /// Frozen neutral sink for every generation.
    pub neutral_sink: RuntimeKey,
}

impl ReopenLineageV1 {
    /// Construct a never-created lineage value for a new persistent tombstone.
    pub fn new(
        adapter_program: RuntimeKey,
        semantic_binding_id: ContentId,
        lineage_account: RuntimeKey,
        family: LineageFamilyV1,
        source_work_schedule_id: ContentId,
        neutral_sink: RuntimeKey,
    ) -> Result<Self> {
        let value = Self {
            adapter_program,
            semantic_binding_id,
            lineage_account,
            family,
            latest_generation: 0,
            is_open: false,
            active_account: RuntimeKey::ZERO,
            last_opened_state_id: ContentId::ZERO,
            last_close_receipt_id: ContentId::ZERO,
            source_work_schedule_id,
            neutral_sink,
        };
        value.validate()?;
        Ok(value)
    }

    /// Validate exhaustive never-created/open/closed state partition.
    pub fn validate(&self) -> Result<()> {
        self.adapter_program.validate()?;
        live_id(self.semantic_binding_id)?;
        self.lineage_account.validate()?;
        live_id(self.source_work_schedule_id)?;
        self.neutral_sink.validate()?;
        if self.adapter_program == self.lineage_account
            || self.adapter_program == self.neutral_sink
            || self.lineage_account == self.neutral_sink
        {
            return Err(Error::IdentityAlias);
        }
        if self.latest_generation == 0 {
            if self.is_open
                || !self.active_account.is_zero()
                || !self.last_opened_state_id.is_zero()
                || !self.last_close_receipt_id.is_zero()
            {
                return Err(Error::InvalidLineage);
            }
        } else if self.is_open {
            self.active_account.validate()?;
            live_id(self.last_opened_state_id)?;
            if !self.last_close_receipt_id.is_zero() {
                return Err(Error::InvalidLineage);
            }
        } else if !self.active_account.is_zero()
            || self.last_opened_state_id.is_zero()
            || self.last_close_receipt_id.is_zero()
        {
            return Err(Error::InvalidLineage);
        }
        Ok(())
    }

    /// Encode exact canonical lineage bytes.
    pub fn encode(&self) -> Result<[u8; REOPEN_LINEAGE_BYTES]> {
        self.validate()?;
        let mut out = [0; REOPEN_LINEAGE_BYTES];
        out[..8].copy_from_slice(&LINEAGE_MAGIC);
        out[8..10].copy_from_slice(&SCHEMA_V1.to_le_bytes());
        out[10] = self.family as u8;
        out[16..48].copy_from_slice(&self.adapter_program.bytes());
        out[48..80].copy_from_slice(&self.semantic_binding_id.bytes());
        out[80..112].copy_from_slice(&self.lineage_account.bytes());
        out[112..144].copy_from_slice(&self.active_account.bytes());
        out[144..152].copy_from_slice(&self.latest_generation.to_le_bytes());
        out[152] = u8::from(self.is_open);
        out[160..192].copy_from_slice(&self.last_opened_state_id.bytes());
        out[192..224].copy_from_slice(&self.last_close_receipt_id.bytes());
        out[224..256].copy_from_slice(&self.source_work_schedule_id.bytes());
        out[256..288].copy_from_slice(&self.neutral_sink.bytes());
        Ok(out)
    }

    /// Hostile-decode exact canonical lineage bytes.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != REOPEN_LINEAGE_BYTES
            || input[..8] != LINEAGE_MAGIC
            || le_u16(&input[8..10]) != SCHEMA_V1
            || input[11..16].iter().any(|byte| *byte != 0)
            || input[153..160].iter().any(|byte| *byte != 0)
        {
            return Err(Error::InvalidCodec);
        }
        let is_open = match input[152] {
            0 => false,
            1 => true,
            _ => return Err(Error::InvalidCodec),
        };
        let value = Self {
            adapter_program: key_at(input, 16),
            semantic_binding_id: id_at(input, 48),
            lineage_account: key_at(input, 80),
            active_account: key_at(input, 112),
            latest_generation: le_u64(&input[144..152]),
            is_open,
            last_opened_state_id: id_at(input, 160),
            last_close_receipt_id: id_at(input, 192),
            source_work_schedule_id: id_at(input, 224),
            neutral_sink: key_at(input, 256),
            family: LineageFamilyV1::decode(input[10])?,
        };
        value.validate()?;
        Ok(value)
    }

    /// Content identity of the exact current lineage state.
    pub fn id(&self) -> Result<ContentId> {
        Ok(domain_id(LINEAGE_DOMAIN, &self.encode()?))
    }

    /// PDA recipe identity for the persistent lineage account.
    pub fn recipe_id(&self) -> Result<ContentId> {
        self.validate()?;
        let mut bytes = [0; 104];
        bytes[0] = self.family as u8;
        bytes[8..40].copy_from_slice(&self.adapter_program.bytes());
        bytes[40..72].copy_from_slice(&self.semantic_binding_id.bytes());
        bytes[72..104].copy_from_slice(&self.source_work_schedule_id.bytes());
        Ok(domain_id(LINEAGE_RECIPE_DOMAIN, &bytes))
    }
}

/// Required instruction privilege for one lineage account use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineageAccessV1 {
    /// Inspect exact durable history without mutation.
    ReadOnly,
    /// Atomically advance open/state/close history.
    Mutable,
}

/// Runtime-authenticated exact lineage account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedReopenLineageV1 {
    lineage: ReopenLineageV1,
    account_data_id: ContentId,
    access: LineageAccessV1,
    authentication_id: ContentId,
}

impl AuthenticatedReopenLineageV1 {
    /// Complete canonical lineage body.
    pub const fn lineage(self) -> ReopenLineageV1 {
        self.lineage
    }

    /// Digest of complete lineage account bytes.
    pub const fn account_data_id(self) -> ContentId {
        self.account_data_id
    }

    /// Runtime instruction privilege authenticated for this use.
    pub const fn access(self) -> LineageAccessV1 {
        self.access
    }

    /// Complete owner/PDA/body authentication identity.
    pub const fn id(self) -> ContentId {
        self.authentication_id
    }
}

/// Authenticate the persistent lineage account under its exact runtime PDA.
pub fn authenticate_reopen_lineage_account(
    route: AuthenticatedSourceRouteV1,
    account: RuntimeAccountViewV1<'_>,
    derived_pda: RuntimeDerivedPdaV1,
    access: LineageAccessV1,
) -> Result<AuthenticatedReopenLineageV1> {
    if account.owner != route.adapter_program() {
        return Err(Error::WrongOwner);
    }
    if account.executable
        || account.signer
        || account.writable != (access == LineageAccessV1::Mutable)
    {
        return Err(Error::WrongPrivilege);
    }
    let lineage = ReopenLineageV1::decode(account.data)?;
    if lineage.lineage_account != account.key
        || lineage.adapter_program != route.adapter_program()
        || lineage.source_work_schedule_id != route.source_work_schedule_id()
        || lineage.neutral_sink != route.neutral_sink()
    {
        return Err(Error::InvalidLineage);
    }
    derived_pda.validate_for(
        route.adapter_program(),
        lineage.recipe_id()?,
        account.key,
        derived_pda.bump,
    )?;
    let account_data_id = account_data_id(account.key, account.data)?;
    let mut bytes = [0; 136];
    bytes[..32].copy_from_slice(&route.route_id().bytes());
    bytes[32..64].copy_from_slice(&account.key.bytes());
    bytes[64..96].copy_from_slice(&account_data_id.bytes());
    bytes[96..128].copy_from_slice(&lineage.id()?.bytes());
    bytes[128] = derived_pda.bump;
    Ok(AuthenticatedReopenLineageV1 {
        lineage,
        account_data_id,
        access,
        authentication_id: domain_id(LINEAGE_ACCOUNT_AUTH_DOMAIN, &bytes),
    })
}

/// Exact next-generation authority derived from durable lineage and runtime PDA facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReopenAuthorizationV1 {
    family: LineageFamilyV1,
    semantic_binding_id: ContentId,
    lineage_before_id: ContentId,
    target_account: RuntimeKey,
    next_generation: u64,
    authorization_id: ContentId,
}

impl ReopenAuthorizationV1 {
    /// Account family authorized to open.
    pub const fn family(self) -> LineageFamilyV1 {
        self.family
    }

    /// Exact semantic coordinate.
    pub const fn semantic_binding_id(self) -> ContentId {
        self.semantic_binding_id
    }

    /// Physical target account.
    pub const fn target_account(self) -> RuntimeKey {
        self.target_account
    }

    /// Exact monotone next generation.
    pub const fn next_generation(self) -> u64 {
        self.next_generation
    }

    /// Content identity of the complete reopen authorization.
    pub const fn id(self) -> ContentId {
        self.authorization_id
    }
}

/// Authorize exactly the next account generation from a closed/never-created lineage.
pub fn authorize_reopen(
    route: AuthenticatedSourceRouteV1,
    authenticated_lineage: AuthenticatedReopenLineageV1,
    family: LineageFamilyV1,
    semantic_binding_id: ContentId,
    pda_recipe_id: ContentId,
    target_account: RuntimeKey,
    derived_pda: RuntimeDerivedPdaV1,
) -> Result<ReopenAuthorizationV1> {
    if authenticated_lineage.access() != LineageAccessV1::Mutable {
        return Err(Error::WrongPrivilege);
    }
    let lineage = authenticated_lineage.lineage();
    lineage.validate()?;
    live_id(pda_recipe_id)?;
    target_account.validate()?;
    if lineage.is_open
        || lineage.adapter_program != route.adapter_program()
        || lineage.family != family
        || lineage.semantic_binding_id != semantic_binding_id
        || lineage.source_work_schedule_id != route.source_work_schedule_id()
        || lineage.neutral_sink != route.neutral_sink()
    {
        return Err(Error::InvalidLineage);
    }
    derived_pda.validate_for(
        route.adapter_program(),
        pda_recipe_id,
        target_account,
        derived_pda.bump,
    )?;
    let next_generation = lineage
        .latest_generation
        .checked_add(1)
        .ok_or(Error::ArithmeticOverflow)?;
    let lineage_before_id = lineage.id()?;
    let mut bytes = [0; 152];
    bytes[0] = family as u8;
    bytes[8..40].copy_from_slice(&semantic_binding_id.bytes());
    bytes[40..72].copy_from_slice(&lineage_before_id.bytes());
    bytes[72..104].copy_from_slice(&target_account.bytes());
    bytes[104..112].copy_from_slice(&next_generation.to_le_bytes());
    bytes[112..144].copy_from_slice(&pda_recipe_id.bytes());
    bytes[144] = derived_pda.bump;
    Ok(ReopenAuthorizationV1 {
        family,
        semantic_binding_id,
        lineage_before_id,
        target_account,
        next_generation,
        authorization_id: domain_id(REOPEN_AUTH_DOMAIN, &bytes),
    })
}

/// Atomically mark the authorized generation open at one canonical state digest.
pub fn open_lineage_generation(
    lineage: ReopenLineageV1,
    authorization: ReopenAuthorizationV1,
    opened_state_id: ContentId,
) -> Result<ReopenLineageV1> {
    lineage.validate()?;
    live_id(opened_state_id)?;
    if lineage.id()? != authorization.lineage_before_id
        || lineage.family != authorization.family
        || lineage.semantic_binding_id != authorization.semantic_binding_id
        || lineage.is_open
        || lineage.latest_generation.checked_add(1) != Some(authorization.next_generation)
    {
        return Err(Error::InvalidLineage);
    }
    let next = ReopenLineageV1 {
        latest_generation: authorization.next_generation,
        is_open: true,
        active_account: authorization.target_account,
        last_opened_state_id: opened_state_id,
        last_close_receipt_id: ContentId::ZERO,
        ..lineage
    };
    next.validate()?;
    Ok(next)
}

/// Compare-and-swap the state digest owned by one open lineage generation.
pub fn advance_lineage_state(
    lineage: ReopenLineageV1,
    account: RuntimeKey,
    generation: u64,
    before_state_id: ContentId,
    after_state_id: ContentId,
) -> Result<ReopenLineageV1> {
    lineage.validate()?;
    live_id(before_state_id)?;
    live_id(after_state_id)?;
    if !lineage.is_open
        || lineage.active_account != account
        || lineage.latest_generation != generation
        || lineage.last_opened_state_id != before_state_id
        || before_state_id == after_state_id
    {
        return Err(Error::InvalidLineage);
    }
    let next = ReopenLineageV1 {
        last_opened_state_id: after_state_id,
        ..lineage
    };
    next.validate()?;
    Ok(next)
}

/// Atomically record the final state and exact close receipt for one generation.
pub fn close_lineage_generation(
    lineage: ReopenLineageV1,
    account: RuntimeKey,
    generation: u64,
    final_state_id: ContentId,
    close_receipt_id: ContentId,
) -> Result<ReopenLineageV1> {
    lineage.validate()?;
    live_id(final_state_id)?;
    live_id(close_receipt_id)?;
    if !lineage.is_open
        || lineage.active_account != account
        || lineage.latest_generation != generation
        || lineage.last_opened_state_id != final_state_id
    {
        return Err(Error::InvalidLineage);
    }
    let next = ReopenLineageV1 {
        is_open: false,
        active_account: RuntimeKey::ZERO,
        last_close_receipt_id: close_receipt_id,
        ..lineage
    };
    next.validate()?;
    Ok(next)
}

fn key_at(input: &[u8], at: usize) -> RuntimeKey {
    let mut bytes = [0; 32];
    bytes.copy_from_slice(&input[at..at + 32]);
    RuntimeKey::from_bytes(bytes)
}

fn id_at(input: &[u8], at: usize) -> ContentId {
    ContentId::from_bytes(key_at(input, at).bytes())
}

fn le_u16(input: &[u8]) -> u16 {
    let mut word = [0; 2];
    word.copy_from_slice(input);
    u16::from_le_bytes(word)
}

fn le_u64(input: &[u8]) -> u64 {
    let mut word = [0; 8];
    word.copy_from_slice(input);
    u64::from_le_bytes(word)
}
