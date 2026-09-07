//! Family-neutral selected-capability root activation.
//!
//! A family authenticates its published activation artifacts and supplies their
//! derived record pairs.  This module owns the one Core/Trading activation
//! frame, caller authority, lookup-table submission, and the generic root and
//! funding-ledger postconditions.  Family code therefore cannot grow a second
//! root authority merely to create its particular root tail.

use dclutch_core_contract::ContentId;
use dclutch_market::{
    Action, CapabilityFundingHeaderV2, CoreEffectActionV1, CoreEffectEnvelopeV1, CoreState,
    Identity, Phase, Request, Role,
    capability_manifest::{
        CapabilityManifestV1, FundingLedgerStatusV2, FundingLedgerV2,
        capability_dependency_closure_mask_v1,
    },
    capability_program::{
        CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1, SelectedRecordBumpsV1,
    },
};
use dclutch_registry::{
    record::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId},
    release_set::{CallerAuthoritySeedsV1, CapabilityExecutionSelectionV1, ExecutionRoleV1},
};
use sha2::{Digest as _, Sha256};
use solana_program::hash::hash;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Error, Result,
    model::TransactionEvidence,
    rpc::{Rpc, RpcAccount},
};

/// Finalized raw/staging Registry coordinates used in an activation frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SelectedActivationRecordPairV1 {
    pub(crate) raw: Pubkey,
    pub(crate) staging: Pubkey,
    /// Registry schema under which the family authenticated this record.
    pub(crate) schema: [u8; 32],
    /// SHA-256 content identity of the exact record body.
    pub(crate) content: [u8; 32],
    /// Canonical bumps of the raw and closed staging PDAs.  Activation records
    /// these facts in the root so a later hot reader derives, rather than
    /// searches, its selected records.
    pub(crate) bumps: [u8; 2],
}

impl SelectedActivationRecordPairV1 {
    fn authenticate(self, registry: Pubkey, label: &str) -> Result<()> {
        let key = RecordKeyV1::new(
            SchemaReleaseId::new(self.schema).map_err(|error| {
                Error::new(format!("selected activation {label} schema: {error:?}"))
            })?,
            ContentDigest::new(self.content).map_err(|error| {
                Error::new(format!("selected activation {label} content: {error:?}"))
            })?,
        );
        let coordinate = |seeds: RecordPdaSeedsV1| {
            Pubkey::find_program_address(
                &[
                    seeds.domain(),
                    seeds.schema_release_id().as_bytes(),
                    seeds.expected_digest().as_bytes(),
                ],
                &registry,
            )
        };
        let (raw, raw_bump) = coordinate(key.raw_record_pda_seeds());
        let (staging, staging_bump) = coordinate(key.staging_cursor_pda_seeds());
        if self.raw != raw || self.staging != staging || self.bumps != [raw_bump, staging_bump] {
            return Err(Error::new(format!(
                "selected activation {label} record coordinates are noncanonical"
            )));
        }
        Ok(())
    }

    pub(crate) fn metas(self) -> [AccountMeta; 2] {
        [
            AccountMeta::new_readonly(self.raw, false),
            AccountMeta::new_readonly(self.staging, false),
        ]
    }
}

/// Family-authenticated facts consumed by the universal activation route.
///
/// The family owns validating every artifact body and its own root tail.  The
/// generic executor validates the Market/manifest/entry/funding facts and
/// never accepts an alternate root selection or Core authority context.
pub(crate) struct SelectedCapabilityActivationInputV1<'a> {
    pub(crate) market: Pubkey,
    pub(crate) market_account: &'a RpcAccount,
    pub(crate) current_slot: u64,
    pub(crate) core: Pubkey,
    pub(crate) core_programdata: Pubkey,
    pub(crate) trading: Pubkey,
    pub(crate) trading_programdata: Pubkey,
    pub(crate) resolution: Pubkey,
    pub(crate) resolution_programdata: Pubkey,
    pub(crate) registry: Pubkey,
    pub(crate) activation_cache: Pubkey,
    pub(crate) realm: SelectedActivationRecordPairV1,
    pub(crate) manifest: SelectedActivationRecordPairV1,
    pub(crate) manifest_body: &'a [u8],
    /// Manifest position re-derived by the family while it authenticated its
    /// selected closure.  Record PDA addresses intentionally carry no
    /// content identity, so the generic frame accepts this typed fact and
    /// proves it against the finalized manifest below.
    pub(crate) entry_index: u16,
    pub(crate) program_set: SelectedActivationRecordPairV1,
    /// SHA-256 of the authenticated ProgramSet body, which must equal the
    /// selected manifest entry's release identity.
    pub(crate) program_set_id: [u8; 32],
    pub(crate) config: SelectedActivationRecordPairV1,
    /// SHA-256 of the authenticated config body, which must equal the entry.
    pub(crate) config_id: [u8; 32],
    /// Capability kind decoded from the authenticated selected descriptor.
    pub(crate) capability_kind: [u8; 32],
    pub(crate) account_profile: SelectedActivationRecordPairV1,
    pub(crate) effect: SelectedActivationRecordPairV1,
    pub(crate) descriptor: SelectedActivationRecordPairV1,
    pub(crate) selected_funding_ledger: Pubkey,
    pub(crate) selected_funding_ledger_account: &'a RpcAccount,
    pub(crate) resolution_funding_ledger: Pubkey,
    /// The exact selected descriptor's activation body, validated by its
    /// family owner before it reaches this generic executor.
    pub(crate) family_activation_request: &'a [u8],
    /// Domain-separated family activation context.  It is opaque to Core but
    /// must be fixed by the family compiler, never copied from a CLI flag.
    pub(crate) context: [u8; 32],
}

/// Prepared universal activation instruction and its poststate expectations.
pub(crate) struct SelectedCapabilityActivationPlanV1 {
    pub(crate) instruction: Instruction,
    pub(crate) root: Pubkey,
    pub(crate) caller_authority: Pubkey,
    pub(crate) selection: CapabilityExecutionSelectionV1,
    pub(crate) expected_root_header: CapabilityRootHeaderV1,
    pub(crate) manifest_body: Vec<u8>,
    pub(crate) selected_funding_ledger_prestate: Vec<u8>,
    pub(crate) selected_funding_ledger: Pubkey,
    pub(crate) entry_index: u16,
    pub(crate) facts_slot: u64,
}

/// Generic poststate returned to the family tail verifier.
pub(crate) struct SelectedCapabilityActivationOutcomeV1 {
    pub(crate) activation: TransactionEvidence,
    pub(crate) routing_transactions: Vec<TransactionEvidence>,
    pub(crate) root_account: RpcAccount,
    pub(crate) funding_ledger_account: RpcAccount,
}

/// Build one immutable-root activation plan from finalized facts.
pub(crate) fn build_selected_capability_activation_plan_v1(
    input: SelectedCapabilityActivationInputV1<'_>,
) -> Result<SelectedCapabilityActivationPlanV1> {
    if input.current_slot == 0 || input.family_activation_request.is_empty() {
        return Err(Error::new(
            "selected-capability activation omitted finalized slot or family activation bytes",
        ));
    }
    for (label, pair) in [
        ("realm", input.realm),
        ("manifest", input.manifest),
        ("program set", input.program_set),
        ("config", input.config),
        ("account profile", input.account_profile),
        ("effect", input.effect),
        ("descriptor", input.descriptor),
    ] {
        pair.authenticate(input.registry, label)?;
    }
    let market = CoreState::decode(&input.market_account.data)
        .map_err(|error| Error::new(format!("selected activation Core Market: {error:?}")))?;
    if input.market_account.owner != input.core
        || market.phase != Phase::Open
        || market.identity.market_id.to_bytes() != input.market.to_bytes()
        || market.identity.registry_program.to_bytes() != input.registry.to_bytes()
    {
        return Err(Error::new(
            "selected-capability activation Market does not match its finalized Core state",
        ));
    }
    let manifest_id: [u8; 32] = Sha256::digest(input.manifest_body).into();
    if market.identity.capability_manifest.to_bytes() != manifest_id {
        return Err(Error::new(
            "selected-capability activation manifest differs from the Market identity",
        ));
    }
    let manifest = CapabilityManifestV1::decode(input.manifest_body)
        .map_err(|error| Error::new(format!("selected activation manifest: {error:?}")))?;
    let selected_index = input.entry_index;
    let entry = manifest
        .entry(selected_index)
        .map_err(|error| Error::new(format!("selected activation manifest entry: {error:?}")))?;
    if entry.release_id().to_bytes() != input.program_set_id
        || entry.config_id().to_bytes() != input.config_id
        || entry.kind_id().to_bytes() != input.capability_kind
    {
        return Err(Error::new(
            "selected-capability activation entry differs from its authenticated family closure",
        ));
    }
    if input.current_slot > entry.activation_deadline_slot() {
        return Err(Error::new(
            "selected-capability activation deadline elapsed before submission",
        ));
    }
    let selection = CapabilityExecutionSelectionV1::new(
        selected_index,
        ContentId::new(manifest_id)
            .map_err(|_| Error::new("selected activation manifest identity"))?,
        entry.kind_id(),
        entry.release_id(),
        entry.config_id(),
    )
    .map_err(|error| Error::new(format!("selected activation selection: {error:?}")))?;
    let root_header = CapabilityRootHeaderV1::new(
        ContentId::new(market.identity.selected_release_set.to_bytes())
            .map_err(|_| Error::new("selected activation release set identity"))?,
        input.market.to_bytes(),
        market.identity.generation,
        selection.with_capability_release_record_bumps(
            input.program_set.bumps[0],
            input.program_set.bumps[1],
        ),
        SelectedRecordBumpsV1::new(
            input.manifest.bumps[0],
            input.manifest.bumps[1],
            input.config.bumps[0],
            input.config.bumps[1],
        ),
    )
    .map_err(|error| Error::new(format!("selected activation root header: {error:?}")))?;
    let root = Pubkey::find_program_address(&root_header.seeds().as_slices(), &input.trading).0;

    let closure_mask =
        capability_dependency_closure_mask_v1(manifest, selected_index).map_err(|error| {
            Error::new(format!("selected activation dependency closure: {error:?}"))
        })?;
    let required_union = crate::market::manifest_required_union_v1(manifest.entry_count())?;
    if closure_mask != required_union {
        return Err(Error::new(
            "selected-capability activation dependency closure does not cover its manifest",
        ));
    }
    let ledger = FundingLedgerV2::decode(&input.selected_funding_ledger_account.data)
        .map_err(|error| Error::new(format!("selected activation funding ledger: {error:?}")))?;
    if input.selected_funding_ledger_account.owner != input.trading
        || ledger
            .authenticate(
                ContentId::new(manifest_id)
                    .map_err(|_| Error::new("selected activation manifest identity"))?,
                manifest,
            )
            .map_err(|error| Error::new(format!("selected activation funding ledger: {error:?}")))?
            .slot(selected_index)
            .map_err(|error| Error::new(format!("selected activation funding slot: {error:?}")))?
            .status()
            != FundingLedgerStatusV2::Pending
    {
        return Err(Error::new(
            "selected-capability activation funding ledger is not pending for its entry",
        ));
    }

    let funding = CapabilityFundingHeaderV2::new(
        2,
        u8::try_from(closure_mask.count_ones())
            .map_err(|_| Error::new("selected activation funding width"))?,
        closure_mask,
    )
    .map_err(|error| Error::new(format!("selected activation funding header: {error:?}")))?;
    let mut role_request = Vec::new();
    role_request.extend_from_slice(&selection.to_bytes());
    role_request.extend_from_slice(&funding.encode());
    role_request.extend_from_slice(input.family_activation_request);
    let role_request_digest = hash(&role_request).to_bytes();
    let caller_authority = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::from_bytes(
            market.identity.selected_release_set.to_bytes(),
            input.market.to_bytes(),
            ExecutionRoleV1::Core,
            input.context,
            role_request_digest,
        )
        .map_err(|error| Error::new(format!("selected activation authority: {error:?}")))?
        .as_slices(),
        &input.core,
    )
    .0;
    let envelope = CoreEffectEnvelopeV1::new(
        CoreEffectActionV1::ActivateCapability,
        Role::Trading,
        Identity::new(input.core.to_bytes())
            .map_err(|_| Error::new("selected activation Core identity"))?,
        Identity::new(caller_authority.to_bytes())
            .map_err(|_| Error::new("selected activation caller identity"))?,
        Identity::new(market.identity.selected_release_set.to_bytes())
            .map_err(|_| Error::new("selected activation release identity"))?,
        Identity::new(input.market.to_bytes())
            .map_err(|_| Error::new("selected activation Market identity"))?,
        Identity::new(input.context)
            .map_err(|_| Error::new("selected activation context identity"))?,
        Identity::new(hash(&input.market_account.data).to_bytes())
            .map_err(|_| Error::new("selected activation Market digest"))?,
        Identity::new(role_request_digest)
            .map_err(|_| Error::new("selected activation request digest"))?,
        market.identity.generation,
        0,
        0,
        u32::try_from(role_request.len())
            .map_err(|_| Error::new("selected activation request width"))?,
    )
    .map_err(|error| Error::new(format!("selected activation Core envelope: {error:?}")))?;
    let request = Request::administrative(
        Action::ActivateCapability,
        market.identity.generation,
        Identity::new(input.market.to_bytes())
            .map_err(|_| Error::new("selected activation Market identity"))?,
    );
    let mut data = request
        .encode()
        .map_err(|error| Error::new(format!("selected activation Core request: {error:?}")))?
        .to_vec();
    data.extend_from_slice(
        &envelope
            .encode()
            .map_err(|error| Error::new(format!("selected activation Core envelope: {error:?}")))?,
    );
    data.extend_from_slice(&role_request);

    let realm = input.realm.metas();
    let manifest_pair = input.manifest.metas();
    let program_set = input.program_set.metas();
    let config = input.config.metas();
    let profile = input.account_profile.metas();
    let effect = input.effect.metas();
    let descriptor = input.descriptor.metas();
    let funding_slice = crate::market::ordered_funding_ledger_slice_v1(
        selected_index,
        AccountMeta::new(input.selected_funding_ledger, false),
        AccountMeta::new_readonly(input.resolution_funding_ledger, false),
    );
    let accounts = vec![
        AccountMeta::new(input.market, false),
        realm[0].clone(),
        realm[1].clone(),
        manifest_pair[0].clone(),
        manifest_pair[1].clone(),
        funding_slice[0].clone(),
        funding_slice[1].clone(),
        AccountMeta::new(root, false),
        AccountMeta::new_readonly(input.activation_cache, false),
        AccountMeta::new_readonly(input.core, false),
        AccountMeta::new_readonly(input.core_programdata, false),
        AccountMeta::new_readonly(input.trading, false),
        AccountMeta::new_readonly(input.trading_programdata, false),
        AccountMeta::new_readonly(input.resolution, false),
        AccountMeta::new_readonly(input.resolution_programdata, false),
        AccountMeta::new_readonly(input.registry, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(caller_authority, false),
        program_set[0].clone(),
        program_set[1].clone(),
        config[0].clone(),
        config[1].clone(),
        profile[0].clone(),
        profile[1].clone(),
        effect[0].clone(),
        effect[1].clone(),
        AccountMeta::new_readonly(input.activation_cache, false),
        AccountMeta::new_readonly(input.core, false),
        AccountMeta::new_readonly(input.core_programdata, false),
        AccountMeta::new_readonly(input.trading, false),
        AccountMeta::new_readonly(input.trading_programdata, false),
        AccountMeta::new_readonly(input.registry, false),
        AccountMeta::new_readonly(sysvar::rent::ID, false),
        AccountMeta::new_readonly(system_program::ID, false),
        descriptor[0].clone(),
        descriptor[1].clone(),
    ];
    Ok(SelectedCapabilityActivationPlanV1 {
        instruction: Instruction {
            program_id: input.core,
            accounts,
            data,
        },
        root,
        caller_authority,
        selection,
        expected_root_header: root_header,
        manifest_body: input.manifest_body.to_vec(),
        selected_funding_ledger_prestate: input.selected_funding_ledger_account.data.clone(),
        selected_funding_ledger: input.selected_funding_ledger,
        entry_index: selected_index,
        facts_slot: input.current_slot,
    })
}

/// Execute a prepared activation and prove only the family-neutral poststate.
pub(crate) fn execute_selected_capability_activation_v1(
    rpc: &mut Rpc,
    payer: &Keypair,
    plan: &SelectedCapabilityActivationPlanV1,
    label: &str,
) -> Result<SelectedCapabilityActivationOutcomeV1> {
    let mut routing_transactions = Vec::new();
    let (observation, tables) = crate::market::publish_routing_table(
        rpc,
        payer,
        label,
        std::slice::from_ref(&plan.instruction),
        &mut routing_transactions,
    )?;
    let activation = rpc.send_v0(
        label,
        std::slice::from_ref(&plan.instruction),
        payer,
        observation,
        &tables,
    )?;
    let root_account = rpc.required_account(plan.root, "activated selected capability root")?;
    if root_account.owner != plan.instruction.accounts[11].pubkey {
        return Err(Error::new(
            "selected-capability activation root owner differs from Trading",
        ));
    }
    let header = CapabilityRootHeaderV1::decode(
        root_account
            .data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or_else(|| Error::new("selected-capability activation root is truncated"))?,
    )
    .map_err(|error| Error::new(format!("selected activation root header: {error:?}")))?;
    if header != plan.expected_root_header {
        return Err(Error::new(
            "selected-capability activation root header differs from its plan",
        ));
    }
    let ledger_after = rpc.required_account(
        plan.selected_funding_ledger,
        "selected funding ledger poststate",
    )?;
    let manifest = CapabilityManifestV1::decode(&plan.manifest_body).map_err(|error| {
        Error::new(format!("selected activation manifest poststate: {error:?}"))
    })?;
    let manifest_id: [u8; 32] = Sha256::digest(&plan.manifest_body).into();
    let manifest_id = ContentId::new(manifest_id)
        .map_err(|_| Error::new("selected activation manifest identity"))?;
    let after = FundingLedgerV2::decode(&ledger_after.data)
        .map_err(|error| Error::new(format!("selected activation ledger poststate: {error:?}")))?;
    let activation_slot = after
        .authenticate(manifest_id, manifest)
        .map_err(|error| Error::new(format!("selected activation ledger poststate: {error:?}")))?
        .slot(plan.entry_index)
        .map_err(|error| Error::new(format!("selected activation ledger slot: {error:?}")))?
        .activation_slot();
    let mut expected = plan.selected_funding_ledger_prestate.clone();
    if activation_slot == 0 {
        return Err(Error::new(
            "selected-capability activation ledger poststate is incomplete",
        ));
    }
    FundingLedgerV2::activate_in_place(
        &mut expected,
        manifest_id,
        manifest,
        plan.entry_index,
        activation_slot,
    )
    .map_err(|error| Error::new(format!("selected activation ledger transition: {error:?}")))?;
    if expected != ledger_after.data {
        return Err(Error::new(
            "selected-capability activation funding ledger differs from its canonical transition",
        ));
    }
    Ok(SelectedCapabilityActivationOutcomeV1 {
        activation,
        routing_transactions,
        root_account,
        funding_ledger_account: ledger_after,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(
        registry: Pubkey,
        schema: [u8; 32],
        content: [u8; 32],
    ) -> SelectedActivationRecordPairV1 {
        let key = RecordKeyV1::new(
            SchemaReleaseId::new(schema).expect("nonzero test schema"),
            ContentDigest::new(content).expect("nonzero test content"),
        );
        let coordinate = |seeds: RecordPdaSeedsV1| {
            Pubkey::find_program_address(
                &[
                    seeds.domain(),
                    seeds.schema_release_id().as_bytes(),
                    seeds.expected_digest().as_bytes(),
                ],
                &registry,
            )
        };
        let (raw, raw_bump) = coordinate(key.raw_record_pda_seeds());
        let (staging, staging_bump) = coordinate(key.staging_cursor_pda_seeds());
        SelectedActivationRecordPairV1 {
            raw,
            staging,
            schema,
            content,
            bumps: [raw_bump, staging_bump],
        }
    }

    #[test]
    fn selected_record_pair_refuses_a_bump_not_derived_from_its_authenticated_content() {
        let registry = Pubkey::new_unique();
        let schema = [7; 32];
        let content = [9; 32];
        let mut observed = pair(registry, schema, content);
        observed.bumps[1] ^= 1;
        let error = observed
            .authenticate(registry, "config")
            .expect_err("a substituted recorded bump must refuse");
        assert_eq!(
            error.to_string(),
            "selected activation config record coordinates are noncanonical"
        );
    }
}
