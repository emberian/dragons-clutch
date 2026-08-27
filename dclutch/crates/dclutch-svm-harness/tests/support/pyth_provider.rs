//! One provenance-pinned synthetic-local Pyth Receiver/router fixture.
//!
//! Both provider campaigns consume these exact bytes and Loader observations;
//! this module is the sole test owner of the captured provider truth.
//!
//! Generation: the two ELFs are the **upgraded** Pyth Core generation and were
//! byte-identical to the live receiver and Wormhole receiver on `mainnet-beta`
//! and `devnet` when last measured (2026-08-27, see
//! `fixtures/pyth/upgraded-2026-08-26/PROVENANCE.md`). The Loader observations
//! below reconstruct **devnet's** ProgramData accounts specifically — slot and
//! upgrade authority are per cluster even though the ELF is not.
//!
//! Everything else here is lab, not cluster: the Config admits a synthetic
//! source (chain 1, emitter `[0x01; 32]`) with fee 1 and
//! `minimum_signatures = 5`, and the guardian set is nineteen synthetic
//! upstream keys. The live generation admits one Pythnet source with fee 0,
//! `minimum_signatures = 3`, and five Pyth keys. The lab's 5-of-19 is a lab
//! shape, not a scaled model of the live 3-of-5.

use std::str::FromStr;

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
use solana_system_interface::instruction::transfer;
use solana_transaction::Transaction;

/// Captured provider publication time.
pub const PUBLISH_TIME: i64 = 1_787_431_680;
/// ProgramTest execution slot after both captured deployments.
pub const PROVIDER_EXECUTION_SLOT: u64 = 460_336_313;
const ENCODED_VAA_HEADER_BYTES: usize = 46;
const FULL_PRICE_UPDATE_BYTES: usize = 134;
const WRITE_CHUNK_BYTES: usize = 600;

/// Captured fixture provenance.
pub const FIXTURE_PROVENANCE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/PROVENANCE.md");
/// Captured upstream license.
pub const UPSTREAM_LICENSE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/UPSTREAM_LICENSE");
/// Captured Receiver ELF.
pub const RECEIVER_ELF: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver.so");
/// Captured router ELF.
pub const ROUTER_ELF: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/router.so");
const ROUTER_INITIALIZE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/router-initialize.data");
const RECEIVER_INITIALIZE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-initialize.data");
/// Captured Receiver Config account bytes.
pub const RECEIVER_CONFIG: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-config.account");
/// Captured lowercase-hex GuardianSet zero account produced by the real router.
pub const GUARDIAN_SET_0_ACCOUNT_HEX: &[u8] = include_bytes!(
    "../../../../fixtures/pyth/local-upgraded-2026-08-22/guardian-set-0.account.hex"
);
const SIGNED_VAA: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/signed.vaa");
/// Captured Receiver PostUpdate instruction bytes.
pub const RECEIVER_POST_UPDATE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-post-update.data");
/// Captured full PriceUpdate account bytes.
pub const PRICE_UPDATE: &[u8] =
    include_bytes!("../../../../fixtures/pyth/local-upgraded-2026-08-22/price-update.account");

/// Pinned provider deployment and program-owned addresses.
#[derive(Clone, Copy)]
pub struct ProviderAddresses {
    /// Receiver executable.
    pub receiver: Pubkey,
    /// Receiver ProgramData.
    pub receiver_programdata: Pubkey,
    /// Receiver Config PDA.
    pub config: Pubkey,
    /// Wormhole router executable.
    pub router: Pubkey,
    /// Router ProgramData.
    pub router_programdata: Pubkey,
    /// GuardianSet zero PDA.
    pub guardian_set: Pubkey,
    /// Receiver treasury PDA.
    pub treasury: Pubkey,
}

impl ProviderAddresses {
    /// Return the exact addresses named by the captured release.
    pub fn pinned() -> Self {
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

fn decode_lower_hex(value: &[u8]) -> Vec<u8> {
    let value = value.strip_suffix(b"\n").unwrap_or(value);
    assert_eq!(value.len() % 2, 0, "hex fixture has complete bytes");
    value
        .chunks_exact(2)
        .map(|pair| {
            (hex_nibble(*pair.first().expect("high nibble")) << 4)
                | hex_nibble(*pair.get(1).expect("low nibble"))
        })
        .collect()
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => panic!("fixture is not lowercase hexadecimal"),
    }
}

/// Recheck every accepted fixture digest before a campaign starts.
pub fn assert_all_fixture_hashes() {
    for (label, bytes, digest) in [
        (
            "PROVENANCE.md",
            FIXTURE_PROVENANCE,
            "2ac2344d5c5a2b0470349fcce305a23218ece64343277ae83f5d8c897481c874",
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
            "guardian-set-0.account.hex",
            GUARDIAN_SET_0_ACCOUNT_HEX,
            "f1b139a3e279943758a39da80a64a0115a5c7d11640bc8579eee9256f77ec146",
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

/// Install both real provider ELFs with bootstrap Loader deployment slots.
pub fn add_upgraded_provider_programs(test: &mut ProgramTest, provider: ProviderAddresses) {
    let rent = Rent::default();
    for (program, programdata, slot, elf, program_hash, programdata_hash) in provider_rows(provider)
    {
        let (program_body, mut programdata_body) = loader_bodies(
            program,
            programdata,
            slot,
            elf,
            program_hash,
            programdata_hash,
        );
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

type ProviderRow = (
    Pubkey,
    Pubkey,
    u64,
    &'static [u8],
    &'static str,
    &'static str,
);

fn provider_rows(provider: ProviderAddresses) -> [ProviderRow; 2] {
    [
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
    ]
}

fn install_captured_programdata_accounts(
    context: &mut ProgramTestContext,
    provider: ProviderAddresses,
) {
    let rent = Rent::default();
    for (program, programdata, slot, elf, program_hash, programdata_hash) in provider_rows(provider)
    {
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

/// Encode the exact synthetic-local Pyth release record.
pub fn synthetic_release_bytes(provider: ProviderAddresses) -> [u8; 440] {
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
    assert_eq!(
        hash(&bytes).to_bytes(),
        hex_32("2c1eb776d5e4664de1e4019c9f115aabc3c926868d9fbfd78490f07e50719641")
    );
    bytes
}

/// Exact synthetic-local Pyth release content identity.
pub fn synthetic_release_id(provider: ProviderAddresses) -> [u8; 32] {
    hash(&synthetic_release_bytes(provider)).to_bytes()
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

/// Submit one provider fixture transaction with the context payer plus any
/// explicit provider-owned signer accounts.
pub async fn submit(
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

/// Initialize the real provider programs and return a verified EncodedVaa.
pub async fn initialize_real_providers(
    context: &mut ProgramTestContext,
    provider: ProviderAddresses,
) -> Pubkey {
    context
        .warp_to_slot(PROVIDER_EXECUTION_SLOT)
        .expect("execute strictly after both captured ProgramData deployment slots");
    let mut initialization_clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("provider initialization Clock");
    initialization_clock.unix_timestamp = PUBLISH_TIME;
    context.set_sysvar(&initialization_clock);
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
    .expect("captured router ELF accepts the pinned synthetic 19-guardian lab initialization");
    let guardians = observed(context, provider.guardian_set)
        .await
        .expect("router GuardianSet zero exists");
    assert_eq!(guardians.owner, provider.router);
    assert_eq!(guardians.data, decode_lower_hex(GUARDIAN_SET_0_ACCOUNT_HEX));
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
    let treasury_rent = context
        .banks_client
        .get_rent()
        .await
        .expect("provider treasury Rent")
        .minimum_balance(0);
    submit(
        context,
        &[transfer(&payer, &provider.treasury, treasury_rent)],
        &[],
    )
    .await
    .expect("capitalize the canonical zero-data Receiver treasury");
    let treasury = observed(context, provider.treasury)
        .await
        .expect("Receiver treasury exists");
    assert_eq!(treasury.owner, system_program::ID);
    assert!(!treasury.executable);
    assert!(treasury.data.is_empty());
    assert_eq!(treasury.lamports, treasury_rent);

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
    .expect("captured router ELF cryptographically verifies the pinned 13-of-19 lab VAA");
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

/// Move ProgramTest time to the captured publication instant.
pub async fn set_fixture_clock(context: &mut ProgramTestContext) {
    let mut clock = context
        .banks_client
        .get_sysvar::<Clock>()
        .await
        .expect("Clock sysvar");
    clock.unix_timestamp = PUBLISH_TIME;
    context.set_sysvar(&clock);
}

/// Construct the exact direct Receiver PostUpdate instruction.
pub fn direct_post_instruction(
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

/// Exercise one full direct post/reclaim against both captured provider ELFs.
pub async fn prove_full_provider_update(
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

async fn observed(context: &mut ProgramTestContext, address: Pubkey) -> Option<Account> {
    context
        .banks_client
        .get_account(address)
        .await
        .expect("bank account query")
}
