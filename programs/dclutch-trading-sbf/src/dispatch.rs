//! Family-neutral authenticated capability-program dispatch.

use dclutch_capability_contract::{CapabilityEntryV1, CapabilityManifestV1};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_ACCOUNT_MAX_BYTES_V1, CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityProgramV1,
    CapabilityRegistersV2, CapabilityRootHeaderV1, Error as CapabilityProgramError,
    SupportedContentV1,
};
use dclutch_core_contract::ContentId;
use dclutch_market_core_codec::{
    CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1, CapabilityFundingHeaderV1,
};
use dclutch_registry_svm::AuthenticatedRoleReceiptV1;
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CapabilityExecutionSelectionV1, ExecutionRoleV1,
};
use solana_program::{hash::hash, pubkey::Pubkey};

use crate::TradingSbfError;

/// Borrowed exact activation request after the Core effect envelope.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TradingActivationRequestV1<'a> {
    selection: CapabilityExecutionSelectionV1,
    funding: CapabilityFundingHeaderV1,
    family_request: &'a [u8],
}

impl<'a> TradingActivationRequestV1<'a> {
    /// Hostile-decode `selector(144) || funding-header(16) || family request`.
    pub fn decode(role_request: &'a [u8]) -> Result<Self, TradingSbfError> {
        let funding_offset = dclutch_release_set_contract::CAPABILITY_EXECUTION_SELECTION_BYTES_V1;
        let family_offset = funding_offset
            .checked_add(CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1)
            .ok_or(TradingSbfError::Content)?;
        let selection = CapabilityExecutionSelectionV1::decode(
            role_request
                .get(..funding_offset)
                .ok_or(TradingSbfError::Content)?,
        )
        .map_err(|_| TradingSbfError::Content)?;
        let funding = CapabilityFundingHeaderV1::decode(
            role_request
                .get(funding_offset..family_offset)
                .ok_or(TradingSbfError::Content)?,
        )
        .map_err(|_| TradingSbfError::Content)?;
        Ok(Self {
            selection,
            funding,
            family_request: role_request
                .get(family_offset..)
                .ok_or(TradingSbfError::Content)?,
        })
    }

    /// Return the exact manifest-derived activation projection.
    pub const fn selection(self) -> CapabilityExecutionSelectionV1 {
        self.selection
    }

    /// Return the bounded count of leading FundingState accounts.
    pub const fn funding(self) -> CapabilityFundingHeaderV1 {
        self.funding
    }

    /// Borrow the schema-selected family request suffix.
    pub const fn family_request(self) -> &'a [u8] {
        self.family_request
    }
}

/// Borrowed prefix of accounts Core passes to a Trading activation CPI.
///
/// The descriptor-selected account profile owns the exact `family_accounts`
/// suffix. It must authenticate the descriptor/config/finalization records and
/// all schema-specific resources; there is no family discriminator here.
/// Common accounts cannot alias each other or the suffix. Safe aliases wholly
/// inside the suffix are owned by the authenticated account profile.
pub struct TradingActivationAccountsV1<'accounts, 'info> {
    core_authority: &'accounts solana_program::account_info::AccountInfo<'info>,
    child_root: &'accounts solana_program::account_info::AccountInfo<'info>,
    funding: &'accounts [solana_program::account_info::AccountInfo<'info>],
    manifest: &'accounts solana_program::account_info::AccountInfo<'info>,
    market: &'accounts solana_program::account_info::AccountInfo<'info>,
    family_accounts: &'accounts [solana_program::account_info::AccountInfo<'info>],
}

impl<'accounts, 'info> TradingActivationAccountsV1<'accounts, 'info> {
    /// Hostile-frame the exact common prefix using the authenticated header count.
    pub fn parse(
        accounts: &'accounts [solana_program::account_info::AccountInfo<'info>],
        funding: CapabilityFundingHeaderV1,
    ) -> Result<Self, TradingSbfError> {
        const AUTHORITY: usize = 0;
        const ROOT: usize = 1;
        const FUNDING_START: usize = 2;
        let manifest_index = FUNDING_START
            .checked_add(usize::from(funding.funding_count()))
            .ok_or(TradingSbfError::Content)?;
        let market_index = manifest_index
            .checked_add(1)
            .ok_or(TradingSbfError::Content)?;
        let family_start = market_index
            .checked_add(1)
            .ok_or(TradingSbfError::Content)?;
        let core_authority = accounts.get(AUTHORITY).ok_or(TradingSbfError::Content)?;
        let child_root = accounts.get(ROOT).ok_or(TradingSbfError::Content)?;
        let funding_accounts = accounts
            .get(FUNDING_START..manifest_index)
            .ok_or(TradingSbfError::Content)?;
        let manifest = accounts
            .get(manifest_index)
            .ok_or(TradingSbfError::Content)?;
        let market = accounts.get(market_index).ok_or(TradingSbfError::Content)?;
        let family_accounts = accounts
            .get(family_start..)
            .ok_or(TradingSbfError::Content)?;
        if !core_authority.is_signer
            || core_authority.is_writable
            || core_authority.executable
            || child_root.is_signer
            || !child_root.is_writable
            || child_root.executable
            || manifest.is_signer
            || manifest.is_writable
            || manifest.executable
            || market.is_signer
            || market.is_writable
            || market.executable
            || funding_accounts
                .iter()
                .any(|account| account.is_signer || !account.is_writable || account.executable)
            || family_accounts.iter().any(|account| account.is_signer)
        {
            return Err(TradingSbfError::Content);
        }
        require_common_accounts_distinct(accounts, family_start)?;
        Ok(Self {
            core_authority,
            child_root,
            funding: funding_accounts,
            manifest,
            market,
            family_accounts,
        })
    }

    /// Return the Core release-set caller-authority signer.
    pub const fn core_authority(
        &self,
    ) -> &'accounts solana_program::account_info::AccountInfo<'info> {
        self.core_authority
    }

    /// Return the one writable composite Trading root.
    pub const fn child_root(&self) -> &'accounts solana_program::account_info::AccountInfo<'info> {
        self.child_root
    }

    /// Return the ordered writable FundingState account list.
    pub const fn funding(&self) -> &'accounts [solana_program::account_info::AccountInfo<'info>] {
        self.funding
    }

    /// Return the exact selected manifest raw-record account.
    pub const fn manifest(&self) -> &'accounts solana_program::account_info::AccountInfo<'info> {
        self.manifest
    }

    /// Return the authenticated Core Market forwarded read-only by Core.
    pub const fn market(&self) -> &'accounts solana_program::account_info::AccountInfo<'info> {
        self.market
    }

    /// Return the exact descriptor-account-profile-owned suffix.
    pub const fn family_accounts(
        &self,
    ) -> &'accounts [solana_program::account_info::AccountInfo<'info>] {
        self.family_accounts
    }
}

fn require_common_accounts_distinct(
    accounts: &[solana_program::account_info::AccountInfo<'_>],
    common_count: usize,
) -> Result<(), TradingSbfError> {
    let common = accounts
        .get(..common_count)
        .ok_or(TradingSbfError::Content)?;
    for (index, account) in common.iter().enumerate() {
        if accounts
            .get(index.saturating_add(1)..)
            .ok_or(TradingSbfError::Content)?
            .iter()
            .any(|other| account.key == other.key)
        {
            return Err(TradingSbfError::Content);
        }
    }
    Ok(())
}

/// Current fixed-role authority and immutable child-root projection supplied
/// to one family-neutral physical register projector.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TradingFamilyContextV1 {
    program_id: [u8; 32],
    child_root_key: [u8; 32],
    root_account_bytes: usize,
    market: [u8; 32],
    generation: u64,
    release_set: ContentId,
    selection: CapabilityExecutionSelectionV1,
    artifact_release: ArtifactReleaseIdV1,
    interpreter_semantic_release: ContentId,
}

impl TradingFamilyContextV1 {
    /// Authenticate a proposed activation projection before creating its root.
    ///
    /// The physical activation handler must additionally require the target
    /// root to be the exact vacant System account and must not create/write it
    /// until descriptor, config, family request, funding, and rent admission
    /// have all succeeded. [`dispatch_activation_authenticated`] rejoins the
    /// supplied width to the authenticated descriptor.
    pub fn authenticate_activation(
        program_id: &Pubkey,
        child_root_key: &Pubkey,
        root: CapabilityRootHeaderV1,
        proposed_root_account_bytes: usize,
        trading_receipt: AuthenticatedRoleReceiptV1,
    ) -> Result<Self, TradingSbfError> {
        Self::authenticate_header(
            program_id,
            child_root_key,
            proposed_root_account_bytes,
            root,
            trading_receipt,
        )
    }

    /// Authenticate one existing immutable root against the current Trading receipt.
    ///
    /// The composing SBF boundary must obtain `trading_receipt` from an
    /// immediate Registry CPI and require the Registry Program as return-data
    /// producer before calling this function.
    pub fn authenticate(
        program_id: &Pubkey,
        child_root_key: &Pubkey,
        child_root_owner: &Pubkey,
        root_account_data: &[u8],
        trading_receipt: AuthenticatedRoleReceiptV1,
    ) -> Result<Self, TradingSbfError> {
        if child_root_owner != program_id {
            return Err(TradingSbfError::Root);
        }
        let root = CapabilityRootHeaderV1::decode(
            root_account_data
                .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
                .ok_or(TradingSbfError::Root)?,
        )
        .map_err(|_| TradingSbfError::Root)?;
        Self::authenticate_header(
            program_id,
            child_root_key,
            root_account_data.len(),
            root,
            trading_receipt,
        )
    }

    fn authenticate_header(
        program_id: &Pubkey,
        child_root_key: &Pubkey,
        root_account_bytes: usize,
        root: CapabilityRootHeaderV1,
        trading_receipt: AuthenticatedRoleReceiptV1,
    ) -> Result<Self, TradingSbfError> {
        if root_account_bytes <= CAPABILITY_ROOT_HEADER_BYTES_V1
            || root_account_bytes > CAPABILITY_ROOT_ACCOUNT_MAX_BYTES_V1
        {
            return Err(TradingSbfError::Root);
        }
        if trading_receipt.role() != ExecutionRoleV1::Trading
            || trading_receipt.program().to_bytes() != program_id.to_bytes()
            || trading_receipt.execution_release_set_id() != root.release_set()
        {
            return Err(TradingSbfError::Release);
        }
        let seeds = root.seeds();
        let expected_root = Pubkey::find_program_address(&seeds.as_slices(), program_id).0;
        if expected_root != *child_root_key {
            return Err(TradingSbfError::Root);
        }
        Ok(Self {
            program_id: program_id.to_bytes(),
            child_root_key: child_root_key.to_bytes(),
            root_account_bytes,
            market: root.market(),
            generation: root.generation(),
            release_set: root.release_set(),
            selection: root.selection(),
            artifact_release: trading_receipt.artifact_release_id(),
            interpreter_semantic_release: trading_receipt.semantic_release_id(),
        })
    }

    /// Return the sole current Trading Program identity.
    pub const fn program_id(self) -> [u8; 32] {
        self.program_id
    }
    /// Return the authenticated immutable child-root account identity.
    pub const fn child_root_key(self) -> [u8; 32] {
        self.child_root_key
    }
    /// Return the observed exact composite root-account width.
    pub const fn root_account_bytes(self) -> usize {
        self.root_account_bytes
    }
    /// Return the exact Market identity.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }
    /// Return the exact Market generation.
    pub const fn generation(self) -> u64 {
        self.generation
    }
    /// Return the immutable execution release-set identity.
    pub const fn release_set(self) -> ContentId {
        self.release_set
    }
    /// Return the exact persisted activation projection.
    pub const fn selection(self) -> CapabilityExecutionSelectionV1 {
        self.selection
    }
    /// Return the current checked Trading artifact release.
    pub const fn artifact_release(self) -> ArtifactReleaseIdV1 {
        self.artifact_release
    }
    /// Return the current Trading interpreter semantic release.
    pub const fn interpreter_semantic_release(self) -> ContentId {
        self.interpreter_semantic_release
    }
}

/// Hostile-authenticate activation content and run one data-defined transition.
///
/// `supported` is compiled into the current checked Trading artifact by the
/// exact register projector/effect boundary. It is never decoded from the
/// instruction. The function mutates `registers` only if every content join
/// and every TransitionVM instruction succeeds. This fail-closed foundation
/// is not the open-family gate: a named-family list of `supported` values is
/// still a closed adapter and must be superseded by interpreted or certified
/// physical profile languages.
pub fn dispatch_activation_authenticated<'descriptor>(
    context: TradingFamilyContextV1,
    manifest_bytes: &[u8],
    descriptor_bytes: &'descriptor [u8],
    config_bytes: &[u8],
    supported: SupportedContentV1,
    registers: CapabilityRegistersV2<'_>,
) -> Result<CapabilityProgramV1<'descriptor>, TradingSbfError> {
    let descriptor =
        authenticate_activation_program(context, manifest_bytes, descriptor_bytes, config_bytes)?;
    supported
        .require(descriptor)
        .map_err(map_capability_program_error)?;
    descriptor
        .execute(registers)
        .map_err(map_capability_program_error)?;
    Ok(descriptor)
}

/// Authenticate one activation descriptor without selecting a Rust family.
///
/// The executable outer uses this join before interpreting the descriptor's
/// finalized AccountProfile and EffectProgram. It deliberately performs no
/// transition and carries no compiled `SupportedContentV1` list.
pub fn authenticate_activation_program<'descriptor>(
    context: TradingFamilyContextV1,
    manifest_bytes: &[u8],
    descriptor_bytes: &'descriptor [u8],
    config_bytes: &[u8],
) -> Result<CapabilityProgramV1<'descriptor>, TradingSbfError> {
    let selection = context.selection();
    if hash(manifest_bytes).to_bytes() != selection.manifest().to_bytes() {
        return Err(TradingSbfError::Content);
    }
    let manifest =
        CapabilityManifestV1::decode(manifest_bytes).map_err(|_| TradingSbfError::Content)?;
    let entry = manifest
        .entry(selection.entry_index())
        .map_err(|_| TradingSbfError::Content)?;
    require_entry_identity(entry, selection)?;

    let descriptor = authenticate_common_content(context, descriptor_bytes, config_bytes)?;
    descriptor
        .validate_selection(selection, entry)
        .map_err(map_capability_program_error)?;
    Ok(descriptor)
}

/// Hostile-authenticate persisted hot-action content and run one transition.
///
/// Activation already joined the embedded selector to the exact manifest
/// entry before creating the immutable root header. Hot actions therefore do
/// not repeat either the selector or the manifest account. They authenticate
/// the persisted header and still require the exact descriptor and config
/// digests on every call so neither semantic input can be substituted.
pub fn dispatch_hot_authenticated<'descriptor>(
    context: TradingFamilyContextV1,
    descriptor_bytes: &'descriptor [u8],
    config_bytes: &[u8],
    supported: SupportedContentV1,
    registers: CapabilityRegistersV2<'_>,
) -> Result<CapabilityProgramV1<'descriptor>, TradingSbfError> {
    let descriptor = authenticate_common_content(context, descriptor_bytes, config_bytes)?;
    supported
        .require(descriptor)
        .map_err(map_capability_program_error)?;
    descriptor
        .validate_persisted_selection(context.selection())
        .map_err(map_capability_program_error)?;
    descriptor
        .execute(registers)
        .map_err(map_capability_program_error)?;
    Ok(descriptor)
}

fn authenticate_common_content<'descriptor>(
    context: TradingFamilyContextV1,
    descriptor_bytes: &'descriptor [u8],
    config_bytes: &[u8],
) -> Result<CapabilityProgramV1<'descriptor>, TradingSbfError> {
    let selection = context.selection();

    if hash(descriptor_bytes).to_bytes() != selection.capability_release().to_bytes()
        || hash(config_bytes).to_bytes() != selection.config().to_bytes()
    {
        return Err(TradingSbfError::Content);
    }
    let descriptor =
        CapabilityProgramV1::decode(descriptor_bytes).map_err(map_capability_program_error)?;
    if descriptor
        .root_account_bytes()
        .map_err(map_capability_program_error)?
        != context.root_account_bytes()
    {
        return Err(TradingSbfError::Root);
    }
    Ok(descriptor)
}

fn require_entry_identity(
    entry: CapabilityEntryV1,
    selection: CapabilityExecutionSelectionV1,
) -> Result<(), TradingSbfError> {
    if entry.kind_id() != selection.kind()
        || entry.release_id() != selection.capability_release()
        || entry.config_id() != selection.config()
    {
        Err(TradingSbfError::Content)
    } else {
        Ok(())
    }
}

fn map_capability_program_error(error: CapabilityProgramError) -> TradingSbfError {
    match error {
        CapabilityProgramError::TransitionRefused => TradingSbfError::Transition,
        CapabilityProgramError::UnsupportedContent => TradingSbfError::UnsupportedContent,
        _ => TradingSbfError::Content,
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use std::{boxed::Box, vec};

    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityManifestV1, FundingAmountsV1,
        FundingQuoteV1, MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_capability_program_contract::{
        CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET,
        CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET, CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET,
        CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, CAPABILITY_PROGRAM_HEADER_BYTES_V1,
        CAPABILITY_PROGRAM_KIND_OFFSET, CAPABILITY_PROGRAM_MAGIC_V1,
        CAPABILITY_PROGRAM_MAX_BYTES_V1, CAPABILITY_PROGRAM_MAX_RENT_LAMPORTS_V1,
        CAPABILITY_PROGRAM_PROFILE_OFFSET, CAPABILITY_PROGRAM_PROFILE_V2,
        CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET, CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET,
        CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
        CAPABILITY_PROGRAM_TRANSITION_MAX_INSTRUCTIONS_V2, CAPABILITY_ROOT_HEADER_BYTES_V1,
        initialize_root_account_v1,
    };
    use dclutch_market_core_codec::{
        CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1, CORE_EFFECT_ENVELOPE_BYTES_V1, REQUEST_BYTES,
    };
    use dclutch_record_contract::{
        APPEND_PAGE_HEADER_BYTES_V1, BEGIN_RECORD_BYTES_V1, CANONICAL_RECORD_PAGE_BYTES_V1,
        UNIT_REQUEST_BYTES_V1,
    };
    use dclutch_release_set_contract::CAPABILITY_EXECUTION_SELECTION_BYTES_V1;
    use dclutch_release_set_contract::{ArtifactReleaseIdV1, ProgramIdentityV1};
    use dclutch_transition_vm::v2::{RegisterInput, RegisterOutput};
    use solana_hash::Hash;
    use solana_message::{AddressLookupTableAccount, VersionedMessage, v0};
    use solana_program::instruction::{AccountMeta, Instruction};

    use super::*;

    const PACKET_DATA_BYTES: usize = 1_232;
    const MAX_FUNDING_ACCOUNTS: usize = 16;
    const REPRESENTATIVE_COMMON_ACCOUNTS: usize = 18;
    const REPRESENTATIVE_MAX_FAMILY_ACTIVATION_REQUEST_BYTES: usize = 256;

    fn id(byte: u8) -> ContentId {
        ContentId::new([byte; 32]).expect("identity")
    }

    fn account(
        key: Pubkey,
        signer: bool,
        writable: bool,
    ) -> solana_program::account_info::AccountInfo<'static> {
        solana_program::account_info::AccountInfo::new(
            Box::leak(Box::new(key)),
            signer,
            writable,
            Box::leak(Box::new(0)),
            Box::leak(vec![].into_boxed_slice()),
            Box::leak(Box::new(Pubkey::new_from_array([200; 32]))),
            false,
        )
    }

    fn fixture_write(output: &mut [u8], offset: usize, source: &[u8]) {
        let end = offset.checked_add(source.len()).expect("fixture width");
        output
            .get_mut(offset..end)
            .expect("fixture destination")
            .copy_from_slice(source);
    }

    fn fixture_fill(output: &mut [u8], offset: usize, width: usize, value: u8) {
        let end = offset.checked_add(width).expect("fixture width");
        output
            .get_mut(offset..end)
            .expect("fixture destination")
            .fill(value);
    }

    fn fixture_set(output: &mut [u8], offset: usize, value: u8) {
        *output.get_mut(offset).expect("fixture byte") = value;
    }

    fn descriptor_bytes() -> vec::Vec<u8> {
        let mut bytes = vec![0_u8; CAPABILITY_PROGRAM_HEADER_BYTES_V1 + 40];
        fixture_write(&mut bytes, 0, &CAPABILITY_PROGRAM_MAGIC_V1);
        fixture_write(&mut bytes, 8, &1_u16.to_le_bytes());
        fixture_write(
            &mut bytes,
            CAPABILITY_PROGRAM_PROFILE_OFFSET,
            &CAPABILITY_PROGRAM_PROFILE_V2.to_le_bytes(),
        );
        for (offset, byte) in [
            (CAPABILITY_PROGRAM_KIND_OFFSET, 1),
            (CAPABILITY_PROGRAM_CONFIG_SCHEMA_OFFSET, 2),
            (CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET, 3),
            (CAPABILITY_PROGRAM_ROOT_SCHEMA_OFFSET, 4),
            (CAPABILITY_PROGRAM_ACCOUNT_PROFILE_OFFSET, 5),
            (CAPABILITY_PROGRAM_DERIVATION_POLICY_OFFSET, 6),
            (CAPABILITY_PROGRAM_CAPACITY_PROFILE_OFFSET, 7),
            (CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET, 8),
        ] {
            fixture_fill(&mut bytes, offset, 32, byte);
        }
        fixture_write(
            &mut bytes,
            CAPABILITY_PROGRAM_ROOT_STATE_BYTES_OFFSET,
            &128_u32.to_le_bytes(),
        );
        let program = CAPABILITY_PROGRAM_HEADER_BYTES_V1;
        fixture_write(&mut bytes, program, b"DCTV");
        fixture_set(&mut bytes, program + 4, 2);
        fixture_write(&mut bytes, program + 6, &1_u16.to_le_bytes());
        fixture_write(&mut bytes, program + 8, &1_u16.to_le_bytes());
        fixture_write(&mut bytes, program + 32, &17_u64.to_le_bytes());
        bytes
    }

    fn activation_result<'descriptor>(
        context: TradingFamilyContextV1,
        manifest: &[u8],
        descriptor: &'descriptor [u8],
        config: &[u8],
        supported: SupportedContentV1,
    ) -> Result<CapabilityProgramV1<'descriptor>, TradingSbfError> {
        let input = [0_u64; 1];
        let input_identities: [[u8; 32]; 0] = [];
        let mut scratch = [0_u64; 1];
        let mut scratch_identities: [[u8; 32]; 0] = [];
        let mut output = [0_u64; 1];
        let mut output_identities: [[u8; 32]; 0] = [];
        dispatch_activation_authenticated(
            context,
            manifest,
            descriptor,
            config,
            supported,
            CapabilityRegistersV2::new(
                RegisterInput {
                    scalars: &input,
                    identities: &input_identities,
                },
                RegisterOutput {
                    scalars: &mut scratch,
                    identities: &mut scratch_identities,
                },
                RegisterOutput {
                    scalars: &mut output,
                    identities: &mut output_identities,
                },
            ),
        )
    }

    fn hot_result<'descriptor>(
        context: TradingFamilyContextV1,
        descriptor: &'descriptor [u8],
        config: &[u8],
        supported: SupportedContentV1,
    ) -> Result<CapabilityProgramV1<'descriptor>, TradingSbfError> {
        let input = [0_u64; 1];
        let input_identities: [[u8; 32]; 0] = [];
        let mut scratch = [0_u64; 1];
        let mut scratch_identities: [[u8; 32]; 0] = [];
        let mut output = [0_u64; 1];
        let mut output_identities: [[u8; 32]; 0] = [];
        dispatch_hot_authenticated(
            context,
            descriptor,
            config,
            supported,
            CapabilityRegistersV2::new(
                RegisterInput {
                    scalars: &input,
                    identities: &input_identities,
                },
                RegisterOutput {
                    scalars: &mut scratch,
                    identities: &mut scratch_identities,
                },
                RegisterOutput {
                    scalars: &mut output,
                    identities: &mut output_identities,
                },
            ),
        )
    }

    fn canonical_dispatch_fixture() -> (
        TradingFamilyContextV1,
        vec::Vec<u8>,
        vec::Vec<u8>,
        vec::Vec<u8>,
        SupportedContentV1,
    ) {
        let descriptor = descriptor_bytes();
        let descriptor_id = ContentId::new(hash(&descriptor).to_bytes()).expect("descriptor ID");
        let config = vec![42_u8; 33];
        let config_id = ContentId::new(hash(&config).to_bytes()).expect("config ID");
        let entry = CapabilityEntryV1::new(
            id(1),
            descriptor_id,
            config_id,
            id(7),
            id(4),
            id(6),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            FundingQuoteV1::new(FundingAmountsV1::default(), None).expect("zero quote"),
        )
        .expect("entry");
        let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut manifest).expect("manifest");
        let manifest_id = ContentId::new(hash(&manifest).to_bytes()).expect("manifest ID");
        let selection =
            CapabilityExecutionSelectionV1::new(0, manifest_id, id(1), descriptor_id, config_id)
                .expect("selection");
        let release_set = id(12);
        let root = CapabilityRootHeaderV1::new(release_set, [13; 32], 14, selection).expect("root");
        let decoded_descriptor = CapabilityProgramV1::decode(&descriptor).expect("descriptor");
        let mut root_account = vec![
            0_u8;
            decoded_descriptor
                .root_account_bytes()
                .expect("root account width")
        ];
        initialize_root_account_v1(&mut root_account, root, decoded_descriptor, &[0; 128])
            .expect("root account");
        let program_id = Pubkey::new_from_array([15; 32]);
        let seeds = root.seeds();
        let child_root = Pubkey::find_program_address(&seeds.as_slices(), &program_id).0;
        let receipt = AuthenticatedRoleReceiptV1::new(
            ExecutionRoleV1::Trading,
            release_set,
            ProgramIdentityV1::new(program_id.to_bytes()).expect("program"),
            ArtifactReleaseIdV1::new([16; 32]).expect("artifact"),
            id(17),
        );
        let context = TradingFamilyContextV1::authenticate(
            &program_id,
            &child_root,
            &program_id,
            &root_account,
            receipt,
        )
        .expect("context");
        let supported = SupportedContentV1 {
            config_schema: id(2),
            request_schema: id(3),
            root_schema: id(4),
            account_profile: id(5),
            derivation_policy: id(6),
            effect_schema: id(8),
        };
        (context, manifest, descriptor, config, supported)
    }

    #[test]
    fn descriptor_hash_is_the_manifest_release_authority() {
        let (context, manifest, descriptor, config, supported) = canonical_dispatch_fixture();
        let admitted = activation_result(context, &manifest, &descriptor, &config, supported)
            .expect("authenticated data-defined dispatch");
        assert_eq!(admitted.kind(), id(1));
        assert_eq!(admitted.transition_program().instruction_count(), 1);
        assert_eq!(admitted.transition_program().scalar_count(), 1);

        let mut substituted_descriptor = descriptor;
        let substituted_effect = substituted_descriptor
            .get(CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET)
            .copied()
            .expect("effect byte")
            ^ 1;
        fixture_set(
            &mut substituted_descriptor,
            CAPABILITY_PROGRAM_EFFECT_SCHEMA_OFFSET,
            substituted_effect,
        );
        assert_eq!(
            activation_result(
                context,
                &manifest,
                &substituted_descriptor,
                &config,
                supported,
            ),
            Err(TradingSbfError::Content)
        );
        let mut substituted_config = config;
        let first = substituted_config.first().copied().expect("config byte") ^ 1;
        fixture_set(&mut substituted_config, 0, first);
        assert_eq!(
            activation_result(
                context,
                &manifest,
                &descriptor_bytes(),
                &substituted_config,
                supported,
            ),
            Err(TradingSbfError::Content)
        );
    }

    #[test]
    fn activation_request_has_one_exact_selector_and_funding_header() {
        let (context, _manifest, _descriptor, _config, _supported) = canonical_dispatch_fixture();
        let family = [31_u8; 9];
        let funding = CapabilityFundingHeaderV1::new(16).expect("funding header");
        let mut bytes = vec![
            0_u8;
            CAPABILITY_EXECUTION_SELECTION_BYTES_V1
                + CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1
                + family.len()
        ];
        fixture_write(&mut bytes, 0, &context.selection().to_bytes());
        fixture_write(
            &mut bytes,
            CAPABILITY_EXECUTION_SELECTION_BYTES_V1,
            &funding.encode(),
        );
        fixture_write(
            &mut bytes,
            CAPABILITY_EXECUTION_SELECTION_BYTES_V1 + CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1,
            &family,
        );
        let decoded = TradingActivationRequestV1::decode(&bytes).expect("activation request");
        assert_eq!(decoded.selection(), context.selection());
        assert_eq!(decoded.funding(), funding);
        assert_eq!(decoded.family_request(), family);

        for truncated in
            0..CAPABILITY_EXECUTION_SELECTION_BYTES_V1 + CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1
        {
            assert_eq!(
                TradingActivationRequestV1::decode(
                    bytes.get(..truncated).expect("truncated role request")
                ),
                Err(TradingSbfError::Content)
            );
        }
        let mut noncanonical_header = bytes;
        fixture_set(
            &mut noncanonical_header,
            CAPABILITY_EXECUTION_SELECTION_BYTES_V1 + 15,
            1,
        );
        assert_eq!(
            TradingActivationRequestV1::decode(&noncanonical_header),
            Err(TradingSbfError::Content)
        );
    }

    #[test]
    fn activation_account_prefix_is_ordered_distinct_and_privilege_exact() {
        let funding = CapabilityFundingHeaderV1::new(2).expect("funding header");
        let accounts = [
            account(Pubkey::new_from_array([1; 32]), true, false),
            account(Pubkey::new_from_array([2; 32]), false, true),
            account(Pubkey::new_from_array([3; 32]), false, true),
            account(Pubkey::new_from_array([4; 32]), false, true),
            account(Pubkey::new_from_array([5; 32]), false, false),
            account(Pubkey::new_from_array([6; 32]), false, false),
            account(Pubkey::new_from_array([7; 32]), false, false),
            account(Pubkey::new_from_array([7; 32]), false, false),
        ];
        let decoded =
            TradingActivationAccountsV1::parse(&accounts, funding).expect("account prefix");
        assert_eq!(decoded.core_authority().key, accounts[0].key);
        assert_eq!(decoded.child_root().key, accounts[1].key);
        assert_eq!(decoded.funding().len(), 2);
        assert_eq!(decoded.manifest().key, accounts[4].key);
        assert_eq!(decoded.market().key, accounts[5].key);
        assert_eq!(decoded.family_accounts().len(), 2);
        assert_eq!(
            decoded.family_accounts().first().expect("suffix first").key,
            decoded.family_accounts().get(1).expect("suffix second").key
        );

        let wrong_privilege = [
            account(Pubkey::new_from_array([11; 32]), false, false),
            account(Pubkey::new_from_array([12; 32]), false, true),
            account(Pubkey::new_from_array([13; 32]), false, true),
            account(Pubkey::new_from_array([14; 32]), false, true),
            account(Pubkey::new_from_array([15; 32]), false, false),
            account(Pubkey::new_from_array([16; 32]), false, false),
        ];
        assert!(TradingActivationAccountsV1::parse(&wrong_privilege, funding).is_err());

        let duplicate = [
            account(Pubkey::new_from_array([21; 32]), true, false),
            account(Pubkey::new_from_array([22; 32]), false, true),
            account(Pubkey::new_from_array([23; 32]), false, true),
            account(Pubkey::new_from_array([23; 32]), false, true),
            account(Pubkey::new_from_array([25; 32]), false, false),
            account(Pubkey::new_from_array([26; 32]), false, false),
        ];
        assert!(TradingActivationAccountsV1::parse(&duplicate, funding).is_err());

        let writable_market = [
            account(Pubkey::new_from_array([31; 32]), true, false),
            account(Pubkey::new_from_array([32; 32]), false, true),
            account(Pubkey::new_from_array([33; 32]), false, true),
            account(Pubkey::new_from_array([34; 32]), false, true),
            account(Pubkey::new_from_array([35; 32]), false, false),
            account(Pubkey::new_from_array([36; 32]), false, true),
        ];
        assert!(TradingActivationAccountsV1::parse(&writable_market, funding).is_err());

        let suffix_substitutes_market = [
            account(Pubkey::new_from_array([41; 32]), true, false),
            account(Pubkey::new_from_array([42; 32]), false, true),
            account(Pubkey::new_from_array([43; 32]), false, true),
            account(Pubkey::new_from_array([44; 32]), false, true),
            account(Pubkey::new_from_array([45; 32]), false, false),
            account(Pubkey::new_from_array([46; 32]), false, false),
            account(Pubkey::new_from_array([46; 32]), false, false),
        ];
        assert!(TradingActivationAccountsV1::parse(&suffix_substitutes_market, funding).is_err());
    }

    #[test]
    fn hot_dispatch_uses_persisted_selection_without_manifest_input() {
        let (context, _manifest, descriptor, config, supported) = canonical_dispatch_fixture();
        let admitted = hot_result(context, &descriptor, &config, supported)
            .expect("authenticated hot dispatch");
        assert_eq!(admitted.kind(), id(1));
        assert_eq!(admitted.transition_program().instruction_count(), 1);

        let mut substituted_descriptor = descriptor;
        let byte = substituted_descriptor
            .get(CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET)
            .copied()
            .expect("request byte")
            ^ 1;
        fixture_set(
            &mut substituted_descriptor,
            CAPABILITY_PROGRAM_REQUEST_SCHEMA_OFFSET,
            byte,
        );
        assert_eq!(
            hot_result(context, &substituted_descriptor, &config, supported,),
            Err(TradingSbfError::Content)
        );

        let mut substituted_config = config;
        let first = substituted_config.first().copied().expect("config byte") ^ 1;
        fixture_set(&mut substituted_config, 0, first);
        assert_eq!(
            hot_result(context, &descriptor_bytes(), &substituted_config, supported,),
            Err(TradingSbfError::Content)
        );
    }

    #[test]
    fn maximum_descriptor_has_exact_rent_and_bounded_record_publication() {
        assert_eq!(CAPABILITY_PROGRAM_TRANSITION_MAX_INSTRUCTIONS_V2, 42);
        assert_eq!(CAPABILITY_PROGRAM_MAX_BYTES_V1, 1_304);
        assert_eq!(
            solana_program::rent::Rent::default().minimum_balance(CAPABILITY_PROGRAM_MAX_BYTES_V1),
            CAPABILITY_PROGRAM_MAX_RENT_LAMPORTS_V1
        );
        let page = usize::try_from(CANONICAL_RECORD_PAGE_BYTES_V1).expect("page width");
        assert_eq!(page, 768);
        assert_eq!(CAPABILITY_PROGRAM_MAX_BYTES_V1.div_ceil(page), 2);
        assert_eq!(BEGIN_RECORD_BYTES_V1, 176);
        assert_eq!(APPEND_PAGE_HEADER_BYTES_V1 + page, 808);
        assert_eq!(APPEND_PAGE_HEADER_BYTES_V1 + (1_304 - page), 576);
        assert_eq!(UNIT_REQUEST_BYTES_V1, 16);
        assert_eq!(
            solana_program::rent::Rent::default()
                .minimum_balance(CAPABILITY_ROOT_HEADER_BYTES_V1 + 128),
            3_396_480
        );
    }

    #[test]
    fn maximum_funding_activation_compiles_as_a_full_v0_packet() {
        let payer = Pubkey::new_from_array([1; 32]);
        let program_id = Pubkey::new_from_array([2; 32]);
        let account_count = REPRESENTATIVE_COMMON_ACCOUNTS + MAX_FUNDING_ACCOUNTS;
        let addresses = (0..account_count)
            .map(|index| Pubkey::new_from_array([u8::try_from(index + 3).expect("key"); 32]))
            .collect::<vec::Vec<_>>();
        let accounts = addresses
            .iter()
            .map(|key| AccountMeta::new_readonly(*key, false))
            .collect::<vec::Vec<_>>();
        let role_bytes = CAPABILITY_EXECUTION_SELECTION_BYTES_V1
            + CAPABILITY_FUNDING_LIST_HEADER_BYTES_V1
            + REPRESENTATIVE_MAX_FAMILY_ACTIVATION_REQUEST_BYTES;
        assert_eq!(role_bytes, 416);
        let instruction_bytes = REQUEST_BYTES + CORE_EFFECT_ENVELOPE_BYTES_V1 + role_bytes;
        assert_eq!(instruction_bytes, 768);
        let instruction = Instruction {
            program_id,
            accounts,
            data: vec![0; instruction_bytes],
        };
        let lookup = AddressLookupTableAccount {
            key: Pubkey::new_from_array([254; 32]),
            addresses,
        };
        let message = v0::Message::try_compile(
            &payer,
            &[instruction],
            &[lookup],
            Hash::new_from_array([255; 32]),
        )
        .expect("maximum activation v0 message");
        let required_signatures = usize::from(message.header.num_required_signatures);
        let wire_bytes =
            1 + required_signatures * 64 + VersionedMessage::V0(message).serialize().len();
        assert_eq!(wire_bytes, 1_042);
        assert!(
            wire_bytes <= PACKET_DATA_BYTES,
            "{wire_bytes} > {PACKET_DATA_BYTES}"
        );
    }
}
