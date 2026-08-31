use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunProgramInput {
    pub(crate) program_id: String,
    pub(crate) elf_path: String,
    pub(crate) elf_sha256: String,
    pub(crate) semantic_release_id: String,
    pub(crate) attestation: String,
    /// Additive and optional. A complete Loader V3 `ProgramData` account body
    /// observed on a cluster: present, the role's release is minted from that
    /// observation and its deployment slot is decoded out of it. This is the
    /// deploy-day shape.
    #[serde(default)]
    pub(crate) observed_programdata: Option<String>,
    /// SHA-256 of the complete live ELF tail inside `observed_programdata`,
    /// including any Loader allocation padding. Required exactly when an
    /// observed ProgramData account is supplied. `elf_sha256` remains the
    /// checked raw build-candidate digest.
    #[serde(default)]
    pub(crate) observed_elf_sha256: Option<String>,
    /// Additive and optional, and **local rehearsal only**. The slot written
    /// into the genesis install this plan materializes, so a local campaign can
    /// exercise a nonzero deployment slot end to end instead of the zero a
    /// genesis install would otherwise have. It is refused together with
    /// `observed_programdata`: an observation is not a caller's to overwrite.
    #[serde(default)]
    pub(crate) genesis_deployment_slot: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SuccessorRunSpec {
    pub(crate) schema: String,
    pub(crate) rpc_url: String,
    pub(crate) launcher: String,
    pub(crate) ledger: String,
    pub(crate) account_dir: String,
    pub(crate) plan: String,
    pub(crate) output: String,
    pub(crate) registry: RunProgramInput,
    pub(crate) core: RunProgramInput,
    pub(crate) claims: RunProgramInput,
    pub(crate) trading: RunProgramInput,
    pub(crate) resolution: RunProgramInput,
    pub(crate) custody: RunProgramInput,
    pub(crate) rent_credit: RunProgramInput,
    pub(crate) market: MarketRunInput,
    /// Additive and optional: absent means `"genesis"`, which is exactly the
    /// behaviour every v2 spec written before this field had. `"transaction"`
    /// removes the nine infrastructure record bodies from genesis and makes
    /// the supervisor publish them on chain, which is the only shape a real
    /// cluster can reach.
    #[serde(default)]
    pub(crate) record_publication: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct MarketRunInput {
    pub(crate) generation: u64,
    pub(crate) collateral_display_decimals: u8,
    /// Local-validator-only collateral kept outside every Hoard principal so
    /// the participant exterior can execute a real Token-2022 transfer after
    /// founding. Zero is the only public/devnet value; the local fixture uses
    /// the one exact amount authenticated by the campaign.
    pub(crate) local_participant_fixture_liquidity_atoms: u64,
    pub(crate) initial_collateral_atoms: u64,
    pub(crate) product_id: String,
    pub(crate) coordinate_domain_id: String,
    pub(crate) result_unit_id: String,
    pub(crate) claim_basis_id: String,
    pub(crate) liability_basis_id: String,
    pub(crate) representation_release_id: String,
    pub(crate) mapping_release_id: String,
    pub(crate) cut_denominator: u64,
    pub(crate) cuts: Vec<String>,
    pub(crate) portfolio_denominator: u64,
    pub(crate) coefficients: Vec<u64>,
    pub(crate) primary_source_spec_id: String,
    pub(crate) window_spec_id: String,
    pub(crate) statistic_spec_id: String,
    pub(crate) failure_policy_release_id: String,
    /// The five source-graph record BODIES whose digests the four identities
    /// above name.
    ///
    /// A finalized record lives at an address derived from the hash of its own
    /// body, so an identity that is not the hash of a body names a record
    /// nobody can ever publish. These fields exist because the demo Market's
    /// source graph used to carry domain-separated demo digests in those four
    /// slots: the Market could create and activate its Resolution funding and
    /// then never reach a certificate, because both provider legs authenticate
    /// the source spec, window spec and statistic spec as finalized records.
    /// The run spec carries the exact bodies for the same reason it already
    /// carries `linked_basis_hex` rather than an opaque digest.
    pub(crate) source_spec_hex: String,
    /// Exact SourceCapacityProfileV1 body named by the SourceSpec.
    pub(crate) source_capacity_profile_hex: String,
    /// ManipulationFloorV1 policy template. Empty selects explicit unbounded;
    /// nonempty is rebound to the immutable Realm mint before publication.
    pub(crate) manipulation_floor_hex: String,
    pub(crate) window_spec_hex: String,
    pub(crate) statistic_spec_hex: String,
    pub(crate) provider_release_hex: String,
    pub(crate) pyth_adapter_config_hex: String,
    /// Exact canonical `PythSponsoredPushReleaseV1` body selected by a
    /// `PythSponsoredPushSnapshot` source. Empty for every other access
    /// profile. The body is published as its own Registry record; the
    /// ProviderRelease names it by SHA-256.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) pyth_sponsored_push_release_hex: String,
    pub(crate) recovery_policy_hex: String,
    pub(crate) capability_manifest_hex: String,
    /// Complete Direct record closure selected by the one non-Resolution
    /// manifest entry. The field is syntactically required; `None` exists only
    /// while an in-process producer is assembling the Resolution base and is
    /// refused by `validate_market_input`.
    pub(crate) direct_capability: Option<DirectMarketCapabilityV1>,
    /// Family-neutral selected-capability closure for the one non-Resolution
    /// manifest entry, in the byte shape the selection seam consumes. Exactly
    /// one of this and `direct_capability` must be present:
    /// `validate_market_input` refuses zero closures and refuses two.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) selected_capability: Option<SelectedCapabilityV1>,
    pub(crate) linked_basis_hex: String,
}

/// One Registry record a selected capability's publication chain finalizes.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectedCapabilityRecordV1 {
    /// Operator-facing name; also the founding-evidence label, so it must be
    /// unique within one closure.
    pub(crate) label: String,
    /// Schema/release identity the record finalizes under, read off the
    /// release's own artifacts by the family compiler.
    pub(crate) schema_hex: String,
    /// Exact semantic bytes.
    pub(crate) body_hex: String,
}

/// One family-neutral selected-capability closure, serialized.
///
/// Every byte field is the family release compiler's own output; the driver
/// derives the manifest entry from these through the selection seam
/// (`selected_capability.rs`) and restates nothing. An additional family is
/// this payload plus its publication — not new driver code.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SelectedCapabilityV1 {
    /// Operator-facing family label (for example "general").
    pub(crate) family: String,
    /// Exact `CapabilityProgramSetV2` bytes.
    pub(crate) program_set_hex: String,
    /// The selected V4 descriptor authoring the entry's kind, capacity
    /// profile, root schema, and derivation policy.
    pub(crate) selected_descriptor_hex: String,
    /// Exact config record body; must be derivable before the Market exists
    /// (the seam's fixed-point invariant).
    pub(crate) config_hex: String,
    /// The family's canonical Market-bindable publication bytes.
    pub(crate) publication_hex: String,
    /// Every record the Registry must finalize for this release.
    pub(crate) records: Vec<SelectedCapabilityRecordV1>,
    pub(crate) activation_deadline_slot: u64,
    pub(crate) root_rent_minimum_lamports: u64,
    pub(crate) selected_manifest_entry_index: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectMarketCapabilityV1 {
    pub(crate) execution_config_hex: String,
    pub(crate) ordinary_account_profile_hex: String,
    pub(crate) ordinary_lifecycle_policy_hex: String,
    pub(crate) ordinary_request_profile_hex: String,
    pub(crate) ordinary_transition_hex: String,
    pub(crate) ordinary_strategy_hex: String,
    pub(crate) ordinary_effect_hex: String,
    pub(crate) ordinary_descriptor_hex: String,
    pub(crate) begin_retiring_account_profile_hex: String,
    pub(crate) begin_retiring_effect_hex: String,
    pub(crate) begin_retiring_descriptor_hex: String,
    pub(crate) native_close_account_profile_hex: String,
    pub(crate) native_close_effect_hex: String,
    pub(crate) native_close_descriptor_hex: String,
    /// The capability-activation artifact trio. Defaulted (empty) so market
    /// inputs sealed before the activation entry existed still parse for
    /// terminal and evidence paths; every authoring, founding, and trade path
    /// refuses an input whose activation artifacts are absent, because such an
    /// input describes a market whose capability can never activate.
    #[serde(default)]
    pub(crate) activation_account_profile_hex: String,
    #[serde(default)]
    pub(crate) activation_effect_hex: String,
    #[serde(default)]
    pub(crate) activation_descriptor_hex: String,
    /// The maker-replay close artifact trio (cohort-9's fifth ProgramSet
    /// entry, wall 22's missing decrement). Defaulted (empty) for the same
    /// reason the activation trio is: market inputs sealed before this entry
    /// existed must still parse for terminal and evidence paths, and every
    /// authoring path refuses an input whose close artifacts are absent --
    /// such an input describes a market that can never retire once filled.
    #[serde(default)]
    pub(crate) close_maker_account_profile_hex: String,
    #[serde(default)]
    pub(crate) close_maker_effect_hex: String,
    #[serde(default)]
    pub(crate) close_maker_descriptor_hex: String,
    pub(crate) program_set_hex: String,
    pub(crate) activation_deadline_slot: u64,
    pub(crate) root_rent_minimum_lamports: u64,
    pub(crate) selected_manifest_entry_index: u16,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct ProgramPin {
    pub(crate) program_id: String,
    pub(crate) programdata_id: String,
    /// Compatibility alias for `checked_candidate_elf_path`; always exact.
    pub(crate) elf_path: String,
    /// Compatibility alias for `checked_candidate_elf_sha256`; never the live
    /// padded payload digest.
    pub(crate) elf_sha256: String,
    /// Checked raw build output used for deployment.
    pub(crate) checked_candidate_elf_path: String,
    /// SHA-256 of the exact raw build candidate.
    pub(crate) checked_candidate_elf_sha256: String,
    /// SHA-256 of the complete live ProgramData ELF tail. This is the digest
    /// bound by `ArtifactReleaseV1` and may differ from the raw candidate only
    /// because of an all-zero allocation suffix.
    pub(crate) live_elf_sha256: String,
    /// Number of proven all-zero bytes after the checked candidate in the live
    /// ProgramData ELF tail.
    pub(crate) live_elf_padding_bytes: usize,
    pub(crate) semantic_release_id: String,
    pub(crate) artifact_release_id: String,
    pub(crate) upgrade_authority: Option<String>,
    /// The slot this role's `ArtifactReleaseV1` binds, hostile-decoded out of a
    /// Loader V3 `ProgramData` account image by the same reader the on-chain
    /// `authenticate_deployment` uses. Never a caller-supplied number.
    pub(crate) deployment_slot: u64,
    /// `"genesis-install"` or `"observed-programdata-account"`. A genesis
    /// install has no deploy transaction and so no slot to observe; only the
    /// second value describes a shape a cluster can be in.
    pub(crate) deployment_source: String,
    /// SHA-256 of the exact `ProgramData` account image the slot was decoded
    /// from. For an observed account this is the only carrier of the retained
    /// former-authority bytes, which no `(elf, slot, authority)` triple
    /// regenerates.
    pub(crate) programdata_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct RecordPair {
    pub(crate) raw: String,
    pub(crate) staging: String,
    pub(crate) schema_id: String,
    pub(crate) content_sha256: String,
    /// The complete record body. Under `record_publication = "genesis"` this
    /// duplicates what the account file already holds; under `"transaction"`
    /// it is the only carrier of the bytes, because nothing writes them at
    /// genesis and the supervisor has to publish them through Registry
    /// `Begin -> Append -> Finalize` like any other record.
    pub(crate) body_hex: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct InfrastructureProfilePin {
    pub(crate) address: String,
    pub(crate) schema_id: String,
    pub(crate) body_sha256: String,
    pub(crate) body_hex: String,
    pub(crate) registry_artifact_release_id: String,
    pub(crate) rent_artifact_release_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct CoreBootstrapPin {
    pub(crate) upgrade_authority: String,
    pub(crate) genesis_programdata_sha256: String,
    pub(crate) post_revoke_programdata_sha256: String,
    pub(crate) release_recognition_requires_revoke: bool,
}

/// How one permanent devnet role entered the checked deployment set.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CheckedDeploymentDispositionV1 {
    /// A complete, freshly reauthenticated one-role Loader Upgrade receipt.
    Upgrade,
    /// An authenticated existing deployment and existing finalized artifact.
    CarryForward,
    /// The role's live payload was READ BACK from a finalized cluster
    /// observation and found byte-identical to the checked candidate, so no
    /// Upgrade transaction exists, and none can: the Loader refuses to replace
    /// a payload with itself and the tool refuses the replay ambiguity.
    ///
    /// This is deliberately a THIRD kind and never either of the other two.
    ///
    /// It is not an `Upgrade`: no receipt, no signature, no buffer, no fee. It
    /// must never be counted or displayed as one.
    ///
    /// It is not `CarryForward` either, and the difference is the whole point.
    /// Carry-forward is a whitelist: it asserts that two named roles were not
    /// part of this cut and says NOTHING about their bytes. This asserts
    /// EQUALITY -- that the bytes on chain are the bytes the checked gate
    /// binds -- and it is refused unless that equality is read back from a
    /// finalized observation at audit time.
    ///
    /// On evidence strength this is the stronger claim, which is why it is
    /// admitted at all: an Upgrade receipt ARGUES from a transaction that the
    /// right bytes arrived, while this READS the deployed bytes and compares
    /// them to the candidate digest. The weaker link in a receipt-backed role
    /// is the argument; here there is no argument.
    AlreadyCurrent,
}

/// The one statement of what an `AlreadyCurrent` row may and may not carry.
///
/// Two facts, and they are the two that EVERY projection of a deployment-set
/// row can see: whether the row claims an Upgrade receipt, and whether it
/// carries the pinned baseline that fixes the width its equality was judged at.
///
/// This rule used to be written five times, across four functions in three
/// files, with five unrelated error strings -- and three of those strings never
/// named the field that was wrong, so an operator reading one learned only that
/// some closure somewhere had failed. The projections genuinely differ: a
/// journal row carries pinned equality evidence that a plan pin has no field
/// for, and only the audit sees a live cluster. So each site still adds the
/// conjuncts only it can see. What no site may do any more is restate THIS.
pub(crate) struct AlreadyCurrentClosureV1<'a> {
    /// The role this row is about, named in every refusal.
    pub(crate) role: &'a str,
    /// The row claims an Upgrade receipt, in any of the shapes a row can.
    pub(crate) receipt_claimed: bool,
    /// The row carries a well-formed pinned baseline.
    pub(crate) baseline_present: bool,
}

impl AlreadyCurrentClosureV1<'_> {
    /// Audit the shared closure, refusing with the field that is wrong.
    pub(crate) fn audit(&self) -> crate::Result<()> {
        if self.receipt_claimed {
            return Err(crate::Error::new(format!(
                "AlreadyCurrent role {}: an Upgrade receipt is claimed, and the two kinds of \
                 evidence are exclusive -- no Upgrade exists for a payload that already IS the \
                 checked candidate, and the Loader refuses to build one",
                self.role
            )));
        }
        if !self.baseline_present {
            return Err(crate::Error::new(format!(
                "AlreadyCurrent role {}: the pinned baseline is absent or malformed, and it is \
                 what fixes the width the equality was judged at",
                self.role
            )));
        }
        Ok(())
    }

    /// Whether the shared closure holds.
    ///
    /// For the two sites that compose this rule into a larger boolean instead
    /// of returning early. They keep their own outer refusal; they no longer
    /// keep their own copy of the rule.
    pub(crate) fn holds(&self) -> bool {
        self.audit().is_ok()
    }
}

/// One permanent devnet role and the exact evidence that authorized its
/// release-plan projection. Upgrade-only and carry-forward-only fields are
/// optional at the wire level so serde can represent the tagged union; the
/// deployment-set authenticator enforces the exact field closure for each tag.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckedUpgradeRolePinV1 {
    pub(crate) role: String,
    pub(crate) disposition: CheckedDeploymentDispositionV1,
    pub(crate) program_id: String,
    pub(crate) programdata_id: String,
    pub(crate) baseline_path: Option<String>,
    pub(crate) baseline_sha256: Option<String>,
    pub(crate) receipt_path: Option<String>,
    pub(crate) receipt_sha256: Option<String>,
    pub(crate) dump_path: String,
    pub(crate) dump_sha256: String,
    pub(crate) checked_candidate_elf_path: String,
    pub(crate) checked_candidate_elf_sha256: String,
    pub(crate) live_elf_sha256: String,
    pub(crate) deployment_slot: u64,
    pub(crate) programdata_account_sha256: String,
    pub(crate) semantic_release_id: String,
    /// Exact existing `ArtifactReleaseV1` body/id for CarryForward; absent for
    /// an Upgrade, whose new body is derived by checked prepare.
    pub(crate) artifact_release_body_hex: Option<String>,
    pub(crate) artifact_release_id: Option<String>,
    /// Exact live ProgramData account body from the carry-forward snapshot.
    /// This is a derived transport projection, never caller-authored prepare
    /// authority, and is absent for receipt-backed Upgrade roles.
    pub(crate) carried_programdata_base64: Option<String>,
}

impl CheckedUpgradeRolePinV1 {
    /// Project this pin onto the shared `AlreadyCurrent` closure.
    ///
    /// The baseline standard is the STRICTER of the two the sites used to
    /// carry: a present, non-empty path and a present, well-formed 32-byte
    /// digest. One site checked only presence and the other checked
    /// well-formedness, for the same field on the same type -- so the weaker
    /// one admitted rows the stronger one refused, and nothing compared them.
    pub(crate) fn already_current_closure(&self) -> AlreadyCurrentClosureV1<'_> {
        AlreadyCurrentClosureV1 {
            role: &self.role,
            receipt_claimed: self.receipt_path.is_some() || self.receipt_sha256.is_some(),
            baseline_present: self
                .baseline_path
                .as_deref()
                .is_some_and(|path| !path.is_empty())
                && self
                    .baseline_sha256
                    .as_deref()
                    .is_some_and(|digest| crate::plan::hex32(digest).is_ok()),
        }
    }

    /// Whether none of the carry-forward-only transport fields is present.
    ///
    /// Shared by the two sites that judge a cut-owned row, which had
    /// byte-identical copies of this three-field predicate.
    pub(crate) fn carries_no_transport_fields(&self) -> bool {
        self.artifact_release_body_hex.is_none()
            && self.artifact_release_id.is_none()
            && self.carried_programdata_base64.is_none()
    }
}

/// Exact singleton infrastructure evidence shared by the two CarryForward
/// rows. All projected addresses and bodies are rederived while admitting the
/// referenced one-context snapshot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckedInfrastructureCarryForwardPinV1 {
    pub(crate) snapshot_path: String,
    pub(crate) snapshot_sha256: String,
    pub(crate) context_slot: u64,
    pub(crate) profile_address: String,
    pub(crate) profile_account_sha256: String,
    pub(crate) profile_body_sha256: String,
    pub(crate) profile_body_hex: String,
    pub(crate) registry_raw_address: String,
    pub(crate) registry_staging_address: String,
    pub(crate) registry_programdata_account_sha256: String,
    pub(crate) rent_raw_address: String,
    pub(crate) rent_staging_address: String,
    pub(crate) rent_programdata_account_sha256: String,
}

/// Canonical mixed permanent deployment-set evidence consumed by checked
/// prepare: Registry/Rent CarryForward plus five receipt-backed Upgrades.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckedUpgradeSetPinV1 {
    pub(crate) schema: String,
    pub(crate) journal_path: String,
    pub(crate) journal_sha256: String,
    pub(crate) final_set_sha256: String,
    pub(crate) checked_release_gate_path: String,
    pub(crate) checked_release_gate_sha256: String,
    pub(crate) source_revision: String,
    pub(crate) source_tree_sha256: String,
    pub(crate) devnet_genesis_hash: String,
    pub(crate) solana_cli_version: String,
    pub(crate) retained_upgrade_authority: String,
    pub(crate) fee_payer: String,
    pub(crate) semantic_derivation: String,
    pub(crate) infrastructure_carry_forward: CheckedInfrastructureCarryForwardPinV1,
    pub(crate) roles: Vec<CheckedUpgradeRolePinV1>,
}

/// One exact mutable Loader pair installed into a fresh localhost validator.
///
/// This is not a devnet Upgrade receipt.  It binds a checked-release ELF to
/// the ProgramData image that the local launcher installs, including the
/// retained authority and synthetic nonzero deployment slot.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckedLocalMutableRolePinV1 {
    pub(crate) role: String,
    pub(crate) program_id: String,
    pub(crate) programdata_id: String,
    pub(crate) checked_candidate_elf_path: String,
    pub(crate) checked_candidate_elf_sha256: String,
    pub(crate) live_elf_sha256: String,
    pub(crate) programdata_account_sha256: String,
    pub(crate) deployment_slot: u64,
    pub(crate) semantic_release_id: String,
}

/// One complete deployment-bound `CheckedReleaseV1` in canonical execution
/// role order. The manifest bytes are retained because the compact
/// multiprogram envelope carries only their content identities.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckedLocalExecutionReleaseRolePinV1 {
    pub(crate) role: String,
    pub(crate) checked_release_id: String,
    pub(crate) checked_release_base64: String,
}

/// Exact five-role local execution evidence derived from one checked build
/// gate and the seven authenticated local Loader pairs.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckedLocalExecutionReleaseSetPinV1 {
    pub(crate) schema: String,
    pub(crate) checked_execution_release_set_id: String,
    pub(crate) execution_release_set_id: String,
    pub(crate) checked_execution_release_set_base64: String,
    pub(crate) roles: Vec<CheckedLocalExecutionReleaseRolePinV1>,
}

/// Checked provenance for the seven-role decision-0012 localhost substrate.
///
/// Permanent-devnet plans continue to require `checked_upgrade_set`; this
/// sibling is admitted only for loopback and is rooted in a fresh checked
/// release gate plus exact genesis ProgramData images.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CheckedLocalMutableSetPinV1 {
    pub(crate) schema: String,
    pub(crate) checked_release_gate_path: String,
    pub(crate) checked_release_gate_sha256: String,
    pub(crate) source_revision: String,
    pub(crate) source_tree_sha256: String,
    pub(crate) solana_cli_version: String,
    pub(crate) retained_upgrade_authority: String,
    pub(crate) execution_release_set: CheckedLocalExecutionReleaseSetPinV1,
    pub(crate) set_sha256: String,
    pub(crate) roles: Vec<CheckedLocalMutableRolePinV1>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct GenesisAccountPin {
    pub(crate) address: String,
    pub(crate) owner: String,
    pub(crate) lamports: u64,
    pub(crate) data_len: usize,
    pub(crate) data_sha256: String,
    pub(crate) account_sha256: String,
    pub(crate) json_file_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct SuccessorPlan {
    pub(crate) schema: String,
    pub(crate) genesis_boundary: Vec<String>,
    pub(crate) bootstrap_order: Vec<String>,
    pub(crate) execution_blocker: String,
    pub(crate) account_dir: String,
    pub(crate) registry: ProgramPin,
    pub(crate) core: ProgramPin,
    pub(crate) claims: ProgramPin,
    pub(crate) trading: ProgramPin,
    pub(crate) resolution: ProgramPin,
    pub(crate) custody: ProgramPin,
    pub(crate) rent_credit: ProgramPin,
    pub(crate) activation: String,
    pub(crate) release_set_id: String,
    pub(crate) core_bootstrap: CoreBootstrapPin,
    /// Present only when `prepare` authenticated the complete permanent-devnet
    /// Upgrade journal instead of accepting caller-authored release facts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) checked_upgrade_set: Option<CheckedUpgradeSetPinV1>,
    /// Present only for an exact checked-release set installed into a fresh
    /// localhost validator with all seven decision-0012 authorities retained.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) checked_local_mutable_set: Option<CheckedLocalMutableSetPinV1>,
    pub(crate) infrastructure_profile: InfrastructureProfilePin,
    pub(crate) records: BTreeMap<String, RecordPair>,
    /// `"genesis"` or `"transaction"`. Devnet has no genesis, so the
    /// transaction mode is the one a real deployment can actually reach; the
    /// genesis mode exists because it is what every campaign to date has run.
    pub(crate) record_publication: String,
    pub(crate) provider_release_id: String,
    pub(crate) fixture_publish_time: i64,
    pub(crate) genesis_accounts: BTreeMap<String, GenesisAccountPin>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct TransactionEvidence {
    pub(crate) label: String,
    pub(crate) signature: String,
    pub(crate) slot: u64,
    pub(crate) transaction_metadata_available: bool,
    pub(crate) fee_lamports: Option<u64>,
    /// Whether this transaction's own balance record says it moved NOTHING
    /// but the payer's fee: `postBalances[0] + fee == preBalances[0]` and
    /// every other pre/post pair equal (static and loaded addresses alike).
    ///
    /// This is the rollback property read from ONE atomic record the chain
    /// wrote, instead of separate before/after account reads that race a
    /// load-balanced endpoint's replicas (measured 2026-08-28: two devnet
    /// founding attempts died at a rollback check whose on-chain arithmetic
    /// was exact). `None` when the transaction metadata carried no balances.
    pub(crate) fee_only_balance_change: Option<bool>,
    pub(crate) compute_units_consumed: Option<u64>,
    pub(crate) error: Option<serde_json::Value>,
    pub(crate) logs: Vec<String>,
}

impl TransactionEvidence {
    /// The program refusal code this transaction actually earned, if any.
    ///
    /// `error` carries the runtime's whole `TransactionError`. A program
    /// refusal is exactly `{"InstructionError":[index,{"Custom":code}]}`;
    /// everything else the runtime can say — a missing signature, a lamport
    /// shortfall, an unknown instruction, a blown compute meter — is a
    /// different kind of failure and returns `None` here. That distinction is
    /// the one a hostile most needs, because "the transaction failed" and "the
    /// wall I aimed at refused it" are different claims and only the second is
    /// evidence.
    pub(crate) fn refusal_code(&self) -> Option<u32> {
        let entry = self.error.as_ref()?.get("InstructionError")?.as_array()?;
        u32::try_from(entry.get(1)?.get("Custom")?.as_u64()?).ok()
    }

    /// Require the exact refusal this hostile claims to prove.
    ///
    /// A hostile that only asserts failure proves that SOMETHING refused, which
    /// is the weakest reading an adversarial probe admits: a typo in the frame,
    /// an unfunded probe wallet, or a wall three layers before the one under
    /// test all satisfy it equally. Three walls stayed invisible behind exactly
    /// that during this wave, and the composed founding's own refusal shared a
    /// code with a hostile that was passing beside it.
    ///
    /// Pinning makes the probe state which wall it believes it hit. A wall that
    /// MOVES then becomes a loud failure naming both codes, instead of a
    /// campaign that stays green while testing something else.
    pub(crate) fn refusing(self, expected: u32) -> crate::Result<Self> {
        match self.refusal_code() {
            Some(code) if code == expected => Ok(self),
            Some(code) => Err(crate::Error::new(format!(
                "{}: refused with 0x{code:04X}, not the pinned 0x{expected:04X}. \
                 The probe still refused, so the campaign would have passed on \
                 the old boolean assertion — read which wall moved before \
                 repinning.",
                self.label
            ))),
            None => Err(crate::Error::new(format!(
                "{}: expected refusal 0x{expected:04X} but the failure carried no \
                 program refusal code: {}",
                self.label,
                self.error
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "no error at all".into())
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct AccountEvidence {
    pub(crate) address: String,
    pub(crate) owner: String,
    pub(crate) lamports: u64,
    pub(crate) executable: bool,
    pub(crate) data_len: usize,
    pub(crate) data_sha256: String,
    pub(crate) account_sha256: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct SuccessorRunEvidence {
    pub(crate) schema: String,
    pub(crate) rpc_url: String,
    pub(crate) ledger: String,
    pub(crate) validator_log: String,
    pub(crate) plan_sha256: String,
    pub(crate) core_upgrade_authority_pubkey: String,
    pub(crate) private_key_persisted: bool,
    /// `"random-per-run"` or `"seeded-deterministic"`.
    ///
    /// `private_key_persisted` answers "did this tool write a key to disk", and
    /// the answer is always no. It does NOT answer "can anyone else produce
    /// these keys", and under `--keypair-seed` the answer to that becomes yes.
    /// The two claims are separate fields because collapsing them would let a
    /// seeded run read as an unreproducible one.
    pub(crate) keypair_derivation: String,
    /// SHA-256 of the seed, absent for a random campaign. The digest, not the
    /// seed: enough to say which seed produced a run's compute-unit numbers,
    /// not enough to sign as it.
    pub(crate) keypair_seed_sha256: Option<String>,
    /// Exact founding action context persisted by the Claims/Custody join.
    /// This is a routing hint for terminal planners; they re-derive and verify
    /// it from the finalized founding aggregate before using it.
    #[serde(rename = "foundingCustodyContext")]
    pub(crate) founding_custody_context: String,
    #[serde(rename = "directSelectedManifestEntryIndex")]
    pub(crate) direct_selected_manifest_entry_index: u16,
    pub(crate) completed: Vec<String>,
    pub(crate) transactions: Vec<TransactionEvidence>,
    pub(crate) accounts: BTreeMap<String, AccountEvidence>,
    pub(crate) remaining_execution_seam: String,
}

#[cfg(test)]
mod refusal_pin_tests {
    use super::TransactionEvidence;
    use serde_json::json;

    fn evidence(error: Option<serde_json::Value>) -> TransactionEvidence {
        TransactionEvidence {
            label: "probe".into(),
            signature: "sig".into(),
            slot: 1,
            transaction_metadata_available: true,
            fee_lamports: Some(5000),
            fee_only_balance_change: Some(true),
            compute_units_consumed: Some(1234),
            error,
            logs: Vec::new(),
        }
    }

    #[test]
    fn reads_the_custom_code_out_of_an_instruction_error() {
        let refused = evidence(Some(json!({"InstructionError": [1, {"Custom": 0x600B}]})));
        assert_eq!(refused.refusal_code(), Some(0x600B));
        let kept = refused
            .refusing(0x600B)
            .expect("the pinned code is the observed one");
        assert_eq!(kept.label, "probe");
    }

    #[test]
    fn a_moved_wall_is_an_error_naming_both_codes() {
        // The whole point: this transaction DID refuse, so the boolean
        // assertion this replaces would have passed it.
        let refused = evidence(Some(json!({"InstructionError": [0, {"Custom": 0x1004}]})));
        let message = refused.refusing(0x1003).unwrap_err().to_string();
        assert!(message.contains("0x1004"), "{message}");
        assert!(message.contains("0x1003"), "{message}");
    }

    #[test]
    fn a_failure_that_is_not_a_program_refusal_is_not_a_refusal() {
        // A hostile that dies on its own malformed signature proves nothing
        // about the wall it aimed at, and must not read as the pinned code.
        for other in [
            json!("AccountNotFound"),
            json!({"InstructionError": [0, "PrivilegeEscalation"]}),
            json!({"InsufficientFundsForRent": {"account_index": 0}}),
        ] {
            let failed = evidence(Some(other.clone()));
            assert_eq!(failed.refusal_code(), None, "{other}");
            assert!(failed.refusing(0x3001).is_err(), "{other}");
        }
        assert_eq!(evidence(None).refusal_code(), None);
    }
}

#[cfg(test)]
mod already_current_closure_tests {
    use super::{AlreadyCurrentClosureV1, CheckedDeploymentDispositionV1, CheckedUpgradeRolePinV1};

    const DIGEST: &str = "11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff";

    /// A canonical AlreadyCurrent plan pin: baseline present and well-formed,
    /// no receipt, no carry-forward transport.
    fn pin() -> CheckedUpgradeRolePinV1 {
        CheckedUpgradeRolePinV1 {
            role: "trading".into(),
            disposition: CheckedDeploymentDispositionV1::AlreadyCurrent,
            program_id: "program".into(),
            programdata_id: "programdata".into(),
            baseline_path: Some("/baseline.json".into()),
            baseline_sha256: Some(DIGEST.into()),
            receipt_path: None,
            receipt_sha256: None,
            dump_path: "/dump.so".into(),
            dump_sha256: DIGEST.into(),
            checked_candidate_elf_path: "/candidate.so".into(),
            checked_candidate_elf_sha256: DIGEST.into(),
            live_elf_sha256: DIGEST.into(),
            deployment_slot: 490_000_000,
            programdata_account_sha256: DIGEST.into(),
            semantic_release_id: DIGEST.into(),
            artifact_release_body_hex: None,
            artifact_release_id: None,
            carried_programdata_base64: None,
        }
    }

    #[test]
    fn a_canonical_already_current_row_is_admitted() {
        let pin = pin();
        assert!(pin.already_current_closure().holds());
        assert!(pin.carries_no_transport_fields());
        assert!(pin.already_current_closure().audit().is_ok());
    }

    #[test]
    fn a_claimed_upgrade_receipt_refuses_and_says_which_field() {
        // Red-proof by mutation, in both shapes a receipt can be claimed. The
        // rule used to live at five sites; a row carrying a receipt must refuse
        // no matter which of them reaches it first.
        for mutate in [
            |pin: &mut CheckedUpgradeRolePinV1| pin.receipt_path = Some("/receipt.json".into()),
            |pin: &mut CheckedUpgradeRolePinV1| pin.receipt_sha256 = Some(DIGEST.into()),
        ] {
            let mut pin = pin();
            mutate(&mut pin);
            let message = pin
                .already_current_closure()
                .audit()
                .expect_err("an AlreadyCurrent row never claims an Upgrade receipt")
                .to_string();
            assert!(message.contains("trading"), "{message}");
            assert!(message.contains("Upgrade receipt is claimed"), "{message}");
        }
    }

    #[test]
    fn an_absent_or_malformed_baseline_refuses_at_the_baseline() {
        // The stricter of the two standards the sites used to carry: one
        // checked only presence, so it admitted the malformed digest and the
        // empty path that the other refused.
        for mutate in [
            |pin: &mut CheckedUpgradeRolePinV1| pin.baseline_path = None,
            |pin: &mut CheckedUpgradeRolePinV1| pin.baseline_path = Some(String::new()),
            |pin: &mut CheckedUpgradeRolePinV1| pin.baseline_sha256 = None,
            |pin: &mut CheckedUpgradeRolePinV1| pin.baseline_sha256 = Some("not-a-digest".into()),
        ] {
            let mut pin = pin();
            mutate(&mut pin);
            let message = pin
                .already_current_closure()
                .audit()
                .expect_err("an AlreadyCurrent row carries the baseline its equality was judged at")
                .to_string();
            assert!(message.contains("trading"), "{message}");
            assert!(message.contains("pinned baseline"), "{message}");
        }
    }

    #[test]
    fn a_carry_forward_transport_field_is_refused_by_its_own_conjunct() {
        // Not part of the shared closure -- it is the row kind's own rule, and
        // the two sites that judge it now share one copy of that too.
        for mutate in [
            |pin: &mut CheckedUpgradeRolePinV1| pin.artifact_release_body_hex = Some("00".into()),
            |pin: &mut CheckedUpgradeRolePinV1| pin.artifact_release_id = Some(DIGEST.into()),
            |pin: &mut CheckedUpgradeRolePinV1| {
                pin.carried_programdata_base64 = Some("AA==".into())
            },
        ] {
            let mut pin = pin();
            mutate(&mut pin);
            assert!(!pin.carries_no_transport_fields());
            // The shared closure is untouched by transport fields, which is
            // exactly why it is a separate conjunct.
            assert!(pin.already_current_closure().holds());
        }
    }

    #[test]
    fn the_refusal_always_names_the_role_it_is_about() {
        // Three of the five messages this replaces never named the failing
        // field, and one never named the role either.
        let refusal = AlreadyCurrentClosureV1 {
            role: "resolution",
            receipt_claimed: true,
            baseline_present: true,
        }
        .audit()
        .expect_err("a claimed receipt refuses")
        .to_string();
        assert!(
            refusal.starts_with("AlreadyCurrent role resolution:"),
            "{refusal}"
        );
    }
}
