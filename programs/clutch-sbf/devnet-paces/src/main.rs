//! Devnet paces driver for the Dragon's Clutch market lifecycle.
//!
//! Runs the full public-cluster campaign against a DEPLOYED program: the real
//! value-neutral prefix (Token-2022 collateral mint, sealed policy/grid/Terms
//! artifacts, Realm, Profile, `CreateMarket` allocating the whole market
//! plane), then the asserted refusal boundaries that prove the deployed ELF
//! refuses value admission exactly as sealed.  Fresh throwaway keys only; the
//! sole keypair read is the `--payer` path.  Mainnet is refused twice: by URL
//! admission and by genesis hash.
//!
//! Claim vocabulary: a green run is PUBLIC-TESTNET evidence for the deployed
//! ELF at `--program-id`.  It is not local SBF-EXECUTED evidence (that lives
//! in `svm-tests` and the committed gate) and it is not mainnet anything.
//! The funded mock walk is devnet-impossible; `steps::devnet_impossible`
//! enumerates exactly which local steps die on a public cluster and which
//! asserted refusal replaces each.

mod rpc;
mod steps;
mod transcript;
mod walk;

use clutch_kernel::BasisMode;
use clutch_solana_layout::{
    native_resolution::{NativeResolutionAccount, NATIVE_RESOLUTION_LEN},
    HoardAccount, MarketAccount, PositionAccount, ProfileAccount, RealmAccount,
    SupplyLedgerAccount,
};
use clutch_solana_reference::{KernelAccount, ReplayAccount, KERNEL_ACCOUNT_LEN};
use rpc::{AccountView, Rpc};
use serde_json::Value;
use solana_address::Address;
use solana_hash::Hash;
use solana_instruction::Instruction;
use solana_keypair::{read_keypair_file, write_keypair_file, Keypair};
use solana_program_pack::Pack;
use solana_signer::Signer;
use solana_system_interface::instruction as system_instruction;
use solana_transaction::Transaction;
use spl_token_2022_interface::{
    extension::StateWithExtensions,
    instruction as token_instruction,
    instruction::AuthorityType,
    state::{Account as TokenAccount, Mint},
};
use std::{
    collections::BTreeMap,
    env,
    path::PathBuf,
    process,
    str::FromStr,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use steps::{Profile, StepKind};
use transcript::{sha256_hex, BoundaryRecord, ReloadRecord, StepRecord, Transcript};
use walk::{ArtifactRoute, Walk, DEGREE, SETS};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type Snapshot = BTreeMap<String, Option<AccountView>>;

/// Lamports granted to the fresh actor: it pays rent for the whole market
/// plane it founds (well under 0.1 SOL) with ample headroom.
const ACTOR_FUNDING: u64 = 500_000_000;
/// Minimum payer balance to start: the actor grant (in case the faucet
/// refuses and the payer funds it) plus artifact/realm/profile rents and fees.
const PAYER_MINIMUM: u64 = 700_000_000;
/// Artifact stages live this many slots past the begin slot (bounds: 8 and
/// 432 000); roughly 48 minutes, far beyond one campaign.
const ARTIFACT_LIFETIME_SLOTS: u64 = 7_200;

struct Args {
    url: String,
    program_id: String,
    payer: String,
    profile: Profile,
    out: PathBuf,
    throttle_ms: u64,
}

fn parse_args(args: &[String]) -> std::result::Result<Args, String> {
    let mut url = "https://api.devnet.solana.com".to_string();
    let mut program_id = None;
    let mut payer = None;
    let mut profile = None;
    let mut out = None;
    let mut throttle_ms = 400_u64;
    let mut iterator = args.iter();
    while let Some(flag) = iterator.next() {
        let mut value = |name: &str| {
            iterator
                .next()
                .cloned()
                .ok_or_else(|| format!("{name} requires a value"))
        };
        match flag.as_str() {
            "--url" => url = value("--url")?,
            "--program-id" => program_id = Some(value("--program-id")?),
            "--payer" => payer = Some(value("--payer")?),
            "--profile" => {
                let text = value("--profile")?;
                profile = Some(
                    Profile::parse(&text)
                        .ok_or_else(|| format!("unknown profile {text}: use default|mock"))?,
                );
            }
            "--out" => out = Some(PathBuf::from(value("--out")?)),
            "--throttle-ms" => {
                throttle_ms = value("--throttle-ms")?
                    .parse()
                    .map_err(|error| format!("--throttle-ms: {error}"))?;
            }
            other => return Err(format!("unknown flag {other}")),
        }
    }
    Ok(Args {
        url,
        program_id: program_id.ok_or("--program-id is required")?,
        payer: payer.ok_or("--payer is required")?,
        profile: profile.ok_or("--profile is required (default|mock)")?,
        out: out.ok_or("--out is required")?,
        throttle_ms,
    })
}

fn usage() -> ! {
    eprintln!(
        "usage: devnet-paces --program-id <base58> --payer <keypair.json> \
         --profile default|mock --out <dir> [--url <https-devnet-or-loopback>] \
         [--throttle-ms <n>]"
    );
    process::exit(2);
}

/* ------------------------------------------------------------------------ */
/* Step-name plumbing                                                        */
/* ------------------------------------------------------------------------ */

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArtifactAction {
    Begin,
    Write(usize),
    Seal,
}

fn artifact_step(name: &str) -> Option<(ArtifactRoute, ArtifactAction)> {
    let (route_name, action_name) = name.split_once("-artifact-")?;
    let route = match route_name {
        "policy" => ArtifactRoute::Policy,
        "grid" => ArtifactRoute::Grid,
        "terms" => ArtifactRoute::Terms,
        _ => return None,
    };
    let action = match action_name {
        "begin" => ArtifactAction::Begin,
        "seal" => ArtifactAction::Seal,
        other => ArtifactAction::Write(other.strip_prefix("write-")?.parse().ok()?),
    };
    Some((route, action))
}

fn refusal_error(status: &Value) -> Option<(u64, u64)> {
    let parts = status
        .get("err")?
        .get("InstructionError")?
        .as_array()?;
    let index = parts.first()?.as_u64()?;
    let code = parts.get(1)?.get("Custom")?.as_u64()?;
    Some((index, code))
}

/* ------------------------------------------------------------------------ */
/* Submission                                                                */
/* ------------------------------------------------------------------------ */

/// Sign against a fresh blockhash, submit, and confirm; re-sign only after
/// the previous blockhash has provably expired unobserved (safe from double
/// execution).
fn sign_submit_confirm(
    rpc: &mut Rpc,
    payer: &Keypair,
    extras: &[&Keypair],
    instructions: &[Instruction],
) -> Result<(String, Value)> {
    for _attempt in 0..3 {
        let blockhash_text = rpc.latest_blockhash()?;
        let blockhash = Hash::from_str(&blockhash_text)
            .map_err(|error| format!("blockhash {blockhash_text} does not parse: {error}"))?;
        let mut signers: Vec<&Keypair> = vec![payer];
        signers.extend_from_slice(extras);
        let transaction = Transaction::new_signed_with_payer(
            instructions,
            Some(&payer.pubkey()),
            &signers,
            blockhash,
        );
        let wire = bincode::serialize(&transaction)?;
        if let Some(confirmed) = rpc.submit_and_confirm(&wire, &blockhash_text)? {
            return Ok(confirmed);
        }
        eprintln!("  blockhash expired unconfirmed; re-signing");
    }
    Err("transaction could not be confirmed across three blockhashes".into())
}

fn await_signature(rpc: &mut Rpc, signature: &str, timeout: Duration) -> Result<Option<Value>> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(status) = rpc.signature_status(signature)? {
            let confirmation = status
                .get("confirmationStatus")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if confirmation == "confirmed" || confirmation == "finalized" {
                return Ok(Some(status));
            }
        }
        std::thread::sleep(Duration::from_millis(1_200));
    }
    Ok(None)
}

/* ------------------------------------------------------------------------ */
/* Reload checks                                                             */
/* ------------------------------------------------------------------------ */

fn reload(rpc: &mut Rpc, role: &str, address: Address) -> Result<(AccountView, ReloadRecord)> {
    let text = address.to_string();
    let view = rpc
        .account(&text, false)?
        .ok_or_else(|| format!("{role} ({text}) is absent after an accepted step"))?;
    let record = ReloadRecord {
        role: role.to_string(),
        address: text,
        len: view.data.len(),
        sha256: sha256_hex(&view.data),
    };
    Ok((view, record))
}

fn require(condition: bool, detail: &str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(detail.to_string().into())
    }
}

fn classify_network(url: &str, genesis: &str) -> Result<&'static str> {
    if url == rpc::PUBLIC_DEVNET_RPC {
        require(
            genesis == rpc::DEVNET_GENESIS,
            "the canonical devnet RPC endpoint does not report the pinned DEVNET genesis hash; refusing to continue",
        )?;
        Ok("devnet")
    } else {
        require(
            genesis != rpc::MAINNET_GENESIS,
            "the loopback RPC endpoint reports the MAINNET genesis hash; refusing to continue",
        )?;
        Ok("loopback-or-other")
    }
}

fn check_token(
    rpc: &mut Rpc,
    role: &str,
    address: Address,
    mint: Address,
    owner: Address,
    amount: u64,
) -> Result<ReloadRecord> {
    let (view, record) = reload(rpc, role, address)?;
    let token = StateWithExtensions::<TokenAccount>::unpack(&view.data)
        .map_err(|error| format!("{role}: token account does not unpack: {error}"))?;
    require(token.base.mint == mint, &format!("{role}: wrong mint"))?;
    require(token.base.owner == owner, &format!("{role}: wrong owner"))?;
    require(
        token.base.amount == amount,
        &format!(
            "{role}: amount {} differs from expected {amount}",
            token.base.amount
        ),
    )?;
    Ok(record)
}

fn check_mint(
    rpc: &mut Rpc,
    role: &str,
    address: Address,
    supply: u64,
    authority_gone: bool,
) -> Result<ReloadRecord> {
    let (view, record) = reload(rpc, role, address)?;
    let mint = StateWithExtensions::<Mint>::unpack(&view.data)
        .map_err(|error| format!("{role}: mint does not unpack: {error}"))?;
    require(
        mint.base.supply == supply,
        &format!("{role}: supply {} differs from {supply}", mint.base.supply),
    )?;
    if authority_gone {
        require(
            mint.base.mint_authority.is_none(),
            &format!("{role}: mint authority survived the freeze"),
        )?;
    }
    Ok(record)
}

fn check_bytes(
    rpc: &mut Rpc,
    role: &str,
    address: Address,
    program_id: Address,
    expected: &[u8],
) -> Result<ReloadRecord> {
    let (view, record) = reload(rpc, role, address)?;
    require(
        view.owner == program_id.to_string(),
        &format!("{role}: wrong owning program {}", view.owner),
    )?;
    require(
        view.data == expected,
        &format!(
            "{role}: committed bytes differ (observed {}, expected {})",
            view.data.len(),
            expected.len()
        ),
    )?;
    Ok(record)
}

fn check_absent(rpc: &mut Rpc, role: &str, address: Address) -> Result<()> {
    let text = address.to_string();
    require(
        rpc.account(&text, false)?.is_none(),
        &format!("{role} ({text}) exists but must be absent"),
    )
}

fn check_realm(rpc: &mut Rpc, walk: &Walk) -> Result<Vec<ReloadRecord>> {
    let (view, record) = reload(rpc, "realm", walk.realm)?;
    let realm = RealmAccount::decode(&view.data)
        .map_err(|error| format!("realm does not decode: {error:?}"))?;
    require(realm.realm == walk.realm_id, "realm: wrong realm identity")?;
    require(realm.profile == walk.profile_id, "realm: wrong profile")?;
    Ok(vec![record])
}

fn check_profile(rpc: &mut Rpc, walk: &Walk) -> Result<Vec<ReloadRecord>> {
    let (view, record) = reload(rpc, "profile", walk.profile)?;
    let profile = ProfileAccount::decode(&view.data)
        .map_err(|error| format!("profile does not decode: {error:?}"))?;
    require(profile.profile == walk.profile_id, "profile: wrong identity")?;
    require(profile.realm == walk.realm_id, "profile: wrong realm")?;
    require(
        profile.collateral_policy_digest == walk.policy_digest,
        "profile: wrong policy digest",
    )?;
    Ok(vec![record])
}

/// The full blank-plane reload of `CreateMarket`, mirroring the local gate's
/// `assert_blank_bank_reload` and extending it with the token-side accounts.
fn check_market_plane(rpc: &mut Rpc, walk: &Walk) -> Result<Vec<ReloadRecord>> {
    let mut records = Vec::new();

    let (view, record) = reload(rpc, "market", walk.market)?;
    records.push(record);
    let market = MarketAccount::decode(&view.data)
        .map_err(|error| format!("market does not decode: {error:?}"))?;
    require(market.market == walk.market_id, "market: wrong identity")?;
    require(market.realm == walk.realm_id, "market: wrong realm")?;
    require(market.profile == walk.profile_id, "market: wrong profile")?;
    require(market.terms == walk.terms_id, "market: wrong terms")?;
    require(market.outcome_count == walk::OUTCOMES, "market: wrong outcomes")?;
    require(market.lifecycle == 0, "market: lifecycle is not blank")?;
    require(
        market.collateral_cap == walk.terms_value.collateral_cap,
        "market: wrong collateral cap",
    )?;

    let (view, record) = reload(rpc, "kernel", walk.kernel)?;
    records.push(record);
    require(
        view.data.len() == KERNEL_ACCOUNT_LEN,
        "kernel: wrong length",
    )?;
    let kernel = KernelAccount::decode(&view.data)
        .map_err(|error| format!("kernel does not decode: {error:?}"))?;
    require(
        kernel.basis_mode == BasisMode::DerivedBasis,
        "kernel: wrong basis mode",
    )?;
    require(kernel.phase == 0, "kernel: phase is not blank")?;
    require(
        kernel.total_supply.iter().all(|supply| *supply == 0),
        "kernel: supply is not zero",
    )?;

    let (view, record) = reload(rpc, "resolution", walk.resolution)?;
    records.push(record);
    require(
        view.data.len() == NATIVE_RESOLUTION_LEN,
        "resolution: wrong length",
    )?;
    let resolution = NativeResolutionAccount::decode(&view.data)
        .map_err(|error| format!("resolution does not decode: {error:?}"))?;
    require(!resolution.is_resolved(), "resolution: already resolved")?;
    require(resolution.market == walk.market_id, "resolution: wrong market")?;

    let (view, record) = reload(rpc, "position", walk.position)?;
    records.push(record);
    let position = PositionAccount::decode(&view.data)
        .map_err(|error| format!("position does not decode: {error:?}"))?;
    require(position.cash_atoms == 0, "position: cash is not zero")?;
    require(
        position.internal.iter().all(|held| *held == 0),
        "position: holdings are not zero",
    )?;

    let (view, record) = reload(rpc, "hoard", walk.hoard)?;
    records.push(record);
    let hoard = HoardAccount::decode(&view.data)
        .map_err(|error| format!("hoard does not decode: {error:?}"))?;
    require(hoard.collateral_atoms == 0, "hoard: collateral is not zero")?;

    let (view, record) = reload(rpc, "supply", walk.supply)?;
    records.push(record);
    let supply = SupplyLedgerAccount::decode(&view.data)
        .map_err(|error| format!("supply ledger does not decode: {error:?}"))?;
    require(
        supply
            .internal_supply
            .iter()
            .chain(supply.external_supply.iter())
            .all(|term| *term == 0),
        "supply: ledger is not zero",
    )?;

    let (view, record) = reload(rpc, "replay", walk.replay)?;
    records.push(record);
    let replay = ReplayAccount::decode(&view.data)
        .map_err(|error| format!("replay does not decode: {error:?}"))?;
    require(replay.sequence == 0, "replay: sequence is not zero")?;

    records.push(check_token(
        rpc,
        "hoard-token",
        walk.hoard_token,
        walk.collateral_mint,
        walk.hoard_authority,
        0,
    )?);
    for (index, mint) in walk.outcome_mints.iter().enumerate() {
        records.push(check_mint(
            rpc,
            &format!("outcome-mint-{index}"),
            *mint,
            0,
            false,
        )?);
    }
    Ok(records)
}

/* ------------------------------------------------------------------------ */
/* Watched-state snapshots                                                   */
/* ------------------------------------------------------------------------ */

fn watch_list(walk: &Walk) -> Vec<(String, Address)> {
    let mut list = vec![
        ("realm".to_string(), walk.realm),
        ("profile".to_string(), walk.profile),
        ("policy".to_string(), walk.policy_account),
        ("grid".to_string(), walk.grid_account),
        ("terms".to_string(), walk.terms_account),
        ("market".to_string(), walk.market),
        ("hoard".to_string(), walk.hoard),
        ("position".to_string(), walk.position),
        ("kernel".to_string(), walk.kernel),
        ("replay".to_string(), walk.replay),
        ("supply".to_string(), walk.supply),
        ("resolution".to_string(), walk.resolution),
        ("hoard-token".to_string(), walk.hoard_token),
        ("collateral-mint".to_string(), walk.collateral_mint),
        ("actor-collateral".to_string(), walk.actor_token),
        ("bearer-collateral".to_string(), walk.bearer_token),
        ("feed".to_string(), walk.feed),
        ("source-spec".to_string(), walk.source_spec),
        ("source-archive".to_string(), walk.source_archive),
    ];
    for (index, mint) in walk.outcome_mints.iter().enumerate() {
        list.push((format!("outcome-mint-{index}"), *mint));
    }
    list
}

fn snapshot(rpc: &mut Rpc, list: &[(String, Address)]) -> Result<Snapshot> {
    let mut out = Snapshot::new();
    for (role, address) in list {
        out.insert(role.clone(), rpc.account(&address.to_string(), false)?);
    }
    Ok(out)
}

/* ------------------------------------------------------------------------ */
/* Funding                                                                   */
/* ------------------------------------------------------------------------ */

/// Fund the actor with [`ACTOR_FUNDING`] lamports: faucet first, payer
/// transfer when the faucet refuses or stalls.
fn fund_actor(
    rpc: &mut Rpc,
    payer: &Keypair,
    actor: Address,
) -> Result<(String, Option<String>)> {
    let target = actor.to_string();
    if rpc.balance(&target)? >= ACTOR_FUNDING {
        return Ok(("already-funded".to_string(), None));
    }
    match rpc.request_airdrop(&target, ACTOR_FUNDING) {
        Ok(signature) => {
            if await_signature(rpc, &signature, Duration::from_secs(40))?.is_some()
                && rpc.balance(&target)? >= ACTOR_FUNDING
            {
                return Ok(("airdrop".to_string(), Some(signature)));
            }
            eprintln!("  airdrop did not confirm; falling back to a payer transfer");
        }
        Err(error) => {
            eprintln!("  airdrop refused ({error}); falling back to a payer transfer");
        }
    }
    let instructions = [system_instruction::transfer(
        &payer.pubkey(),
        &actor,
        ACTOR_FUNDING,
    )];
    let (signature, _) = sign_submit_confirm(rpc, payer, &[], &instructions)?;
    require(
        rpc.balance(&target)? >= ACTOR_FUNDING,
        "actor balance is still short after the payer transfer",
    )?;
    Ok(("payer-transfer".to_string(), Some(signature)))
}

/* ------------------------------------------------------------------------ */
/* The campaign                                                              */
/* ------------------------------------------------------------------------ */

struct Identities {
    actor: Keypair,
    bearer: Keypair,
    collateral_mint: Keypair,
    actor_token: Keypair,
    bearer_token: Keypair,
}

impl Identities {
    fn fresh() -> Self {
        Self {
            actor: Keypair::new(),
            bearer: Keypair::new(),
            collateral_mint: Keypair::new(),
            actor_token: Keypair::new(),
            bearer_token: Keypair::new(),
        }
    }

    fn persist(&self, directory: &std::path::Path) -> Result<()> {
        std::fs::create_dir_all(directory)?;
        for (name, keypair) in [
            ("actor", &self.actor),
            ("bearer", &self.bearer),
            ("collateral-mint", &self.collateral_mint),
            ("actor-collateral-token", &self.actor_token),
            ("bearer-collateral-token", &self.bearer_token),
        ] {
            let path = directory.join(format!("{name}.json"));
            write_keypair_file(keypair, &path)
                .map_err(|error| format!("writing {name} keypair: {error}"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt as _;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            }
        }
        Ok(())
    }
}

fn run(args: &Args, transcript: &mut Transcript) -> Result<()> {
    rpc::admit_url(&args.url)?;
    let payer = read_keypair_file(&args.payer)
        .map_err(|error| format!("payer keypair {}: {error}", args.payer))?;
    let mut rpc = Rpc::new(&args.url, args.throttle_ms);

    /* Preflight: never mainnet, program deployed and executable, payer able
     * to carry the walk. */
    let genesis = rpc.genesis_hash()?;
    let network = classify_network(&args.url, &genesis)?;
    transcript.genesis_hash.clone_from(&genesis);
    transcript.network = network.to_string();
    transcript.claim = transcript::claim_line(args.profile.name(), &transcript.network);

    let program_id = Address::from_str(&args.program_id)
        .map_err(|error| format!("--program-id does not parse: {error}"))?;
    let program_account = rpc
        .account(&args.program_id, true)?
        .ok_or("the program account does not exist on this cluster")?;
    require(
        program_account.executable,
        "the program account exists but is not executable",
    )?;
    transcript.program_owner.clone_from(&program_account.owner);

    let start_slot = rpc.slot()?;
    transcript.start_slot = start_slot;
    let payer_balance = rpc.balance(&payer.pubkey().to_string())?;
    require(
        payer_balance >= PAYER_MINIMUM,
        &format!(
            "payer holds {payer_balance} lamports; at least {PAYER_MINIMUM} are required \
             (actor grant {ACTOR_FUNDING} plus rents and fees)"
        ),
    )?;

    /* Fresh throwaway identities, persisted beside the transcript. */
    let identities = Identities::fresh();
    identities.persist(&args.out.join("keys"))?;
    transcript.payer = payer.pubkey().to_string();
    for (name, address) in [
        ("actor", identities.actor.pubkey()),
        ("bearer", identities.bearer.pubkey()),
        ("collateral-mint", identities.collateral_mint.pubkey()),
        ("actor-collateral-token", identities.actor_token.pubkey()),
        ("bearer-collateral-token", identities.bearer_token.pubkey()),
    ] {
        transcript
            .identities
            .insert(name.to_string(), address.to_string());
    }

    let unix_now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("wall clock: {error}"))?
        .as_secs();
    let walk = Walk::build(
        program_id,
        payer.pubkey(),
        identities.actor.pubkey(),
        identities.bearer.pubkey(),
        identities.collateral_mint.pubkey(),
        identities.actor_token.pubkey(),
        identities.bearer_token.pubkey(),
        unix_now.saturating_sub(2),
        unix_now,
    )?;
    for (name, address) in [
        ("realm", walk.realm),
        ("profile", walk.profile),
        ("policy", walk.policy_account),
        ("grid", walk.grid_account),
        ("terms", walk.terms_account),
        ("market", walk.market),
        ("hoard", walk.hoard),
        ("position", walk.position),
        ("kernel", walk.kernel),
        ("replay", walk.replay),
        ("supply", walk.supply),
        ("resolution", walk.resolution),
        ("hoard-authority", walk.hoard_authority),
        ("hoard-token", walk.hoard_token),
        ("feed", walk.feed),
        ("source-spec", walk.source_spec),
        ("source-archive", walk.source_archive),
    ] {
        transcript
            .addresses
            .insert(name.to_string(), address.to_string());
    }
    for (index, mint) in walk.outcome_mints.iter().enumerate() {
        transcript
            .addresses
            .insert(format!("outcome-mint-{index}"), mint.to_string());
    }
    transcript.addresses.insert(
        "window-id-hex".to_string(),
        transcript::hex(&walk.window_id.bytes()),
    );
    transcript.addresses.insert(
        "feed-id-hex".to_string(),
        transcript::hex(&walk.feed_id.bytes()),
    );

    for boundary in steps::devnet_impossible(args.profile) {
        transcript.boundaries.push(BoundaryRecord {
            local_step: boundary.local_step.to_string(),
            status: "devnet-impossible".to_string(),
            reason: boundary.reason.to_string(),
            asserted_instead: boundary.asserted_instead.to_string(),
        });
    }

    let rent_mint = rpc.minimum_rent(Mint::LEN)?;
    let rent_token = rpc.minimum_rent(TokenAccount::LEN)?;
    let expires_slot = start_slot + ARTIFACT_LIFETIME_SLOTS;
    let watch = watch_list(&walk);
    let mut watched_before: Option<Snapshot> = None;

    let plan = steps::step_table(args.profile);
    for (ordinal, step) in plan.iter().enumerate() {
        let name = step.name.as_str();
        println!("{:>2} {name}", ordinal + 1);

        /* Funding is its own step shape: the faucet is not a transaction this
         * driver signs. */
        if name == "fund-actor" {
            let (method, signature) =
                fund_actor(&mut rpc, &payer, identities.actor.pubkey())?;
            transcript.steps.push(StepRecord {
                ordinal: ordinal + 1,
                name: name.to_string(),
                kind: "funding".to_string(),
                expect_code: None,
                signature,
                slot: None,
                confirmation: None,
                observed_error: None,
                method: Some(method),
                reloads: Vec::new(),
                watched_unchanged: None,
            });
            continue;
        }

        /* Build the step's instructions and extra signers. */
        let budget = Walk::budget();
        let (instructions, extras): (Vec<Instruction>, Vec<&Keypair>) =
            if let Some((route, action)) = artifact_step(name) {
                let instruction = match action {
                    ArtifactAction::Begin => walk.artifact_begin(route, expires_slot),
                    ArtifactAction::Write(cursor) => walk.artifact_write(route, cursor),
                    ArtifactAction::Seal => walk.artifact_seal(route),
                };
                (vec![budget, instruction], vec![])
            } else {
                match name {
                    "create-collateral-mint" => (
                        vec![
                            system_instruction::create_account(
                                &payer.pubkey(),
                                &walk.collateral_mint,
                                rent_mint,
                                u64::try_from(Mint::LEN)?,
                                &clutch_svm_fixture::TOKEN_2022,
                            ),
                            token_instruction::initialize_mint2(
                                &clutch_svm_fixture::TOKEN_2022,
                                &walk.collateral_mint,
                                &payer.pubkey(),
                                None,
                                6,
                            )?,
                        ],
                        vec![&identities.collateral_mint],
                    ),
                    "create-actor-collateral-token" | "create-bearer-collateral-token" => {
                        let (account, owner, keypair) = if name.starts_with("create-actor") {
                            (walk.actor_token, walk.actor, &identities.actor_token)
                        } else {
                            (walk.bearer_token, walk.bearer, &identities.bearer_token)
                        };
                        (
                            vec![
                                system_instruction::create_account(
                                    &payer.pubkey(),
                                    &account,
                                    rent_token,
                                    u64::try_from(TokenAccount::LEN)?,
                                    &clutch_svm_fixture::TOKEN_2022,
                                ),
                                token_instruction::initialize_account3(
                                    &clutch_svm_fixture::TOKEN_2022,
                                    &account,
                                    &walk.collateral_mint,
                                    &owner,
                                )?,
                            ],
                            vec![keypair],
                        )
                    }
                    "mint-collateral-and-freeze" => (
                        vec![
                            token_instruction::mint_to(
                                &clutch_svm_fixture::TOKEN_2022,
                                &walk.collateral_mint,
                                &walk.actor_token,
                                &payer.pubkey(),
                                &[],
                                SETS,
                            )?,
                            token_instruction::set_authority(
                                &clutch_svm_fixture::TOKEN_2022,
                                &walk.collateral_mint,
                                None,
                                AuthorityType::MintTokens,
                                &payer.pubkey(),
                                &[],
                            )?,
                        ],
                        vec![],
                    ),
                    "init-realm" => (vec![budget, walk.init_realm()], vec![]),
                    "init-profile" => (vec![budget, walk.init_profile()], vec![]),
                    "create-market" => (
                        vec![budget, walk.create_market()],
                        vec![&identities.actor],
                    ),
                    "init-source-spec-refused" => (vec![budget, walk.init_source_spec()], vec![]),
                    "init-source-archive-refused" => {
                        (vec![budget, walk.init_source_archive()], vec![])
                    }
                    "endow-refused-no-spec" => (
                        vec![budget, walk.endow(0, SETS)],
                        vec![&identities.actor],
                    ),
                    other => return Err(format!("unplanned step {other}").into()),
                }
            };

        /* Refusals watch state around the confirmed failure. */
        let before = if matches!(step.kind, StepKind::Refuse { .. }) {
            Some(match watched_before.take() {
                Some(existing) => existing,
                None => snapshot(&mut rpc, &watch)?,
            })
        } else {
            None
        };

        let (signature, status) = sign_submit_confirm(&mut rpc, &payer, &extras, &instructions)?;
        let confirmation = status
            .get("confirmationStatus")
            .and_then(Value::as_str)
            .unwrap_or("unknown")
            .to_string();
        let slot = status.get("slot").and_then(Value::as_u64);
        let observed_error = status.get("err").filter(|err| !err.is_null()).cloned();

        let mut record = StepRecord {
            ordinal: ordinal + 1,
            name: name.to_string(),
            kind: match step.kind {
                StepKind::Accept => "accept".to_string(),
                StepKind::Refuse { .. } => "refuse".to_string(),
            },
            expect_code: match step.kind {
                StepKind::Accept => None,
                StepKind::Refuse { code } => Some(code),
            },
            signature: Some(signature.clone()),
            slot,
            confirmation: Some(confirmation.clone()),
            observed_error: observed_error.clone(),
            method: None,
            reloads: Vec::new(),
            watched_unchanged: None,
        };

        match step.kind {
            StepKind::Accept => {
                if let Some(error) = &observed_error {
                    transcript.steps.push(record);
                    return Err(format!("{name}: expected success, observed {error}").into());
                }
                record.reloads = accept_checks(&mut rpc, &walk, name)?;
            }
            StepKind::Refuse { code } => {
                let Some((index, observed)) =
                    observed_error.as_ref().and_then(|_| refusal_error(&status))
                else {
                    transcript.steps.push(record);
                    return Err(format!(
                        "{name}: expected Custom({code:#06x}), observed {observed_error:?}"
                    )
                    .into());
                };
                if index != 1 || observed != u64::from(code) {
                    transcript.steps.push(record);
                    return Err(format!(
                        "{name}: expected Custom({code:#06x}) at instruction 1, observed \
                         Custom({observed:#06x}) at instruction {index}"
                    )
                    .into());
                }
                let after = snapshot(&mut rpc, &watch)?;
                let before = before.expect("refusals snapshot before submitting");
                if before != after {
                    let changed: Vec<&String> = before
                        .iter()
                        .filter(|(role, view)| after.get(*role) != Some(view))
                        .map(|(role, _)| role)
                        .collect();
                    transcript.steps.push(record);
                    return Err(format!(
                        "{name}: refused transaction changed watched state: {changed:?}"
                    )
                    .into());
                }
                for role in ["source-spec", "feed", "source-archive"] {
                    require(
                        after.get(role).is_some_and(Option::is_none),
                        &format!("{name}: {role} exists after an asserted refusal"),
                    )?;
                }
                record.watched_unchanged = Some(after.len());
                watched_before = Some(after);
            }
        }

        println!(
            "   {} confirmed={confirmation} signature={signature}",
            match step.kind {
                StepKind::Accept => "accept".to_string(),
                StepKind::Refuse { code } => format!("refuse Custom({code:#06x})"),
            },
        );
        transcript.steps.push(record);
    }
    Ok(())
}

/// The reload expectations of each accepted step.
fn accept_checks(rpc: &mut Rpc, walk: &Walk, name: &str) -> Result<Vec<ReloadRecord>> {
    if let Some((route, action)) = artifact_step(name) {
        if action == ArtifactAction::Seal {
            let (_, _, _, final_account, stage, body) = walk.artifact_route(route);
            let record = check_bytes(rpc, name, final_account, walk.program_id, body)?;
            check_absent(rpc, "artifact-stage", stage)?;
            return Ok(vec![record]);
        }
        return Ok(Vec::new());
    }
    match name {
        "create-collateral-mint" => {
            Ok(vec![check_mint(rpc, name, walk.collateral_mint, 0, false)?])
        }
        "create-actor-collateral-token" => Ok(vec![check_token(
            rpc,
            name,
            walk.actor_token,
            walk.collateral_mint,
            walk.actor,
            0,
        )?]),
        "create-bearer-collateral-token" => Ok(vec![check_token(
            rpc,
            name,
            walk.bearer_token,
            walk.collateral_mint,
            walk.bearer,
            0,
        )?]),
        "mint-collateral-and-freeze" => Ok(vec![
            check_token(
                rpc,
                "actor-collateral",
                walk.actor_token,
                walk.collateral_mint,
                walk.actor,
                SETS,
            )?,
            check_mint(rpc, "collateral-mint", walk.collateral_mint, SETS, true)?,
        ]),
        "init-realm" => check_realm(rpc, walk),
        "init-profile" => check_profile(rpc, walk),
        "create-market" => {
            let records = check_market_plane(rpc, walk)?;
            for (role, address) in [
                ("source-spec", walk.source_spec),
                ("feed", walk.feed),
                ("source-archive", walk.source_archive),
            ] {
                check_absent(rpc, role, address)?;
            }
            Ok(records)
        }
        _ => Ok(Vec::new()),
    }
}

fn main() {
    let raw: Vec<String> = env::args().skip(1).collect();
    let args = match parse_args(&raw) {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            usage();
        }
    };
    if let Err(error) = std::fs::create_dir_all(&args.out) {
        eprintln!("cannot create --out {}: {error}", args.out.display());
        process::exit(1);
    }
    let transcript_path = args.out.join("transcript.json");
    if transcript_path.exists() {
        eprintln!(
            "refusing to overwrite existing evidence: {}",
            transcript_path.display()
        );
        process::exit(1);
    }

    let mut transcript = Transcript {
        claim: transcript::claim_line(args.profile.name(), "unknown"),
        profile: args.profile.name().to_string(),
        url: args.url.clone(),
        genesis_hash: "unknown".to_string(),
        network: "unknown".to_string(),
        program_id: args.program_id.clone(),
        program_owner: "unknown".to_string(),
        start_slot: 0,
        degree: DEGREE,
        payer: String::new(),
        identities: BTreeMap::new(),
        addresses: BTreeMap::new(),
        steps: Vec::new(),
        boundaries: Vec::new(),
        outcome: "INCOMPLETE".to_string(),
    };

    let result = run(&args, &mut transcript);
    match &result {
        Ok(()) => transcript.outcome = "PASS".to_string(),
        Err(error) => transcript.outcome = format!("FAIL: {error}"),
    }
    if let Err(error) = transcript.write(&transcript_path) {
        eprintln!("transcript write failed: {error}");
    }
    match result {
        Ok(()) => {
            println!(
                "PASS: {} steps confirmed on {} ({}); transcript at {}",
                transcript.steps.len(),
                transcript.network,
                transcript.claim,
                transcript_path.display()
            );
        }
        Err(error) => {
            eprintln!("FAIL: {error}");
            process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(tokens: &[&str]) -> std::result::Result<Args, String> {
        let owned: Vec<String> = tokens.iter().map(ToString::to_string).collect();
        parse_args(&owned)
    }

    #[test]
    fn arguments_parse_with_the_devnet_default_url() {
        let args = parse(&[
            "--program-id",
            "3SLhMAFm2fXZsqwtTDoDQCQBALBqAEu79N11AySHY2jG",
            "--payer",
            "payer.json",
            "--profile",
            "default",
            "--out",
            "out",
        ])
        .expect("arguments parse");
        assert_eq!(args.url, "https://api.devnet.solana.com");
        assert_eq!(args.profile, Profile::Default);
        assert_eq!(args.throttle_ms, 400);
    }

    #[test]
    fn missing_required_flags_are_refused() {
        assert!(parse(&["--payer", "p.json", "--profile", "mock", "--out", "o"]).is_err());
        assert!(parse(&["--program-id", "x", "--profile", "mock", "--out", "o"]).is_err());
        assert!(parse(&["--program-id", "x", "--payer", "p", "--out", "o"]).is_err());
        assert!(parse(&["--program-id", "x", "--payer", "p", "--profile", "mock"]).is_err());
        assert!(parse(&["--program-id", "x", "--payer", "p", "--profile", "prod", "--out", "o"])
            .is_err());
        assert!(parse(&["--unknown"]).is_err());
    }

    #[test]
    fn public_devnet_and_loopback_genesis_are_classified_fail_closed() {
        assert_eq!(
            classify_network(rpc::PUBLIC_DEVNET_RPC, rpc::DEVNET_GENESIS).unwrap(),
            "devnet"
        );
        assert!(classify_network(rpc::PUBLIC_DEVNET_RPC, "wrong-genesis").is_err());
        assert_eq!(
            classify_network("http://127.0.0.1:8899", "local-genesis").unwrap(),
            "loopback-or-other"
        );
        assert!(classify_network("http://127.0.0.1:8899", rpc::MAINNET_GENESIS).is_err());
    }

    #[test]
    fn artifact_step_names_round_trip() {
        assert_eq!(
            artifact_step("policy-artifact-begin"),
            Some((ArtifactRoute::Policy, ArtifactAction::Begin))
        );
        assert_eq!(
            artifact_step("terms-artifact-write-384"),
            Some((ArtifactRoute::Terms, ArtifactAction::Write(384)))
        );
        assert_eq!(
            artifact_step("grid-artifact-seal"),
            Some((ArtifactRoute::Grid, ArtifactAction::Seal))
        );
        assert_eq!(artifact_step("create-market"), None);
        assert_eq!(artifact_step("policy-artifact-write-x"), None);
    }

    #[test]
    fn refusal_errors_extract_index_and_custom_code() {
        let status = serde_json::json!({
            "err": {"InstructionError": [1, {"Custom": 121}]}
        });
        assert_eq!(refusal_error(&status), Some((1, 121)));
        let unrelated = serde_json::json!({
            "err": {"InstructionError": [1, "ComputationalBudgetExceeded"]}
        });
        assert_eq!(refusal_error(&unrelated), None);
        assert_eq!(refusal_error(&serde_json::json!({"err": null})), None);
    }

    #[test]
    fn every_planned_step_has_a_builder_shape() {
        /* Names the executor must recognize: anything outside this set (and
         * the artifact pattern) would abort mid-campaign on devnet.  Catch it
         * offline instead. */
        for profile in [Profile::Default, Profile::Mock] {
            for step in steps::step_table(profile) {
                let name = step.name.as_str();
                let known = artifact_step(name).is_some()
                    || [
                        "fund-actor",
                        "create-collateral-mint",
                        "create-actor-collateral-token",
                        "create-bearer-collateral-token",
                        "mint-collateral-and-freeze",
                        "init-realm",
                        "init-profile",
                        "create-market",
                        "init-source-spec-refused",
                        "init-source-archive-refused",
                        "endow-refused-no-spec",
                    ]
                    .contains(&name);
                assert!(known, "no builder for step {name}");
            }
        }
    }

    #[test]
    #[allow(clippy::assertions_on_constants)]
    fn funding_constants_leave_headroom() {
        assert!(PAYER_MINIMUM > ACTOR_FUNDING);
        /* The on-chain artifact lifetime bounds are 8 and 432 000 slots. */
        assert!(ARTIFACT_LIFETIME_SLOTS >= 8);
        assert!(ARTIFACT_LIFETIME_SLOTS <= 432_000);
    }
}
