//! Non-production executable evidence for the synthetic-local Pyth route.
//!
//! This campaign loads the locally compiled dClutch SBF ELF and the two
//! provenance-pinned upgraded provider ELFs.  It registers no native or mock
//! processor.  The signed observation is cryptographically real but names a
//! synthetic feed and is not devnet, provider-availability, production-release,
//! or mainnet evidence.

use std::{env, fs, path::PathBuf, str::FromStr};

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityFundingDerivationV1,
    CapabilityManifestV1, CompartmentFundingV1, FundingAmountsV1, FundingQuoteV1,
    MANIFEST_HEADER_BYTES, MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_collateral_contract::{COMPACT_TERMINAL_MARKET_BYTES, CompactTerminalMarketV1};
use dclutch_core_contract::{ContentId, MarketIdentity, MarketRoot, Phase};
use dclutch_kernel::resolution::categorical_pyth_v1::{
    CategoricalPythV1PolicyInput, MAX_PRICE_CELLS,
};
use dclutch_market_contract::market::{CategoricalMarketV1, CategoricalSettlementSummaryV1};
use dclutch_pyth_contract::{
    feed_profile::PythFeedProfileV1,
    funding::{
        FUNDING_BYTES, construct_required_resolution_funding, required_resolution_minimum_balance,
    },
    instruction::ResolveCategoricalPythV1,
    policy::CategoricalPythPolicyRecordV1,
    resolution_material::CategoricalPythResolutionMaterialV1,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1};
use solana_account::{Account, AccountSharedData};
use solana_program::{
    clock::Clock,
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::signature::{Keypair, Signer};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::{InstructionError, Transaction, TransactionError};

const PROGRAM_ID: Pubkey = Pubkey::new_from_array([71; 32]);
const GENERATION: u64 = 73;
const OPEN_CHILD_COUNT: u64 = 2;
const PROVIDER_FEE: u64 = 1;
const SUCCESS_BOUNTY: u64 = 5;
const RESOLVER_OPENING: u64 = 20_000_000;
const PUBLISH_TIME: i64 = 1_787_431_680;
const PROVIDER_EXECUTION_SLOT: u64 = 460_336_313;
const ENCODED_VAA_HEADER_BYTES: usize = 46;
const FULL_PRICE_UPDATE_BYTES: usize = 134;
const WRITE_CHUNK_BYTES: usize = 600;
const MATERIAL_SCHEMA_LABEL: &[u8] = b"dclutch/schema/categorical-pyth-resolution-material-v1";
const MANIFEST_SCHEMA_LABEL: &[u8] = b"dclutch/schema/capability-manifest-profile-1-v1";

const FIXTURE_PROVENANCE: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/PROVENANCE.md");
const UPSTREAM_LICENSE: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/UPSTREAM_LICENSE");
const RECEIVER_ELF: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/receiver.so");
const ROUTER_ELF: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/router.so");
const ROUTER_INITIALIZE: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/router-initialize.data");
const RECEIVER_INITIALIZE: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-initialize.data");
const RECEIVER_CONFIG: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-config.account");
const SIGNED_VAA: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/signed.vaa");
const RECEIVER_POST_UPDATE: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-post-update.data");
const PRICE_UPDATE: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/price-update.account");

#[derive(Clone, Copy)]
struct ProviderAddresses {
    receiver: Pubkey,
    receiver_programdata: Pubkey,
    config: Pubkey,
    router: Pubkey,
    router_programdata: Pubkey,
    guardian_set: Pubkey,
    treasury: Pubkey,
}

impl ProviderAddresses {
    fn pinned() -> Self {
        let receiver = pubkey("rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp");
        let router = pubkey("HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL");
        let (config, _) = Pubkey::find_program_address(&[b"config"], &receiver);
        let (guardian_set, _) =
            Pubkey::find_program_address(&[b"GuardianSet", &0_u32.to_be_bytes()], &router);
        let (treasury, _) = Pubkey::find_program_address(&[b"treasury", &[0]], &receiver);
        Self {
            receiver,
            receiver_programdata: pubkey("3UV7w2yTaqVcUAbWm1KUXdcE1Ziw8CfyyCpZvhKFkPfX"),
            config,
            router,
            router_programdata: pubkey("9hLWdeVhSG9ufuQFA5d6zUoZ6qXoMRWrS8i4HGFHnR1x"),
            guardian_set,
            treasury,
        }
    }
}

struct ResolutionFixture {
    test: Option<ProgramTest>,
    provider: ProviderAddresses,
    resolver: Keypair,
    update: Keypair,
    market: Pubkey,
    fund: Pubkey,
    material: Pubkey,
    manifest: Pubkey,
    rent_credit: Pubkey,
    material_cursor: Pubkey,
    manifest_cursor: Pubkey,
    market_before: Account,
    fund_before: Account,
    rent_credit_before: Account,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AtomicSnapshot {
    market: Option<Account>,
    fund: Option<Account>,
    rent_credit: Option<Account>,
    resolver: Option<Account>,
    treasury: Option<Account>,
    update: Option<Account>,
    encoded_vaa: Option<Account>,
    config: Option<Account>,
}

fn pubkey(value: &str) -> Pubkey {
    Pubkey::from_str(value).expect("pinned public address")
}

fn hex_32(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64, "SHA-256 pin must be 32 bytes");
    let mut output = [0_u8; 32];
    for (index, slot) in output.iter_mut().enumerate() {
        let start = index * 2;
        *slot = u8::from_str_radix(&value[start..start + 2], 16).expect("lowercase hex pin");
    }
    output
}

fn hex_20(value: &str) -> [u8; 20] {
    assert_eq!(value.len(), 40, "commit pin must be 20 bytes");
    let mut output = [0_u8; 20];
    for (index, slot) in output.iter_mut().enumerate() {
        let start = index * 2;
        *slot = u8::from_str_radix(&value[start..start + 2], 16).expect("lowercase hex pin");
    }
    output
}

fn assert_sha256(label: &str, bytes: &[u8], expected: &str) {
    assert_eq!(
        hash(bytes).to_bytes(),
        hex_32(expected),
        "fixture SHA-256 mismatch for {label}"
    );
}

fn assert_all_fixture_hashes() {
    for (label, bytes, digest) in [
        (
            "PROVENANCE.md",
            FIXTURE_PROVENANCE,
            "636e590b02585c98e55ad8603bf06d03c7df2426a1816958f8eae2dffca2fd87",
        ),
        (
            "UPSTREAM_LICENSE",
            UPSTREAM_LICENSE,
            "814162e3e1ec1c02ab68400bf98859ad73af3d67e19c026e98426a91085973a1",
        ),
        (
            "receiver.so",
            RECEIVER_ELF,
            "c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64",
        ),
        (
            "router.so",
            ROUTER_ELF,
            "f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb",
        ),
        (
            "router-initialize.data",
            ROUTER_INITIALIZE,
            "3667940a4428a8f2411a0ff11157ecc4ba1076c3c61273a108da6405c51e0b0b",
        ),
        (
            "receiver-initialize.data",
            RECEIVER_INITIALIZE,
            "d9c80906af92f99a0c8441f4463186056b1c12cb990999acfa198a46ec62729f",
        ),
        (
            "receiver-config.account",
            RECEIVER_CONFIG,
            "05038cf707afceac3df1aae735b096344ad639506b00f1db0ac1c084d6b645aa",
        ),
        (
            "signed.vaa",
            SIGNED_VAA,
            "ed8b973f36a932b9ec88659953859c8096f14e5aebd085bbe32b22c41a142c0d",
        ),
        (
            "receiver-post-update.data",
            RECEIVER_POST_UPDATE,
            "3bf9188bd6183155ea30738c3ab9da706ea7013bf5a7887a531e90b9bea85e1d",
        ),
        (
            "price-update.account",
            PRICE_UPDATE,
            "e5435e5b2e54d6083a9d1230e33f0635f6c74eb9db62899cfbb559f99c798a2b",
        ),
    ] {
        assert_sha256(label, bytes, digest);
    }
}

fn require_lab_sbf() {
    let directory = env::var("SBF_OUT_DIR").expect(
        "SBF_OUT_DIR is required; build the real adapter with `cargo build-sbf --manifest-path programs/dclutch-sbf/Cargo.toml --features non-production-real-pyth-lab`, then point SBF_OUT_DIR at its deploy directory",
    );
    let artifact = PathBuf::from(directory).join("dclutch_sbf.so");
    let bytes = fs::read(&artifact).unwrap_or_else(|error| {
        panic!(
            "cannot read the required compiled dClutch SBF ELF {}: {error}",
            artifact.display()
        )
    });
    assert_eq!(bytes.get(..4), Some(&[0x7f, b'E', b'L', b'F'][..]));
    assert!(
        bytes
            .windows(b"local-upgraded-2026-08-22".len())
            .any(|window| window == b"local-upgraded-2026-08-22"),
        "the compiled dClutch ELF lacks the explicit non-production-real-pyth-lab release; rebuild it with that feature instead of weakening release authentication"
    );
    eprintln!(
        "NON-PRODUCTION synthetic-local dClutch ELF SHA-256: {:?}",
        hash(&bytes).to_bytes()
    );
}

fn loader_bodies(
    program: Pubkey,
    programdata: Pubkey,
    deployment_slot: u64,
    elf: &[u8],
    program_digest: &str,
    programdata_digest: &str,
) -> (Vec<u8>, Vec<u8>) {
    let derived = Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
    assert_eq!(derived, programdata, "canonical Loader V3 ProgramData PDA");

    let mut program_body = Vec::with_capacity(36);
    program_body.extend_from_slice(&2_u32.to_le_bytes());
    program_body.extend_from_slice(programdata.as_ref());

    // The accepted fixture provenance omitted this public header field.  It was
    // recovered solely from Dragon's Clutch source commit
    // 169a1bad530d1d62b55c11acf39fa285a1740cb0,
    // docs/reviews/DEVNET_REAL_SOURCE_SNAPSHOT_2026-08-22.md:47.  It is a
    // captured Loader-account fact, not copied source code or authority used by
    // this test; that source repository's LICENSE at the same commit is
    // AGPL-3.0.  The two complete-body hashes below independently validate the
    // public fact for both programs.
    let upgrade_authority = pubkey("upg8KLALUN7ByDHiBu4wEbMDTC6UnSVFSYfTyGfXuzr");
    let mut programdata_body = Vec::with_capacity(45 + elf.len());
    programdata_body.extend_from_slice(&3_u32.to_le_bytes());
    programdata_body.extend_from_slice(&deployment_slot.to_le_bytes());
    programdata_body.push(1);
    programdata_body.extend_from_slice(upgrade_authority.as_ref());
    programdata_body.extend_from_slice(elf);

    assert_sha256("complete Program", &program_body, program_digest);
    assert_sha256(
        "complete ProgramData",
        &programdata_body,
        programdata_digest,
    );
    (program_body, programdata_body)
}

fn add_upgraded_provider_programs(test: &mut ProgramTest, provider: ProviderAddresses) {
    let rent = Rent::default();
    for (program, programdata, slot, elf, program_hash, programdata_hash) in [
        (
            provider.receiver,
            provider.receiver_programdata,
            460_336_311,
            RECEIVER_ELF,
            "ef37dd1cee22d731902a8c04ed2e13136a2b8aa7068d9db3aff2ed1ec7b634e5",
            "7122abc6b5e78d30bf88c869cb5d8783adaf897369d04eca827d3af8ffe18e5d",
        ),
        (
            provider.router,
            provider.router_programdata,
            460_336_290,
            ROUTER_ELF,
            "1ee590ae23d5ecbf775aba910f06a993dee8f77bfd7028790dbd349651c8034b",
            "f26f4b53b0f980455886116f500fa74ba475e51b1acb7f486b18afa9d73d948f",
        ),
    ] {
        let (program_body, mut programdata_body) = loader_bodies(
            program,
            programdata,
            slot,
            elf,
            program_hash,
            programdata_hash,
        );
        // ProgramTest banks start at slot 0 and Agave 4.2 cannot preload a
        // Loader-v3 program whose truthful deployment slot is far in the
        // future relative to that genesis bank.  Bootstrap only the loader's
        // deployment-slot header at zero; the ELF and public upgrade authority
        // are already exact.  After both real programs have executed once and
        // populated the cache, the full hash-pinned ProgramData bodies replace
        // these bootstrap headers before dClutch authenticates or invokes them.
        programdata_body[4..12].copy_from_slice(&0_u64.to_le_bytes());
        test.add_genesis_account(
            program,
            Account {
                lamports: rent.minimum_balance(program_body.len()),
                data: program_body,
                owner: bpf_loader_upgradeable::ID,
                executable: true,
                rent_epoch: 0,
            },
        );
        test.add_genesis_account(
            programdata,
            Account {
                lamports: rent.minimum_balance(programdata_body.len()),
                data: programdata_body,
                owner: bpf_loader_upgradeable::ID,
                executable: false,
                rent_epoch: 0,
            },
        );
    }
}

fn install_captured_programdata_accounts(
    context: &mut ProgramTestContext,
    provider: ProviderAddresses,
) {
    let rent = Rent::default();
    for (program, programdata, slot, elf, program_hash, programdata_hash) in [
        (
            provider.receiver,
            provider.receiver_programdata,
            460_336_311,
            RECEIVER_ELF,
            "ef37dd1cee22d731902a8c04ed2e13136a2b8aa7068d9db3aff2ed1ec7b634e5",
            "7122abc6b5e78d30bf88c869cb5d8783adaf897369d04eca827d3af8ffe18e5d",
        ),
        (
            provider.router,
            provider.router_programdata,
            460_336_290,
            ROUTER_ELF,
            "1ee590ae23d5ecbf775aba910f06a993dee8f77bfd7028790dbd349651c8034b",
            "f26f4b53b0f980455886116f500fa74ba475e51b1acb7f486b18afa9d73d948f",
        ),
    ] {
        let (_, programdata_body) = loader_bodies(
            program,
            programdata,
            slot,
            elf,
            program_hash,
            programdata_hash,
        );
        context.set_account(
            &programdata,
            &AccountSharedData::from(Account {
                lamports: rent.minimum_balance(programdata_body.len()),
                data: programdata_body,
                owner: bpf_loader_upgradeable::ID,
                executable: false,
                rent_epoch: 0,
            }),
        );
    }
}

fn synthetic_release_id(provider: ProviderAddresses) -> [u8; 32] {
    let mut bytes = [0_u8; 440];
    bytes[0..8].copy_from_slice(b"DCLTPR01");
    bytes[8..10].copy_from_slice(&1_u16.to_le_bytes());
    for (offset, value) in [
        (
            10,
            hex_32("4081d55d4031313fcf4b7c41313d547a9441c8f9c048741a7a951b3e035e22d9"),
        ),
        (42, provider.receiver.to_bytes()),
        (74, provider.receiver_programdata.to_bytes()),
        (106, provider.config.to_bytes()),
        (138, provider.router.to_bytes()),
        (170, provider.router_programdata.to_bytes()),
        (
            202,
            hex_32("05038cf707afceac3df1aae735b096344ad639506b00f1db0ac1c084d6b645aa"),
        ),
        (
            234,
            // Semantic receiver ABI identity from the accepted release row;
            // deliberately distinct from the receiver ELF digest.
            hex_32("c507955864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af604"),
        ),
        (
            266,
            hex_32("f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb"),
        ),
        (
            298,
            hex_32("12d0ce8bc3907ae2949043397eaf3d5bd25deed98450c6969d957be402c807ae"),
        ),
        (
            330,
            hex_32("3fdfc94589c69b133864468320976f8e790e7fe0f145897b6eabc22bd7c8711b"),
        ),
    ] {
        bytes[offset..offset + 32].copy_from_slice(&value);
    }
    bytes[362..370].copy_from_slice(&460_336_311_u64.to_le_bytes());
    bytes[370..378].copy_from_slice(&460_336_290_u64.to_le_bytes());
    bytes[378] = 19;
    bytes[379] = 10;
    bytes[380..400].copy_from_slice(&hex_20("f50a3faf9fc5a223a22889799b2f778900f186b3"));
    bytes[400..432].copy_from_slice(&hex_32(
        "245b1b03dd2177402018b6072fcbb7bea5b3d280427b1954796bf1dc189be48b",
    ));
    let release_id = hash(&bytes).to_bytes();
    assert_eq!(
        release_id,
        hex_32("2c1eb776d5e4664de1e4019c9f115aabc3c926868d9fbfd78490f07e50719641")
    );
    release_id
}

fn protocol_account(data: Vec<u8>) -> Account {
    Account {
        lamports: Rent::default().minimum_balance(data.len()),
        data,
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn finalized_record(schema_label: &[u8], content: Vec<u8>) -> (Pubkey, Pubkey, Account) {
    let schema = hash(schema_label).to_bytes();
    let digest = hash(&content).to_bytes();
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
        &PROGRAM_ID,
    )
    .0;
    let cursor = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema.as_slice(),
            digest.as_slice(),
        ],
        &PROGRAM_ID,
    )
    .0;
    (raw, cursor, protocol_account(content))
}

fn resolution_fixture() -> ResolutionFixture {
    assert_all_fixture_hashes();
    require_lab_sbf();
    let provider = ProviderAddresses::pinned();
    let release_id = synthetic_release_id(provider);
    let feed = PythFeedProfileV1::new([0x2a; 32], [0xb1; 32], [0xb2; 32])
        .expect("explicitly synthetic feed semantics");
    let upper_edges = [0_u128; MAX_PRICE_CELLS];
    let policy = CategoricalPythPolicyRecordV1::new(CategoricalPythV1PolicyInput {
        pyth_release_id: release_id,
        feed_profile_id: hash(&feed.to_bytes()).to_bytes(),
        target_time: PUBLISH_TIME,
        grace: 0,
        window: 60,
        max_crossing_lag: 0,
        max_age: 60,
        max_future_skew: 1,
        confidence_multiplier: 1,
        max_confidence_bps: 100,
        max_normalized_confidence_atoms: 10_000,
        normalized_decimals: 8,
        price_cell_count: 1,
        upper_edges,
        failure_outcome_index: 1,
    })
    .expect("fixture observation has one exact valid categorical cell");
    let material_value =
        CategoricalPythResolutionMaterialV1::new(policy, feed).expect("canonical material");
    let material_bytes = material_value.to_bytes().to_vec();
    let (material, material_cursor, material_account) =
        finalized_record(MATERIAL_SCHEMA_LABEL, material_bytes);

    let policy_id = ContentId::new(hash(&policy.to_bytes()).to_bytes()).expect("policy ID");
    let fund_rent = Rent::default().minimum_balance(FUNDING_BYTES);
    let entry = CapabilityEntryV1::new(
        ContentId::new([21; 32]).expect("capability kind"),
        ContentId::new(release_id).expect("synthetic release ID"),
        policy_id,
        ContentId::new([22; 32]).expect("capacity profile"),
        ContentId::new([23; 32]).expect("fund schema"),
        ContentId::new([24; 32]).expect("fund derivation"),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        FundingQuoteV1::new(
            FundingAmountsV1::new(
                CompartmentFundingV1::native_lamports(fund_rent)
                    .expect("Fund state rent is native lamports"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::native_lamports(PROVIDER_FEE)
                    .expect("receiver fee is native lamports"),
                CompartmentFundingV1::native_lamports(SUCCESS_BOUNTY)
                    .expect("resolver bounty is native lamports"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            )
            .expect("typed one-shot resolution funding"),
            None,
        )
        .expect("native-only one-shot resolution quote"),
    )
    .expect("canonical resolution capability");
    let mut manifest_bytes = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    let manifest_value = CapabilityManifestV1::encode_into(&[entry], &mut manifest_bytes)
        .expect("canonical manifest");
    let manifest_id =
        ContentId::new(hash(manifest_value.as_bytes()).to_bytes()).expect("manifest ID");
    let selected = manifest_value
        .required_founding_entry_for_config(policy_id)
        .expect("one required resolution entry");
    let funding =
        construct_required_resolution_funding(manifest_id, manifest_value, selected, fund_rent, 1)
            .expect("active prepaid resolution funding");
    let refund_beneficiary = Pubkey::new_from_array([82; 32]);
    let refund_authority =
        RefundAuthority::new(refund_beneficiary.to_bytes()).expect("rent beneficiary");
    let refund_authority_bytes = refund_authority.to_bytes();
    let (rent_credit, rent_credit_bump) = Pubkey::find_program_address(
        &[RENT_CREDIT_PDA_DOMAIN_V1, refund_authority_bytes.as_slice()],
        &PROGRAM_ID,
    );
    let rent_credit_state = RentCreditV1::new(refund_authority, rent_credit_bump);
    let rent_credit_before = protocol_account(rent_credit_state.to_bytes().to_vec());

    let identity = MarketIdentity::new(
        ContentId::new([31; 32]).expect("Realm ID"),
        ContentId::new([32; 32]).expect("Product ID"),
        ContentId::new([33; 32]).expect("Claim ID"),
        policy_id,
        manifest_id,
        GENERATION,
    );
    let identity_digest = hash(&identity.to_bytes()).to_bytes();
    let market = Pubkey::find_program_address(
        &[b"dclutch/market-root/v1", identity_digest.as_slice()],
        &PROGRAM_ID,
    )
    .0;
    let mut root = MarketRoot::founding(identity, refund_beneficiary.to_bytes())
        .expect("canonical founding root");
    root.register_child(GENERATION, 0).expect("Fund child");
    root.register_child(GENERATION, 1).expect("custody child");
    root.transition_phase(GENERATION, Phase::Open)
        .expect("Open prerequisite state");
    let market_value =
        CategoricalMarketV1::<2>::new(root, 0, [0, 0], CategoricalSettlementSummaryV1::empty())
            .expect("provider-neutral Open Market");
    let mut market_bytes =
        vec![0_u8; CategoricalMarketV1::<2>::encoded_len().expect("binary Market width")];
    market_value
        .encode(&mut market_bytes)
        .expect("canonical Open Market bytes");
    let market_before = protocol_account(market_bytes);

    let fund_derivation = CapabilityFundingDerivationV1::new(
        market.to_bytes(),
        GENERATION,
        manifest_id,
        manifest_value,
        funding,
    )
    .expect("canonical Fund derivation");
    let fund = Pubkey::find_program_address(&fund_derivation.seed_components(), &PROGRAM_ID).0;
    let (manifest, manifest_cursor, manifest_account) =
        finalized_record(MANIFEST_SCHEMA_LABEL, manifest_bytes);
    let fund_before = Account {
        lamports: required_resolution_minimum_balance(funding).expect("exact Fund minimum"),
        data: funding.to_bytes().to_vec(),
        owner: PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    let resolver = Keypair::new();
    let update = Keypair::new();
    let mut test = ProgramTest::new("dclutch_sbf", PROGRAM_ID, None);
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    add_upgraded_provider_programs(&mut test, provider);
    test.add_account(
        resolver.pubkey(),
        Account::new(RESOLVER_OPENING, 0, &system_program::ID),
    );
    test.add_account(market, market_before.clone());
    test.add_account(fund, fund_before.clone());
    test.add_account(material, material_account);
    test.add_account(manifest, manifest_account);
    test.add_account(rent_credit, rent_credit_before.clone());
    test.add_account(material_cursor, Account::new(0, 0, &system_program::ID));
    test.add_account(manifest_cursor, Account::new(0, 0, &system_program::ID));

    ResolutionFixture {
        test: Some(test),
        provider,
        resolver,
        update,
        market,
        fund,
        material,
        manifest,
        rent_credit,
        material_cursor,
        manifest_cursor,
        market_before,
        fund_before,
        rent_credit_before,
    }
}

fn system_create_account(
    payer: Pubkey,
    created: Pubkey,
    lamports: u64,
    space: usize,
    owner: Pubkey,
) -> Instruction {
    let mut data = Vec::with_capacity(52);
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    data.extend_from_slice(
        &u64::try_from(space)
            .expect("bounded account size")
            .to_le_bytes(),
    );
    data.extend_from_slice(owner.as_ref());
    Instruction {
        program_id: system_program::ID,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(created, true),
        ],
        data,
    }
}

fn anchor_discriminator(name: &[u8]) -> Vec<u8> {
    hash(name).to_bytes()[..8].to_vec()
}

fn write_encoded_vaa_instruction(
    router: Pubkey,
    authority: Pubkey,
    encoded_vaa: Pubkey,
    index: usize,
    bytes: &[u8],
) -> Instruction {
    let mut data = anchor_discriminator(b"global:write_encoded_vaa");
    data.extend_from_slice(
        &u32::try_from(index)
            .expect("bounded VAA index")
            .to_le_bytes(),
    );
    data.extend_from_slice(
        &u32::try_from(bytes.len())
            .expect("bounded VAA chunk")
            .to_le_bytes(),
    );
    data.extend_from_slice(bytes);
    Instruction {
        program_id: router,
        accounts: vec![
            AccountMeta::new_readonly(authority, true),
            AccountMeta::new(encoded_vaa, false),
        ],
        data,
    }
}

async fn submit(
    context: &mut ProgramTestContext,
    instructions: &[Instruction],
    signers: &[&Keypair],
) -> Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await?;
    let mut all_signers: Vec<&dyn Signer> = Vec::with_capacity(signers.len() + 1);
    all_signers.push(&context.payer);
    all_signers.extend(signers.iter().copied().map(|signer| signer as &dyn Signer));
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&context.payer.pubkey()),
        &all_signers,
        blockhash,
    );
    context.banks_client.process_transaction(transaction).await
}

async fn initialize_real_providers(
    context: &mut ProgramTestContext,
    provider: ProviderAddresses,
) -> Pubkey {
    context
        .warp_to_slot(PROVIDER_EXECUTION_SLOT)
        .expect("execute strictly after both captured ProgramData deployment slots");
    let payer = context.payer.pubkey();
    let bridge = Pubkey::find_program_address(&[b"Bridge"], &provider.router).0;
    let fee_collector = Pubkey::find_program_address(&[b"fee_collector"], &provider.router).0;
    submit(
        context,
        &[Instruction {
            program_id: provider.router,
            accounts: vec![
                AccountMeta::new(bridge, false),
                AccountMeta::new(provider.guardian_set, false),
                AccountMeta::new(fee_collector, false),
                AccountMeta::new(payer, true),
                AccountMeta::new_readonly(sysvar::clock::ID, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: ROUTER_INITIALIZE.to_vec(),
        }],
        &[],
    )
    .await
    .expect("captured router ELF accepts the pinned 19-guardian initialization");

    submit(
        context,
        &[Instruction {
            program_id: provider.receiver,
            accounts: vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(provider.config, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: RECEIVER_INITIALIZE.to_vec(),
        }],
        &[],
    )
    .await
    .expect("captured receiver ELF accepts its pinned synthetic-local Config");
    let config = observed(context, provider.config)
        .await
        .expect("receiver Config exists");
    assert_eq!(config.owner, provider.receiver);
    assert_eq!(config.data, RECEIVER_CONFIG);

    let encoded = Keypair::new();
    let encoded_size = ENCODED_VAA_HEADER_BYTES + SIGNED_VAA.len();
    submit(
        context,
        &[system_create_account(
            payer,
            encoded.pubkey(),
            Rent::default().minimum_balance(encoded_size),
            encoded_size,
            provider.router,
        )],
        &[&encoded],
    )
    .await
    .expect("create exact encoded-VAA buffer");
    submit(
        context,
        &[Instruction {
            program_id: provider.router,
            accounts: vec![
                AccountMeta::new_readonly(payer, true),
                AccountMeta::new(encoded.pubkey(), false),
            ],
            data: anchor_discriminator(b"global:init_encoded_vaa"),
        }],
        &[],
    )
    .await
    .expect("real router initializes the encoded-VAA header");
    for (chunk_index, chunk) in SIGNED_VAA.chunks(WRITE_CHUNK_BYTES).enumerate() {
        submit(
            context,
            &[write_encoded_vaa_instruction(
                provider.router,
                payer,
                encoded.pubkey(),
                chunk_index * WRITE_CHUNK_BYTES,
                chunk,
            )],
            &[],
        )
        .await
        .expect("real router writes one exact signed-VAA chunk");
    }
    submit(
        context,
        &[Instruction {
            program_id: provider.router,
            accounts: vec![
                AccountMeta::new_readonly(payer, true),
                AccountMeta::new(encoded.pubkey(), false),
                AccountMeta::new_readonly(provider.guardian_set, false),
            ],
            data: anchor_discriminator(b"global:verify_encoded_vaa_v1"),
        }],
        &[],
    )
    .await
    .expect("captured router ELF cryptographically verifies the pinned 13-of-19 VAA");
    let verified = observed(context, encoded.pubkey())
        .await
        .expect("verified EncodedVaa persists");
    assert_eq!(verified.owner, provider.router);
    assert_eq!(verified.data.len(), encoded_size);
    assert_eq!(verified.data.get(8), Some(&2), "ProcessingStatus::Verified");
    assert_eq!(verified.data.get(41), Some(&1), "verified VAA version");
    install_captured_programdata_accounts(context, provider);
    encoded.pubkey()
}

async fn set_fixture_clock(context: &mut ProgramTestContext) {
    let mut clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock sysvar");
    clock.unix_timestamp = PUBLISH_TIME;
    context.set_sysvar(&clock);
}

fn direct_post_instruction(
    provider: ProviderAddresses,
    payer: Pubkey,
    encoded_vaa: Pubkey,
    update: Pubkey,
) -> Instruction {
    Instruction {
        program_id: provider.receiver,
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(encoded_vaa, false),
            AccountMeta::new_readonly(provider.config, false),
            AccountMeta::new(provider.treasury, false),
            AccountMeta::new(update, true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(payer, true),
        ],
        data: RECEIVER_POST_UPDATE.to_vec(),
    }
}

async fn prove_full_provider_update(
    context: &mut ProgramTestContext,
    provider: ProviderAddresses,
    encoded_vaa: Pubkey,
) {
    let update = Keypair::new();
    let payer = context.payer.pubkey();
    submit(
        context,
        &[direct_post_instruction(
            provider,
            payer,
            encoded_vaa,
            update.pubkey(),
        )],
        &[&update],
    )
    .await
    .expect("real receiver posts the cryptographically verified update");
    let posted = observed(context, update.pubkey())
        .await
        .expect("full PriceUpdateV2 exists before reclaim");
    assert_eq!(posted.owner, provider.receiver);
    assert_eq!(posted.data.len(), FULL_PRICE_UPDATE_BYTES);
    assert_eq!(&posted.data[..8], &PRICE_UPDATE[..8]);
    assert_eq!(&posted.data[8..40], payer.as_ref());
    assert_eq!(&posted.data[40..125], &PRICE_UPDATE[40..125]);
    assert_eq!(posted.data[133], 0);
    assert_eq!(
        posted.lamports,
        Rent::default().minimum_balance(FULL_PRICE_UPDATE_BYTES)
    );

    submit(
        context,
        &[Instruction {
            program_id: provider.receiver,
            accounts: vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(update.pubkey(), false),
            ],
            data: anchor_discriminator(b"global:reclaim_rent"),
        }],
        &[],
    )
    .await
    .expect("real receiver reclaims the temporary update");
    assert!(observed(context, update.pubkey()).await.is_none());
}

fn price_resolution_instruction(fixture: &ResolutionFixture, encoded_vaa: Pubkey) -> Instruction {
    assert_eq!(RECEIVER_POST_UPDATE.len(), 102);
    let body = &RECEIVER_POST_UPDATE[8..];
    let request = ResolveCategoricalPythV1::new(GENERATION, OPEN_CHILD_COUNT, body)
        .expect("nonempty exact provider body");
    let mut data = vec![0_u8; 40 + body.len()];
    request.encode(&mut data).expect("exact resolve wire");
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.resolver.pubkey(), true),
            AccountMeta::new(fixture.update.pubkey(), true),
            AccountMeta::new(fixture.market, false),
            AccountMeta::new(fixture.fund, false),
            AccountMeta::new_readonly(fixture.material, false),
            AccountMeta::new_readonly(fixture.manifest, false),
            AccountMeta::new(fixture.rent_credit, false),
            AccountMeta::new_readonly(fixture.provider.receiver, false),
            AccountMeta::new_readonly(fixture.provider.receiver_programdata, false),
            AccountMeta::new_readonly(fixture.provider.config, false),
            AccountMeta::new_readonly(encoded_vaa, false),
            AccountMeta::new_readonly(fixture.provider.router, false),
            AccountMeta::new_readonly(fixture.provider.router_programdata, false),
            AccountMeta::new(fixture.provider.treasury, false),
            AccountMeta::new_readonly(fixture.material_cursor, false),
            AccountMeta::new_readonly(fixture.manifest_cursor, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ],
        data,
    }
}

fn deliberately_late_refusal(fixture: &ResolutionFixture) -> Instruction {
    let mut data = [0_u8; COMPACT_TERMINAL_MARKET_BYTES];
    CompactTerminalMarketV1::new(GENERATION)
        .encode(&mut data)
        .expect("exact compaction wire");
    Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(fixture.market, false),
            AccountMeta::new(fixture.rent_credit, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ],
        data: data.to_vec(),
    }
}

async fn observed(context: &mut ProgramTestContext, address: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(address)
        .await
        .expect("bank account query")
}

async fn snapshot(
    context: &mut ProgramTestContext,
    fixture: &ResolutionFixture,
    encoded_vaa: Pubkey,
) -> AtomicSnapshot {
    AtomicSnapshot {
        market: observed(context, fixture.market).await,
        fund: observed(context, fixture.fund).await,
        rent_credit: observed(context, fixture.rent_credit).await,
        resolver: observed(context, fixture.resolver.pubkey()).await,
        treasury: observed(context, fixture.provider.treasury).await,
        update: observed(context, fixture.update.pubkey()).await,
        encoded_vaa: observed(context, encoded_vaa).await,
        config: observed(context, fixture.provider.config).await,
    }
}

#[tokio::test]
async fn captured_programs_verify_post_and_resolve_the_synthetic_price_through_real_elfs() {
    let mut fixture = resolution_fixture();
    let provider = fixture.provider;
    let mut context = fixture
        .test
        .take()
        .expect("unstarted real-program fixture")
        .start_with_context()
        .await;
    let encoded_vaa = initialize_real_providers(&mut context, provider).await;
    set_fixture_clock(&mut context).await;
    prove_full_provider_update(&mut context, provider, encoded_vaa).await;
    let treasury_before = observed(&mut context, provider.treasury)
        .await
        .expect("probe created treasury")
        .lamports;

    submit(
        &mut context,
        &[price_resolution_instruction(&fixture, encoded_vaa)],
        &[&fixture.resolver, &fixture.update],
    )
    .await
    .expect("real dClutch ELF atomically posts, resolves, reclaims, and closes funding");

    let market = observed(&mut context, fixture.market)
        .await
        .expect("resolved Market persists");
    let market = CategoricalMarketV1::<2>::decode(&market.data).expect("resolved Market bytes");
    let resolution = market
        .settlement()
        .resolution()
        .expect("terminal categorical truth");
    assert_eq!(resolution.winner(), 0);
    assert_eq!(market.root().phase(), Phase::Resolved);
    assert_eq!(market.root().outstanding_children(), 1);
    assert!(observed(&mut context, fixture.fund).await.is_none());
    assert!(
        observed(&mut context, fixture.update.pubkey())
            .await
            .is_none()
    );
    let resolver = observed(&mut context, fixture.resolver.pubkey())
        .await
        .expect("resolver persists");
    assert_eq!(resolver.lamports, RESOLVER_OPENING + SUCCESS_BOUNTY);
    let treasury = observed(&mut context, provider.treasury)
        .await
        .expect("provider treasury persists");
    assert_eq!(treasury.lamports, treasury_before + PROVIDER_FEE);
    let rent_credit = observed(&mut context, fixture.rent_credit)
        .await
        .expect("RentCredit persists");
    assert_eq!(
        rent_credit.lamports,
        fixture.rent_credit_before.lamports + Rent::default().minimum_balance(FUNDING_BYTES)
    );
}

#[tokio::test]
async fn late_dclutch_refusal_rolls_back_provider_and_protocol_writes_together() {
    let mut fixture = resolution_fixture();
    let provider = fixture.provider;
    let mut context = fixture
        .test
        .take()
        .expect("unstarted real-program fixture")
        .start_with_context()
        .await;
    let encoded_vaa = initialize_real_providers(&mut context, provider).await;
    set_fixture_clock(&mut context).await;
    prove_full_provider_update(&mut context, provider, encoded_vaa).await;
    let before = snapshot(&mut context, &fixture, encoded_vaa).await;
    assert_eq!(before.market, Some(fixture.market_before.clone()));
    assert_eq!(before.fund, Some(fixture.fund_before.clone()));
    assert_eq!(before.rent_credit, Some(fixture.rent_credit_before.clone()));
    assert!(before.treasury.is_some());
    assert!(before.update.is_none());

    let result = submit(
        &mut context,
        &[
            price_resolution_instruction(&fixture, encoded_vaa),
            // The first instruction has completed the provider post/reclaim,
            // Market resolution, and Fund close.  Compaction then refuses
            // because the live custody child makes this Market nonterminal.
            deliberately_late_refusal(&fixture),
        ],
        &[&fixture.resolver, &fixture.update],
    )
    .await;
    assert!(
        matches!(
            result,
            Err(BanksClientError::TransactionError(
                TransactionError::InstructionError(1, InstructionError::Custom(11))
            ))
        ),
        "instruction 1 must refuse with dClutch MarketTransition after Price18 completed"
    );
    assert_eq!(
        snapshot(&mut context, &fixture, encoded_vaa).await,
        before,
        "SVM transaction rollback must restore provider treasury/update state and every dClutch Market/Fund/RentCredit write"
    );
}
