//! The sponsored-push provider release, READ OFF THE CHAIN at plan time.
//!
//! # What this exists to prevent
//!
//! A market pins its provider release AT FOUNDING, and
//! `sponsored_push_v1::authenticate_provider_program_pin` pins Pyth's Receiver
//! and push oracle by exact `(ProgramData, deployment_slot, upgrade_authority)`
//! equality — because rehashing 1.64 MiB on every capture is not a transaction
//! path, so Loader-v3's monotonic slot is the proxy and any deployment movement
//! fails closed as `ResolutionError::ReleaseSuperseded`.
//!
//! `dclutch_pyth_svm::devnet_sponsored_sol_usd_release_v1` is a CONSTANT, read
//! with finalized commitment on 2026-08-28. Pyth redeployed their devnet
//! Receiver at slot 491,006,444 and changed their Receiver `Config` body some
//! time after that reading. Cohort-13 was founded 4.36 days after the redeploy
//! and cohort-14 5.64 days after it, both against the stale constant, and both
//! markets are permanently uncapturable: cohort-14's capture refused `0x8014`
//! after 101,787 CU, and cohort-13's would have refused at any second of its
//! window.
//!
//! So the chain-owned half of the release is no longer typed. It is observed,
//! exactly the way `general_devnet_market` observes the accelerator's
//! deployment: the slot and the upgrade authority are hostile-decoded out of a
//! finalized ProgramData image by `ProgramDataV3View`, the ELF digest is the
//! SHA-256 of the observed tail, and the Receiver `Config` digest is the
//! SHA-256 of the exact account the program itself hashes.
//!
//! # The partition, which is the whole design
//!
//! Nine facts are CHAIN-OWNED and are observed. Everything else is DECLARED and
//! comes from the constant, because it says *which* accounts this release is
//! about rather than what they currently contain:
//!
//! | declared | observed |
//! | --- | --- |
//! | cluster, both program ids, Config address, price account, feed id | both ProgramData deployment slots |
//! | codec, adapter, family and transport identities | both ProgramData ELF digests |
//! | shard, feed-account bump, activation time | both Loader-v3 upgrade authorities |
//! | both ProgramData ADDRESSES (a moved one is a different lineage) | the Receiver `Config` body digest |
//!
//! A moved ProgramData *address* is refused rather than absorbed: Loader-v3
//! derives it from the program id, so an address that moved means the program
//! id did, and that is a different provider — not a newer release of this one.
//!
//! # Supersession, and what the chain's rule actually is
//!
//! **There is no in-place supersession of a provider release, and the chain
//! does not admit forward slot movement.** `authenticate_provider_program_pin`
//! is exact equality in both directions, and `ArtifactReleaseV1::slot_pin_refusal`
//! (`crates/dclutch-registry-contract/src/artifact.rs:272`) turns a strictly
//! later slot into `ReleaseSupersededByUpgrade` rather than into an admission.
//! The one "forward movement admits" rule in this tree
//! (`campaign::ObservedRoleV1::pin_conflicts_allowing_forward_slot`) is scoped
//! to the Registry role under an infrastructure-succession plan and has nothing
//! to do with a provider.
//!
//! A sponsored release supersedes its predecessor by being a DIFFERENT RECORD.
//! The 592 bytes are the body, the body's SHA-256 is its identity, and that
//! identity flows into `ProviderReleaseV1`, `SourceSpecV1`, `SourceMaterialV2`
//! and finally into the Market PDA's own seeds — so a re-minted release founds a
//! NEW market and can never repair an existing one.
//!
//! The forward-only check below is therefore a PLAN-TIME sanity gate of this
//! module's own, not a restatement of a chain rule: Loader-v3's slot is
//! monotonic, so an observation below the declared one is not a stale reading,
//! it is a reading no single chain could have produced, and minting a release on
//! it would put a number in a record that nothing can ever satisfy.
//!
//! # Why it is one snapshot
//!
//! All five accounts are read in a single `getMultipleAccounts` at finalized
//! commitment above a floor slot. Five separate reads could observe a redeploy
//! in progress and mint a release no single chain state ever held.

use dclutch_pyth_svm::{
    PythSponsoredPushReleaseV1, PythSponsoredPushReleaseV1Input, RECEIVER_CONFIG_V2_LEN,
    ReceiverConfigV2View, devnet_sponsored_sol_usd_release_v1,
};
use dclutch_registry_svm::{LOADER_V3_PROGRAM_BYTES, ProgramDataV3View, ProgramV3View};
use sha2::{Digest as _, Sha256};
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::bpf_loader_upgradeable;

use crate::{Error, Result, rpc::Rpc};

fn refusal(code: &str, reason: impl AsRef<str>) -> Error {
    Error::new(format!("REFUSED: [{code}] {}", reason.as_ref()))
}

/// Exactly the facts the chain owns about one sponsored-push provider release.
///
/// Every field here is read from a finalized account. Nothing in it is typed,
/// and nothing outside it is observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProviderChainFactsV1 {
    /// The Receiver's ProgramData address, derived by Loader-v3 from the
    /// program id and READ BACK off the Program account's own body.
    pub(crate) receiver_programdata: [u8; 32],
    /// SHA-256 of the Receiver's observed ProgramData tail.
    pub(crate) receiver_abi_id: [u8; 32],
    /// The Receiver's observed Loader-v3 upgrade authority.
    pub(crate) receiver_upgrade_authority: Option<[u8; 32]>,
    /// The Receiver's observed deployment slot.
    pub(crate) receiver_deployment_slot: u64,
    /// SHA-256 of the exact Receiver `Config` account body the program hashes.
    pub(crate) receiver_config_digest: [u8; 32],
    /// The push oracle's ProgramData address, read back the same way.
    pub(crate) push_oracle_programdata: [u8; 32],
    /// SHA-256 of the push oracle's observed ProgramData tail.
    pub(crate) push_oracle_abi_id: [u8; 32],
    /// The push oracle's observed Loader-v3 upgrade authority.
    pub(crate) push_oracle_upgrade_authority: Option<[u8; 32]>,
    /// The push oracle's observed deployment slot.
    pub(crate) push_oracle_deployment_slot: u64,
}

/// One chain-owned fact that moved between the declared release and the chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MovedProviderFactV1 {
    /// The fact's name, as the release's own field is spelled.
    pub(crate) field: &'static str,
    /// What the declared constant says.
    pub(crate) declared: String,
    /// What the chain says.
    pub(crate) observed: String,
}

/// A sponsored-push release re-minted against a finalized observation.
#[derive(Clone, Debug)]
pub(crate) struct ObservedSponsoredReleaseV1 {
    /// The release a market founded on this observation pins.
    pub(crate) release: PythSponsoredPushReleaseV1,
    /// The release the tree's constant declares, for the comparison.
    pub(crate) declared: PythSponsoredPushReleaseV1,
    /// Which chain-owned facts moved. Empty means the constant is current.
    pub(crate) moved: Vec<MovedProviderFactV1>,
    /// The finalized slot the five accounts were read at.
    pub(crate) finalized_slot: u64,
}

impl ObservedSponsoredReleaseV1 {
    /// A one-line-per-fact report of declared against observed.
    pub(crate) fn report(&self) -> String {
        if self.moved.is_empty() {
            return format!(
                "the declared sponsored release is current at finalized slot {}",
                self.finalized_slot
            );
        }
        let mut out = format!(
            "the sponsored release is RE-MINTED against finalized slot {}; {} chain-owned fact(s) \
             moved:",
            self.finalized_slot,
            self.moved.len()
        );
        for fact in &self.moved {
            out.push_str(&format!(
                "\n  {}: declared {} -> observed {}",
                fact.field, fact.declared, fact.observed
            ));
        }
        out
    }
}

fn hex32(value: &[u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in value {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Re-mint one sponsored-push release from a declared constant and an
/// observation.
///
/// Pure: every refusal below is decidable without a network. The chain-owned
/// fields are replaced and every declared field is carried through unchanged,
/// so the produced release differs from the declared one in exactly the nine
/// facts this module's table names and in no other byte.
pub(crate) fn remint_from_observation_v1(
    declared: PythSponsoredPushReleaseV1,
    facts: ProviderChainFactsV1,
) -> Result<(PythSponsoredPushReleaseV1, Vec<MovedProviderFactV1>)> {
    // A moved ProgramData ADDRESS is a moved program id, which is a different
    // provider rather than a newer release of this one.
    if facts.receiver_programdata != declared.receiver_programdata() {
        return Err(refusal(
            "provider-release/receiver-programdata-moved",
            format!(
                "the Receiver Program account names ProgramData {} and the declared release pins \
                 {}. Loader-v3 derives that address from the program id, so a moved address is a \
                 different program, not a newer release of this one",
                hex32(&facts.receiver_programdata),
                hex32(&declared.receiver_programdata())
            ),
        ));
    }
    if facts.push_oracle_programdata != declared.push_oracle_programdata() {
        return Err(refusal(
            "provider-release/push-oracle-programdata-moved",
            format!(
                "the push-oracle Program account names ProgramData {} and the declared release \
                 pins {}",
                hex32(&facts.push_oracle_programdata),
                hex32(&declared.push_oracle_programdata())
            ),
        ));
    }
    // Loader-v3's deployment slot is monotonic. Forward movement is a
    // supersession and admits; backward movement is a chain that cannot have
    // produced both numbers.
    if facts.receiver_deployment_slot < declared.receiver_deployment_slot() {
        return Err(refusal(
            "provider-release/receiver-slot-rollback",
            format!(
                "the Receiver's observed deployment slot {} is BELOW the declared {}. Loader-v3's \
                 slot is monotonic, so supersession admits forward movement only",
                facts.receiver_deployment_slot,
                declared.receiver_deployment_slot()
            ),
        ));
    }
    if facts.push_oracle_deployment_slot < declared.push_oracle_deployment_slot() {
        return Err(refusal(
            "provider-release/push-oracle-slot-rollback",
            format!(
                "the push oracle's observed deployment slot {} is BELOW the declared {}",
                facts.push_oracle_deployment_slot,
                declared.push_oracle_deployment_slot()
            ),
        ));
    }
    // The release body has no room for "immutable": every authority field is a
    // required nonzero identity. A provider that dropped its upgrade authority
    // is a different claim about the world and refuses here rather than being
    // encoded as a zero.
    let receiver_upgrade_authority = facts.receiver_upgrade_authority.ok_or_else(|| {
        refusal(
            "provider-release/receiver-immutable",
            "the Receiver deployment carries no Loader-v3 upgrade authority. A sponsored-push \
             release states an exact current authority and has no encoding for an immutable \
             provider",
        )
    })?;
    let push_oracle_upgrade_authority = facts.push_oracle_upgrade_authority.ok_or_else(|| {
        refusal(
            "provider-release/push-oracle-immutable",
            "the push-oracle deployment carries no Loader-v3 upgrade authority",
        )
    })?;

    let release = PythSponsoredPushReleaseV1::new(PythSponsoredPushReleaseV1Input {
        // Declared: which cluster, which accounts, which codec, which policy.
        cluster_id: declared.cluster_id(),
        receiver_program: declared.receiver_program(),
        push_oracle_program: declared.push_oracle_program(),
        receiver_config: declared.receiver_config(),
        price_account: declared.price_account(),
        feed_id: declared.feed_id(),
        price_update_codec_id: declared.price_update_codec_id(),
        adapter_id: declared.adapter_id(),
        provider_family_id: declared.provider_family_id(),
        transport_profile_id: declared.transport_profile_id(),
        shard: declared.shard(),
        feed_account_bump: declared.feed_account_bump(),
        activation_time: declared.activation_time(),
        // Observed: what those accounts currently hold.
        receiver_programdata: facts.receiver_programdata,
        receiver_abi_id: facts.receiver_abi_id,
        receiver_upgrade_authority,
        receiver_deployment_slot: facts.receiver_deployment_slot,
        receiver_config_digest: facts.receiver_config_digest,
        push_oracle_programdata: facts.push_oracle_programdata,
        push_oracle_abi_id: facts.push_oracle_abi_id,
        push_oracle_upgrade_authority,
        push_oracle_deployment_slot: facts.push_oracle_deployment_slot,
    })
    .map_err(|error| {
        refusal(
            "provider-release/invalid",
            format!("the re-minted sponsored release is not canonical: {error:?}"),
        )
    })?;

    let mut moved = Vec::new();
    let mut note = |field: &'static str, declared: String, observed: String| {
        if declared != observed {
            moved.push(MovedProviderFactV1 {
                field,
                declared,
                observed,
            });
        }
    };
    note(
        "receiver_deployment_slot",
        declared.receiver_deployment_slot().to_string(),
        facts.receiver_deployment_slot.to_string(),
    );
    note(
        "receiver_abi_id",
        hex32(&declared.receiver_abi_id()),
        hex32(&facts.receiver_abi_id),
    );
    note(
        "receiver_upgrade_authority",
        hex32(&declared.receiver_upgrade_authority()),
        hex32(&receiver_upgrade_authority),
    );
    note(
        "receiver_config_digest",
        hex32(&declared.receiver_config_digest()),
        hex32(&facts.receiver_config_digest),
    );
    note(
        "push_oracle_deployment_slot",
        declared.push_oracle_deployment_slot().to_string(),
        facts.push_oracle_deployment_slot.to_string(),
    );
    note(
        "push_oracle_abi_id",
        hex32(&declared.push_oracle_abi_id()),
        hex32(&facts.push_oracle_abi_id),
    );
    note(
        "push_oracle_upgrade_authority",
        hex32(&declared.push_oracle_upgrade_authority()),
        hex32(&push_oracle_upgrade_authority),
    );
    Ok((release, moved))
}

/// Read the nine chain-owned facts in one finalized five-account snapshot.
pub(crate) fn observe_provider_chain_facts_v1(
    rpc: &mut Rpc,
    declared: PythSponsoredPushReleaseV1,
    floor_slot: u64,
) -> Result<(ProviderChainFactsV1, u64)> {
    let receiver = Pubkey::new_from_array(declared.receiver_program());
    let push_oracle = Pubkey::new_from_array(declared.push_oracle_program());
    let receiver_programdata =
        Pubkey::find_program_address(&[receiver.as_ref()], &bpf_loader_upgradeable::ID).0;
    let push_oracle_programdata =
        Pubkey::find_program_address(&[push_oracle.as_ref()], &bpf_loader_upgradeable::ID).0;
    let config = Pubkey::new_from_array(declared.receiver_config());
    let addresses = [
        receiver,
        receiver_programdata,
        push_oracle,
        push_oracle_programdata,
        config,
    ];
    let (finalized_slot, accounts) = rpc.finalized_accounts(&addresses, floor_slot)?;
    if finalized_slot < floor_slot || accounts.len() != addresses.len() {
        return Err(refusal(
            "provider-release/snapshot",
            format!(
                "the finalized provider snapshot was below its floor {floor_slot} or changed its \
                 exact five-account width"
            ),
        ));
    }
    let mut accounts = accounts.into_iter();
    let mut next = |address: Pubkey, label: &str| -> Result<crate::rpc::RpcAccount> {
        accounts.next().flatten().ok_or_else(|| {
            refusal(
                "provider-release/missing",
                format!("no {label} account at {address}"),
            )
        })
    };
    let receiver_account = next(receiver, "Receiver Program")?;
    let receiver_programdata_account = next(receiver_programdata, "Receiver ProgramData")?;
    let push_account = next(push_oracle, "push-oracle Program")?;
    let push_programdata_account = next(push_oracle_programdata, "push-oracle ProgramData")?;
    let config_account = next(config, "Receiver Config")?;

    let receiver_facts = loader_v3_facts_v1(
        "receiver",
        receiver,
        receiver_programdata,
        &receiver_account,
        &receiver_programdata_account,
    )?;
    let push_facts = loader_v3_facts_v1(
        "push-oracle",
        push_oracle,
        push_oracle_programdata,
        &push_account,
        &push_programdata_account,
    )?;

    // The config digest is over the EXACT account body the program hashes, so
    // its shape is authenticated first: a body that is not a canonical
    // Receiver V2 Config would otherwise be hashed into a release the chain
    // then refuses with a code that names neither the width nor the parse.
    if config_account.owner != receiver
        || config_account.executable
        || config_account.data.len() != RECEIVER_CONFIG_V2_LEN
    {
        return Err(refusal(
            "provider-release/config-shape",
            format!(
                "the Receiver Config at {config} is not a {RECEIVER_CONFIG_V2_LEN}-byte \
                 non-executable account owned by {receiver}: owner {}, executable {}, bytes {}",
                config_account.owner,
                config_account.executable,
                config_account.data.len()
            ),
        ));
    }
    ReceiverConfigV2View::parse(&config_account.data).map_err(|error| {
        refusal(
            "provider-release/config-body",
            format!("the Receiver Config at {config} is not canonical: {error:?}"),
        )
    })?;
    let receiver_config_digest: [u8; 32] = Sha256::digest(&config_account.data).into();

    Ok((
        ProviderChainFactsV1 {
            receiver_programdata: receiver_facts.programdata,
            receiver_abi_id: receiver_facts.abi_id,
            receiver_upgrade_authority: receiver_facts.upgrade_authority,
            receiver_deployment_slot: receiver_facts.deployment_slot,
            receiver_config_digest,
            push_oracle_programdata: push_facts.programdata,
            push_oracle_abi_id: push_facts.abi_id,
            push_oracle_upgrade_authority: push_facts.upgrade_authority,
            push_oracle_deployment_slot: push_facts.deployment_slot,
        },
        finalized_slot,
    ))
}

struct LoaderV3FactsV1 {
    programdata: [u8; 32],
    abi_id: [u8; 32],
    upgrade_authority: Option<[u8; 32]>,
    deployment_slot: u64,
}

fn loader_v3_facts_v1(
    label: &str,
    program: Pubkey,
    programdata: Pubkey,
    program_account: &crate::rpc::RpcAccount,
    programdata_account: &crate::rpc::RpcAccount,
) -> Result<LoaderV3FactsV1> {
    let program_view = ProgramV3View::parse(&program_account.data).map_err(|error| {
        refusal(
            "provider-release/loader",
            format!("{label} Program account: {error:?}"),
        )
    })?;
    let programdata_view =
        ProgramDataV3View::parse(&programdata_account.data).map_err(|error| {
            refusal(
                "provider-release/loader",
                format!("{label} ProgramData account: {error:?}"),
            )
        })?;
    if program_account.owner != bpf_loader_upgradeable::ID
        || !program_account.executable
        || program_account.data.len() != LOADER_V3_PROGRAM_BYTES
        || programdata_account.owner != bpf_loader_upgradeable::ID
        || programdata_account.executable
        || program_view.programdata() != programdata.to_bytes()
    {
        return Err(refusal(
            "provider-release/loader",
            format!(
                "{program} is not a Loader V3 deployment linked to {programdata}: owner {}, \
                 executable {}, program bytes {}",
                program_account.owner,
                program_account.executable,
                program_account.data.len()
            ),
        ));
    }
    Ok(LoaderV3FactsV1 {
        programdata: program_view.programdata(),
        abi_id: Sha256::digest(programdata_view.elf()).into(),
        upgrade_authority: programdata_view.upgrade_authority(),
        deployment_slot: programdata_view.deployment_slot(),
    })
}

/// The devnet sponsored SOL/USD release a market founded NOW would pin.
///
/// This is the one authority for that question. Three consumers used to answer
/// it by reading the constant — the market compiler, the capture's release
/// authenticator, and the terminal input producer — and all three would have
/// agreed with each other and with nothing on the chain.
pub(crate) fn observed_devnet_sponsored_release_v1(
    rpc: &mut Rpc,
    floor_slot: u64,
) -> Result<ObservedSponsoredReleaseV1> {
    let declared = devnet_sponsored_sol_usd_release_v1().map_err(|error| {
        refusal(
            "provider-release/declared",
            format!("devnet sponsored Pyth release row: {error:?}"),
        )
    })?;
    let (facts, finalized_slot) = observe_provider_chain_facts_v1(rpc, declared, floor_slot)?;
    let (release, moved) = remint_from_observation_v1(declared, facts)?;
    Ok(ObservedSponsoredReleaseV1 {
        release,
        declared,
        moved,
        finalized_slot,
    })
}

/// Authenticate the release a Market ALREADY PINS against the chain as it is
/// now.
///
/// The capture path used to compare the Market's record to
/// `devnet_sponsored_sol_usd_release_v1` byte for byte. That agrees with the
/// constant and with nothing else: when Pyth moved, the comparison stayed green
/// and the chain refused `0x8014 ReleaseSuperseded` after 101,787 CU with no
/// word for which conjunct failed. Comparing against a fresh observation asks
/// the question the program will ask, before a lamport is spent, and names the
/// facts that moved.
pub(crate) fn authenticate_market_release_against_chain_v1(
    rpc: &mut Rpc,
    market_release: PythSponsoredPushReleaseV1,
) -> Result<()> {
    let observed = observed_devnet_sponsored_release_v1(rpc, 0)?;
    if market_release.to_bytes() == observed.release.to_bytes() {
        return Ok(());
    }
    let (_, moved) = remint_from_observation_v1(
        market_release,
        ProviderChainFactsV1 {
            receiver_programdata: observed.release.receiver_programdata(),
            receiver_abi_id: observed.release.receiver_abi_id(),
            receiver_upgrade_authority: Some(observed.release.receiver_upgrade_authority()),
            receiver_deployment_slot: observed.release.receiver_deployment_slot(),
            receiver_config_digest: observed.release.receiver_config_digest(),
            push_oracle_programdata: observed.release.push_oracle_programdata(),
            push_oracle_abi_id: observed.release.push_oracle_abi_id(),
            push_oracle_upgrade_authority: Some(observed.release.push_oracle_upgrade_authority()),
            push_oracle_deployment_slot: observed.release.push_oracle_deployment_slot(),
        },
    )
    .unwrap_or((market_release, Vec::new()));
    let mut detail = format!(
        "the release this Market pins is not the one the chain admits at finalized slot {}. A          market pins its provider release AT FOUNDING, so this market cannot be repaired; a          market founded on the current observation can.",
        observed.finalized_slot
    );
    if moved.is_empty() {
        detail.push_str(
            "\n  the moved facts could not be enumerated -- the pinned body differs outside the \
             chain-owned set, which is a different release entirely",
        );
    }
    for fact in &moved {
        detail.push_str(&format!(
            "\n  {}: pinned {} -> chain {}",
            fact.field, fact.declared, fact.observed
        ));
    }
    Err(refusal("provider-release/market-pin-superseded", detail))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn declared() -> PythSponsoredPushReleaseV1 {
        devnet_sponsored_sol_usd_release_v1().expect("declared release")
    }

    fn current_facts() -> ProviderChainFactsV1 {
        let declared = declared();
        ProviderChainFactsV1 {
            receiver_programdata: declared.receiver_programdata(),
            receiver_abi_id: declared.receiver_abi_id(),
            receiver_upgrade_authority: Some(declared.receiver_upgrade_authority()),
            receiver_deployment_slot: declared.receiver_deployment_slot(),
            receiver_config_digest: declared.receiver_config_digest(),
            push_oracle_programdata: declared.push_oracle_programdata(),
            push_oracle_abi_id: declared.push_oracle_abi_id(),
            push_oracle_upgrade_authority: Some(declared.push_oracle_upgrade_authority()),
            push_oracle_deployment_slot: declared.push_oracle_deployment_slot(),
        }
    }

    /// An observation that agrees with the constant re-mints it BYTE FOR BYTE
    /// and reports nothing moved. Without this the module could "fix" a market
    /// by quietly changing a declared fact.
    #[test]
    fn an_observation_that_agrees_reproduces_the_declared_release_exactly() {
        let declared = declared();
        let (release, moved) =
            remint_from_observation_v1(declared, current_facts()).expect("re-mint");
        assert_eq!(release.to_bytes(), declared.to_bytes());
        assert!(moved.is_empty(), "{moved:?}");
    }

    /// THE PARTITION, asserted rather than the fix. Every chain-owned fact is
    /// moved at once and the re-minted body is compared to the declared one
    /// byte by byte: the differing offsets must be exactly the nine facts'
    /// own offsets, so a tenth field absorbed into the observation goes red
    /// naming an offset instead of quietly riding along.
    #[test]
    fn exactly_the_chain_owned_facts_differ_from_the_declared_release() {
        let declared = declared();
        let mut facts = current_facts();
        facts.receiver_abi_id = [0x11; 32];
        facts.receiver_upgrade_authority = Some([0x22; 32]);
        facts.receiver_deployment_slot = declared.receiver_deployment_slot() + 7;
        facts.receiver_config_digest = [0x33; 32];
        facts.push_oracle_abi_id = [0x44; 32];
        facts.push_oracle_upgrade_authority = Some([0x55; 32]);
        facts.push_oracle_deployment_slot = declared.push_oracle_deployment_slot() + 9;
        let (release, moved) = remint_from_observation_v1(declared, facts).expect("re-mint");

        let before = declared.to_bytes();
        let mut restored = release.to_bytes();
        // Write the DECLARED value back over each chain-owned extent. If the
        // re-mint touched one byte outside them, the restored body differs from
        // the declared one and this is red at that byte — which a subset
        // comparison of "which offsets differ" could not catch, because a moved
        // field can coincidentally keep a byte.
        for (offset, source) in observed_field_extents_v1(&before, declared) {
            restored[offset..offset + source.len()].copy_from_slice(&source);
        }
        assert_eq!(
            restored, before,
            "the re-mint moved bytes outside the chain-owned facts"
        );
        // And each extent really did move, so the test cannot pass by the
        // re-mint having done nothing.
        for (offset, source) in observed_field_extents_v1(&before, declared) {
            let after = release.to_bytes();
            assert_ne!(
                &after[offset..offset + source.len()],
                source.as_slice(),
                "the chain-owned fact at offset {offset} did not move"
            );
        }
        assert_eq!(moved.len(), 7, "{moved:?}");
    }

    /// Locate each observed field in the encoded body by searching for the
    /// declared value, so the test states WHERE the facts live without
    /// re-typing offsets the codec owns.
    fn observed_field_extents_v1(
        body: &[u8],
        declared: PythSponsoredPushReleaseV1,
    ) -> Vec<(usize, Vec<u8>)> {
        let locate = |needle: Vec<u8>| -> (usize, Vec<u8>) {
            let first = body
                .windows(needle.len())
                .position(|window| window == needle.as_slice())
                .expect("a declared fact appears in its own encoding");
            let last = body
                .windows(needle.len())
                .rposition(|window| window == needle.as_slice())
                .expect("a declared fact appears in its own encoding");
            assert_eq!(
                first, last,
                "a declared fact appears twice in the encoding; the extent is ambiguous"
            );
            (first, needle)
        };
        vec![
            locate(declared.receiver_abi_id().to_vec()),
            locate(declared.receiver_upgrade_authority().to_vec()),
            locate(declared.receiver_config_digest().to_vec()),
            locate(declared.push_oracle_abi_id().to_vec()),
            locate(declared.push_oracle_upgrade_authority().to_vec()),
            locate(declared.receiver_deployment_slot().to_le_bytes().to_vec()),
            locate(
                declared
                    .push_oracle_deployment_slot()
                    .to_le_bytes()
                    .to_vec(),
            ),
        ]
    }

    /// Forward movement admits; backward movement is refused BY NAME, because
    /// Loader-v3's slot is monotonic and no chain produced both numbers.
    #[test]
    fn a_backward_deployment_slot_refuses_and_a_forward_one_admits() {
        let declared = declared();
        let mut back = current_facts();
        back.receiver_deployment_slot = declared.receiver_deployment_slot() - 1;
        let refused = remint_from_observation_v1(declared, back).expect_err("rollback");
        assert!(
            refused
                .to_string()
                .contains("provider-release/receiver-slot-rollback"),
            "{refused}"
        );

        let mut back = current_facts();
        back.push_oracle_deployment_slot = declared.push_oracle_deployment_slot() - 1;
        let refused = remint_from_observation_v1(declared, back).expect_err("rollback");
        assert!(
            refused
                .to_string()
                .contains("provider-release/push-oracle-slot-rollback"),
            "{refused}"
        );

        let mut forward = current_facts();
        forward.receiver_deployment_slot = declared.receiver_deployment_slot() + 1;
        let (release, moved) = remint_from_observation_v1(declared, forward).expect("forward");
        assert_eq!(
            release.receiver_deployment_slot(),
            declared.receiver_deployment_slot() + 1
        );
        assert_eq!(moved.len(), 1);
        assert_eq!(moved[0].field, "receiver_deployment_slot");
    }

    /// A moved ProgramData address is a moved program id, and is refused
    /// rather than absorbed into a "newer release".
    #[test]
    fn a_moved_programdata_address_refuses_for_each_program() {
        let declared = declared();
        let mut moved = current_facts();
        moved.receiver_programdata = [0x99; 32];
        let refused = remint_from_observation_v1(declared, moved).expect_err("moved receiver");
        assert!(
            refused
                .to_string()
                .contains("provider-release/receiver-programdata-moved"),
            "{refused}"
        );

        let mut moved = current_facts();
        moved.push_oracle_programdata = [0x99; 32];
        let refused = remint_from_observation_v1(declared, moved).expect_err("moved push oracle");
        assert!(
            refused
                .to_string()
                .contains("provider-release/push-oracle-programdata-moved"),
            "{refused}"
        );
    }

    /// A provider that dropped its upgrade authority is a different claim, and
    /// the release body has no encoding for it.
    #[test]
    fn an_immutable_provider_refuses_rather_than_encoding_a_zero_authority() {
        let declared = declared();
        let mut immutable = current_facts();
        immutable.receiver_upgrade_authority = None;
        let refused = remint_from_observation_v1(declared, immutable).expect_err("immutable");
        assert!(
            refused
                .to_string()
                .contains("provider-release/receiver-immutable"),
            "{refused}"
        );

        let mut immutable = current_facts();
        immutable.push_oracle_upgrade_authority = None;
        let refused = remint_from_observation_v1(declared, immutable).expect_err("immutable");
        assert!(
            refused
                .to_string()
                .contains("provider-release/push-oracle-immutable"),
            "{refused}"
        );
    }
}
