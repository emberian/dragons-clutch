//! Optional raw same-bank corpus export for canonical CLI/WASM transport parity.
//! No deployment manifest is asserted by this fixture export.

use super::*;
use solana_program_test::ProgramTestContext;
use std::fmt::Write as _;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Write once before submission; the actual native producer consumes these bytes.
/// No fixture-predicted poststate or executable route is emitted here.
pub(super) async fn export_before_submit(
    context: &mut ProgramTestContext,
    campaign: &CampaignV1,
    case: &HostCase,
    fee_payer: Pubkey,
) {
    let Some(directory) = env::var_os("DCLUTCH_GENERAL_NATIVE_CORPUS_DIR") else {
        return;
    };
    let directory = PathBuf::from(directory).join(format!(
        "{}-n{}",
        dclutch_operator::general_successor::action_name_v1(case.action),
        campaign.outcome_count,
    ));
    fs::create_dir_all(directory.parent().expect("export parent"))
        .expect("export parent directory");
    fs::create_dir(&directory).expect("raw General corpus export must be new");
    let clock = context
        .banks_client
        .get_sysvar::<solana_program::clock::Clock>()
        .await
        .expect("observed bank Clock");
    let hot = &case.built.bundle.hot_instruction;
    let activation = dclutch_registry::ActivatedExecutionReleaseSetV1::decode(
        &campaign.releases.activation_data,
    )
    .expect("same-bank activation");
    let trading_release = activation
        .role(dclutch_registry::release_set::ExecutionRoleV1::Trading)
        .release();
    let strategy_count =
        dclutch_market::execution_strategy::admitted_v3::ADMITTED_STRATEGY_EVIDENCE_COUNT_V3
            + case.built.admitted_authorities.entries.len();
    let mut metadata = String::new();
    for (name, value) in [
        (
            "format",
            "dclutch-general-raw-program-test-corpus-v1".to_owned(),
        ),
        (
            "manifestStatus",
            "unmanifested-program-test-fixture".to_owned(),
        ),
        (
            "action",
            dclutch_operator::general_successor::action_name_v1(case.action).to_owned(),
        ),
        ("slot", clock.slot.to_string()),
        ("unixTimestamp", clock.unix_timestamp.to_string()),
        ("recentBlockhash", context.last_blockhash.to_string()),
        ("payer", fee_payer.to_string()),
        ("lookupTable", waist::LOOKUP_TABLE.to_string()),
        ("tradingProgram", hot.program_id.to_string()),
        ("releaseSet", hex(&campaign.releases.release_set)),
        (
            "generation",
            campaign.release.publication.generation.to_string(),
        ),
        (
            "programSet",
            hex(&campaign.release.publication.program_set_id),
        ),
        ("config", hex(&campaign.release.publication.config_id)),
        (
            "tradingArtifactRelease",
            hex(&digest(&trading_release.to_bytes())),
        ),
        (
            "generalArtifactRelease",
            hex(&case.built.evidence.artifact_release.digest),
        ),
        ("fixedAccountCount", HOT_FIXED_ACCOUNT_COUNT_V3.to_string()),
        ("strategyAccountCount", strategy_count.to_string()),
        (
            "outputPageCount",
            usize::from(case.built.admitted_authorities.output_page.is_some()).to_string(),
        ),
    ] {
        writeln!(metadata, "{name}\t{value}").expect("metadata string");
    }
    fs::write(directory.join("metadata.tsv"), metadata).expect("raw export metadata");
    let mut metas = String::new();
    for meta in &hot.accounts {
        writeln!(
            metas,
            "{}\t{}\t{}",
            meta.pubkey, meta.is_signer, meta.is_writable
        )
        .expect("meta string");
    }
    fs::write(directory.join("hot-accounts.tsv"), metas).expect("raw account metas");
    fs::write(directory.join("hot-instruction.bin"), &hot.data).expect("actual Hot bytes");
    let mut addresses = Vec::new();
    for key in hot
        .accounts
        .iter()
        .map(|meta| meta.pubkey)
        .chain([waist::LOOKUP_TABLE])
    {
        if !addresses.contains(&key) {
            addresses.push(key);
        }
    }
    let mut accounts = String::new();
    for key in addresses {
        match context
            .banks_client
            .get_account(key)
            .await
            .expect("raw bank observation")
        {
            Some(account) => {
                fs::write(directory.join(format!("{key}.bin")), &account.data)
                    .expect("raw observed account bytes");
                writeln!(
                    accounts,
                    "{key}\tpresent\t{}\t{}\t{}",
                    account.owner, account.lamports, account.executable
                )
                .expect("account string");
            }
            None => {
                writeln!(accounts, "{key}\tabsent\t{}\t0\tfalse", system_program::ID)
                    .expect("absent account string");
            }
        }
    }
    fs::write(directory.join("accounts.tsv"), accounts).expect("raw account observations");
}
