//! **The one-const boundary.** Every identity byte the R2 pull profile pins.
//!
//! This module exists so that the answer to "what does this ELF trust as a
//! price source?" is one file, readable end to end, with no identity byte
//! reachable from anywhere else in the runtime. Two audiences depend on that:
//! the E2 freeze act, which fills the [`mainnet`] block in, and every reader
//! who needs to see that the shipped default artifact trusts a *fabricated*
//! identity that cannot exist on any cluster.
//!
//! ## The two blocks
//!
//! * [`fixture`] — the fabricated Pyth-shaped identity used by bank tests and
//!   the local-validator walk. It is compiled into the **default** ELF and it
//!   is what retires `SourceReleaseUnavailable` (`0x79`) for exactly one spec.
//!   Every one of its account addresses is a program-derived address, so no
//!   private key exists for any of them and no account can be created at them
//!   on a real cluster by anyone. See [`fixture`]'s own documentation for the
//!   full unreachability argument, which
//!   `fixture_addresses_are_program_derived_and_therefore_unreachable` and
//!   `programs/clutch-sbf/svm-tests/tests/r2_pull_identity.rs` check.
//! * [`mainnet`] — the production pins. **Every value is `None`/absent and the
//!   production release is not registered.** Filling this block in is the E2
//!   freeze act of `docs/implementation/R2_PHASE0_RUNBOOK.md` §4.3, and it is
//!   reserved to ember (`R2_PULL_PROMOTION_PLAN.md` §5, last bullet).
//!
//! ## The E2 checklist, as code
//!
//! `R2_PHASE0_RUNBOOK.md` §4.3 enumerates the Phase-1 collection. Each item
//! maps to exactly one `TODO-ember` marker below:
//!
//! | runbook item | constant here |
//! | --- | --- |
//! | 0 — name the cluster | [`mainnet::CLUSTER`] |
//! | 1 — receiver program identity, ProgramData key, decoded deployment slot | [`mainnet::RECEIVER_PROGRAM`], [`mainnet::RECEIVER_PROGRAMDATA`], [`mainnet::PROGRAMDATA_DEPLOYMENT_SLOT`] |
//! | 2 — `Config` full-body SHA-256 | [`mainnet::CONFIG_DIGEST`] (with [`mainnet::RECEIVER_CONFIG`]) |
//! | 3 — pin the SDK/source release | [`mainnet::PARSER_ID`], [`mainnet::PARSER_VERSION`] |
//! | 4 — `activation_unix_timestamp` at or after the cutover | [`mainnet::ACTIVATION_UNIX_TIMESTAMP`] |
//! | 5 — re-verify the 134-byte `PriceUpdateV2` layout and discriminator | [`crate::pyth_receiver`] (layout) and [`mainnet::POST_ABI`] (the post ABI half of gate 4) |
//! | 6 — release dossier | prose, `research/source-profile-v1/PROVENANCE.md` |
//!
//! Nothing here may be filled in from memory, from an SDK read, or from a
//! block explorer. The runbook's §4.2 collection commands against a named
//! cluster at `finalized` commitment are the only admissible source, and the
//! ProgramData body must be decoded with [`crate::loader_state`] rather than
//! by eye — its stale-authority finding is exactly why.

use crate::instructions_sysvar::{
    PostAbiPositionsV1, PostAbiV2, META_FLAG_IS_SIGNER, META_FLAG_IS_WRITABLE,
};
use crate::loader_state::UPGRADEABLE_LOADER_ID;

#[cfg(all(
    feature = "non-production-mock-source",
    feature = "non-production-real-pyth-lab"
))]
compile_error!(
    "non-production-mock-source and non-production-real-pyth-lab are distinct ELF identities"
);

/// Canonical Clock sysvar address, `SysvarC1ock11111111111111111111111111111111`.
///
/// Cross-checked against the runtime's own pin by
/// `clock_sysvar_matches_the_runtime_pin`; the value is not independent
/// identity, it is the same sysvar `instructions::artifact` already enforces on
/// every archive mutation.
pub const CLOCK_SYSVAR_ID: [u8; 32] = [
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
];

/// Compile-time identity of one registered pull release.
///
/// This is the "one compiled (parser, authenticator, spec-predicate) triple
/// per release, selected by spec generation + exact predicate match, never by
/// caller data" that `R2_PULL_PROMOTION_PLAN.md` P0.9 requires. Every field is
/// matched for byte equality against the immutable SourceSpec v2 body before
/// any state is written; none is ever read from caller instruction data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PullReleaseV2 {
    /// SHA-256 identity of the complete canonical SourceSpec v2 body approved
    /// for this compiled release row.  A caller-selected spec that merely
    /// names the same adapter/parser/program is not a registered release.
    pub registered_spec_id: [u8; 32],
    /// Digest naming the reviewed adapter implementation.
    pub source_adapter_id: [u8; 32],
    /// Version of that adapter, bound by Terms as `source_version`.
    pub source_adapter_version: u32,
    /// Closed parser-registry identifier.
    pub parser_id: u16,
    /// Version of the selected parser.
    pub parser_version: u16,
    /// Receiver program that must own every admitted update account.
    pub receiver_program: [u8; 32],
    /// Loader that must own both the receiver program and its ProgramData.
    pub upgradeable_loader: [u8; 32],
    /// Canonical Clock sysvar address.
    pub clock_sysvar: [u8; 32],
    /// Earliest Clock time at which this identity generation may be consumed.
    pub activation_unix_timestamp: i64,
    /// Exact reviewed receiver-post discriminator, shape, effective flags,
    /// and semantic account positions.
    pub post_abi: PostAbiV2,
}

/// The fabricated, provably unreachable Pyth-shaped laboratory identity.
///
/// ## Why a fabricated identity is compiled into the default ELF
///
/// Before this module the default artifact could not run a source lifecycle at
/// all: `release_registered` was `false` unconditionally and every ingestion
/// arm was `#[cfg(not(non-production-mock-source))] => Err`. The only lifecycle
/// evidence that existed came from a *different ELF* — the mock profile — so
/// "the default artifact's ingestion path works" had never been observed even
/// once. Registering one laboratory identity closes that gap on the artifact
/// that actually ships.
///
/// ## Why it admits nothing anywhere
///
/// Value admission through this release requires an executable, loader-owned
/// account to exist at [`fixture::RECEIVER_PROGRAM`]. That address is
/// `find_program_address(&[b"dc-r2-fixture-receiver"], &UPGRADEABLE_LOADER_ID)`
/// — a program-derived address, hence off the ed25519 curve, hence no private
/// key exists for it. Deploying a program requires the program account to sign
/// its own `DeployWithMaxDataLen`, so no party — including us — can create an
/// executable account there on any cluster. The remaining two addresses follow
/// the *real* derivations a genuine deployment would use
/// ([`fixture::RECEIVER_PROGRAMDATA`] is the loader's own ProgramData
/// derivation, [`fixture::RECEIVER_CONFIG`] the receiver-owned `config` PDA), so
/// the fixture is
/// structurally a deployment rather than a stand-in for one.
///
/// A local bank and a local validator can place arbitrary accounts at
/// arbitrary addresses, which is exactly why the lifecycle runs there and
/// nowhere else.
///
/// ## What it is not
///
/// It is **not** the "interim registry entry to get ahead" that
/// `R2_PULL_PROMOTION_PLAN.md` §6 forbids: that prohibition is about admitting
/// a *provisional production Pyth identity* before the E2 freeze, and no Pyth
/// address, feed id, config digest, or deployment slot appears anywhere in this
/// module. It is also not production-provider evidence: a green campaign here
/// says the *runtime path* is correct, never that the *provider* behaves as
/// modeled (`R2_PULL_PROMOTION_PLAN.md` §6, `PYTH_PULL_PROFILE_R2.md` §"Default
/// release STOPs", all of which remain open).
pub mod fixture {
    use super::{
        PostAbiPositionsV1, PostAbiV2, PullReleaseV2, CLOCK_SYSVAR_ID, META_FLAG_IS_SIGNER,
        META_FLAG_IS_WRITABLE, UPGRADEABLE_LOADER_ID,
    };
    use crate::source_v2::crossing::SELECTION_CROSSING_V1;
    use crate::source_v2::spec::{
        SourceSpecFieldsV2, GRID_ORIGIN_UNIX_SECONDS_V1, ORIENTATION_QUOTE_PER_BASE,
    };

    /// `find_program_address(&[b"dc-r2-fixture-receiver"], &UPGRADEABLE_LOADER_ID)`
    /// = `47uaRq4bCPapeush5yXdyCupWdqmaeHWaw6Qm95Xn39E`.
    ///
    /// Off-curve by construction: no keypair, so no deploy, so no executable
    /// account at this address outside a fabricated ledger.
    pub const RECEIVER_PROGRAM: [u8; 32] = [
        46, 87, 254, 112, 20, 67, 78, 124, 15, 41, 17, 106, 158, 191, 60, 102, 250, 244, 200, 105,
        157, 115, 162, 107, 62, 133, 134, 108, 58, 81, 220, 141,
    ];

    /// `find_program_address(&[RECEIVER_PROGRAM], &UPGRADEABLE_LOADER_ID)`
    /// = `7RHTmDdePPxgAA8vsmohS2anG7mp4nVEHaw55RSnJvKK`.
    ///
    /// This is the Upgradeable Loader's own ProgramData derivation, so the
    /// fixture's program/ProgramData link is the real one, not a convention
    /// invented for tests.
    pub const RECEIVER_PROGRAMDATA: [u8; 32] = [
        95, 94, 172, 136, 217, 105, 237, 21, 74, 192, 23, 112, 176, 51, 104, 16, 167, 24, 124, 97,
        206, 80, 161, 249, 181, 88, 208, 191, 243, 42, 53, 178,
    ];

    /// `find_program_address(&[b"config"], &RECEIVER_PROGRAM)`
    /// = `HtPZJttUD8a4zSq6jV2MfjEzJKoAVXiXbRE1FTpSg9Yc`.
    ///
    /// The Pyth receiver's `Config` PDA derivation, applied to the fixture
    /// receiver.
    pub const RECEIVER_CONFIG: [u8; 32] = [
        250, 229, 30, 14, 232, 19, 7, 169, 196, 236, 196, 76, 219, 242, 169, 124, 2, 168, 239, 52,
        200, 39, 59, 66, 0, 134, 230, 199, 210, 131, 98, 209,
    ];

    /// `SHA-256("dragons-clutch/r2/pyth-pull-adapter/v2")`.
    ///
    /// The adapter identity is a property of *our* reviewed adapter, not of
    /// the provider, so the fixture and a future production release may share
    /// it. Terms bind this as `source_adapter_id`.
    pub const SOURCE_ADAPTER_ID: [u8; 32] = [
        230, 200, 209, 164, 221, 29, 71, 15, 111, 113, 199, 147, 207, 39, 226, 95, 167, 217, 69,
        158, 249, 19, 235, 69, 139, 118, 153, 4, 112, 234, 156, 203,
    ];

    /// Version of the reviewed adapter.
    ///
    /// Version 3 binds the exact `post_update` discriminator, seven-account
    /// count, and every signer/writable flag. Version 2 projected only the
    /// Config, update, and write-authority keys and is intentionally absent
    /// from the registry.
    pub const SOURCE_ADAPTER_VERSION: u32 = 3;

    /// Closed parser-registry id of [`crate::pyth_receiver`].
    pub const PARSER_ID: u16 = 2;

    /// Version of that parser.
    pub const PARSER_VERSION: u16 = 1;

    /// `SHA-256("dragons-clutch/r2-fixture/provider-feed-id")`.
    ///
    /// A Pyth feed id is an arbitrary 32-byte identifier rather than an
    /// address, so a domain digest is exactly the right shape for a fabricated
    /// one and collides with no real feed.
    pub const PROVIDER_FEED_ID: [u8; 32] = [
        246, 16, 71, 102, 96, 216, 2, 133, 211, 247, 190, 104, 41, 179, 135, 7, 154, 130, 94, 171,
        68, 28, 7, 30, 107, 86, 224, 39, 229, 51, 27, 54,
    ];

    /// `SHA-256("dragons-clutch/r2-fixture/base-asset")`.
    pub const BASE_ASSET_ID: [u8; 32] = [
        157, 75, 13, 173, 141, 209, 241, 167, 83, 168, 159, 235, 74, 203, 95, 83, 50, 173, 123,
        216, 96, 222, 91, 46, 246, 31, 188, 128, 156, 89, 37, 206,
    ];

    /// `SHA-256("dragons-clutch/r2-fixture/quote-asset")`.
    pub const QUOTE_ASSET_ID: [u8; 32] = [
        47, 211, 222, 112, 33, 4, 127, 30, 119, 150, 198, 108, 143, 173, 221, 163, 241, 47, 53, 56,
        36, 65, 203, 148, 63, 30, 207, 44, 104, 70, 101, 15,
    ];

    /// Deployment slot the fabricated ProgramData body encodes.
    ///
    /// Chosen non-zero because `SourceSpecV2::new`
    /// refuses a zero slot: a zero slot is the shape an uninitialized or
    /// zero-filled ProgramData account would present.
    pub const PROGRAMDATA_DEPLOYMENT_SLOT: u64 = 8_421_504;

    /// SHA-256 of the complete fabricated `Config` body installed by the SVM
    /// campaign.  The campaign independently rebuilds and re-hashes that body.
    pub const CONFIG_DIGEST: [u8; 32] = [
        235, 242, 162, 135, 171, 94, 79, 133, 24, 167, 105, 92, 111, 190, 251, 217, 186, 155, 16,
        15, 176, 166, 50, 204, 36, 26, 202, 123, 233, 2, 83, 163,
    ];

    /// The one complete immutable spec approved by the fixture release row.
    /// This is the semantic preimage of [`REGISTERED_SPEC_ID`]; tests recompute
    /// the digest with the runtime codec so a hand-transcription cannot widen
    /// the registry silently.
    pub const REGISTERED_SPEC_FIELDS: SourceSpecFieldsV2 = SourceSpecFieldsV2 {
        source_adapter_id: SOURCE_ADAPTER_ID,
        source_adapter_version: SOURCE_ADAPTER_VERSION,
        parser_id: PARSER_ID,
        parser_version: PARSER_VERSION,
        receiver_program: RECEIVER_PROGRAM,
        receiver_programdata: RECEIVER_PROGRAMDATA,
        receiver_config: RECEIVER_CONFIG,
        config_digest: CONFIG_DIGEST,
        provider_feed_id: PROVIDER_FEED_ID,
        programdata_deployment_slot: PROGRAMDATA_DEPLOYMENT_SLOT,
        base_asset_id: BASE_ASSET_ID,
        quote_asset_id: QUOTE_ASSET_ID,
        orientation: ORIENTATION_QUOTE_PER_BASE,
        normalized_decimals: 8,
        grid_family_id: 7,
        grid_version: 1,
        grid_origin_unix_seconds: GRID_ORIGIN_UNIX_SECONDS_V1,
        bucket_seconds: 60,
        boundary_grace_seconds: 5,
        max_staleness_slots: 500,
        max_staleness_seconds: 600,
        max_future_seconds: 15,
        max_confidence_atoms: 1_000_000_000_000,
        max_confidence_bps: 500,
        confidence_multiplier: 3,
        selection_rule: SELECTION_CROSSING_V1,
    };

    /// `SourceSpecV2::feed_id(REGISTERED_SPEC_FIELDS)`, frozen as inert
    /// compiled data.  It is deliberately not derived from caller bytes.
    pub const REGISTERED_SPEC_ID: [u8; 32] = [
        60, 239, 23, 76, 104, 88, 14, 183, 129, 35, 96, 223, 112, 106, 180, 102, 27, 46, 243, 213,
        119, 104, 90, 184, 156, 96, 166, 214, 79, 199, 25, 168,
    ];

    /// Earliest Clock time at which the fixture release may be consumed.
    ///
    /// 2023-11-14T22:13:20Z. The archive campaign drives the Clock well past
    /// it; the constant exists so the activation gate is exercised rather than
    /// trivially true, and
    /// `source_v2::auth`'s
    /// `clock_identity_activation_and_both_freshness_envelopes_fail_closed`
    /// drives it forward until the release has not yet activated.
    pub const ACTIVATION_UNIX_TIMESTAMP: i64 = 1_700_000_000;

    /// Exact post contract the fixture receiver instruction uses.
    ///
    /// The fixture receiver stub lays its `post_update` accounts out in this
    /// order deliberately, matching the shape of the reviewed Pyth
    /// `PostUpdate` context (payer, encoded VAA / guardian set, config,
    /// treasury, price-update account, system program, write authority) so
    /// that the *decoder* work — position-addressed meta reads, adjacency,
    /// aliasing refusals — is exercised against a realistic layout.
    ///
    /// These positions are the fixture's own declaration. The production
    /// positions are [`super::mainnet::POST_ABI`] and are an open E2 pin:
    /// runbook §4.3 item 5 re-verifies the deployed ABI, and gate 4's
    /// receiver-post half is explicitly still open (`R2_PHASE0_RUNBOOK.md` §5).
    pub const POST_UPDATE_DISCRIMINATOR: [u8; 8] = [0x85, 0x5f, 0xcf, 0xaf, 0x0b, 0x4f, 0x76, 0x2c];

    /// Exact reviewed Pyth `PostUpdate` instruction contract.
    pub const POST_ABI: PostAbiV2 = PostAbiV2 {
        discriminator: POST_UPDATE_DISCRIMINATOR,
        account_flags: [
            META_FLAG_IS_SIGNER | META_FLAG_IS_WRITABLE, // payer
            0,                                           // encoded VAA
            0,                                           // Config
            META_FLAG_IS_WRITABLE,                       // treasury
            META_FLAG_IS_SIGNER | META_FLAG_IS_WRITABLE, // update
            0,                                           // System Program
            META_FLAG_IS_SIGNER,                         // write authority
        ],
        writable_alias_elevation: Some((0, 6)),
        positions: PostAbiPositionsV1 {
            config: 2,
            update_account: 4,
            write_authority: 6,
        },
    };

    /// The compiled fixture release triple's identity.
    pub const RELEASE: PullReleaseV2 = PullReleaseV2 {
        registered_spec_id: REGISTERED_SPEC_ID,
        source_adapter_id: SOURCE_ADAPTER_ID,
        source_adapter_version: SOURCE_ADAPTER_VERSION,
        parser_id: PARSER_ID,
        parser_version: PARSER_VERSION,
        receiver_program: RECEIVER_PROGRAM,
        upgradeable_loader: UPGRADEABLE_LOADER_ID,
        clock_sysvar: CLOCK_SYSVAR_ID,
        activation_unix_timestamp: ACTIVATION_UNIX_TIMESTAMP,
        post_abi: POST_ABI,
    };
}

/// Byte pins for the **non-production local-real Pyth campaign**.
///
/// This module is absent unless `non-production-real-pyth-lab` is selected.
/// Its receiver and router bytes were captured from the named devnet
/// deployment, then executed entirely inside a disposable local bank. The
/// signed observation used by that campaign is deterministic synthetic test
/// data from local test guardians. A green campaign therefore establishes
/// real deployed program/ABI/cryptographic execution across the provider-to-
/// Clutch transaction seam; it is not evidence of a devnet price, provider
/// availability, production governance, or suitability for deployment.
#[cfg(feature = "non-production-real-pyth-lab")]
pub mod real_pyth_lab {
    use super::{
        PostAbiPositionsV1, PostAbiV2, PullReleaseV2, CLOCK_SYSVAR_ID, META_FLAG_IS_SIGNER,
        META_FLAG_IS_WRITABLE, UPGRADEABLE_LOADER_ID,
    };
    use crate::source_v2::crossing::SELECTION_CROSSING_V1;
    use crate::source_v2::spec::{
        SourceSpecFieldsV2, GRID_ORIGIN_UNIX_SECONDS_V1, ORIENTATION_QUOTE_PER_BASE,
    };

    /// Pyth Solana Receiver program `rec2HH...` from the captured devnet deployment.
    pub const RECEIVER_PROGRAM: [u8; 32] = [
        12, 183, 250, 122, 93, 166, 40, 251, 172, 169, 154, 234, 153, 247, 191, 59, 220, 54, 137,
        104, 96, 42, 191, 65, 77, 78, 139, 165, 103, 187, 176, 191,
    ];

    /// Upgradeable-loader ProgramData account decoded from [`RECEIVER_PROGRAM`].
    pub const RECEIVER_PROGRAMDATA: [u8; 32] = [
        36, 193, 217, 188, 83, 14, 128, 168, 96, 32, 44, 16, 172, 175, 215, 77, 119, 182, 74, 169,
        54, 67, 73, 241, 216, 23, 185, 252, 58, 36, 131, 42,
    ];

    /// Receiver `Config` PDA `H3R4M45...` initialized afresh by the local campaign.
    pub const RECEIVER_CONFIG: [u8; 32] = [
        238, 89, 90, 195, 222, 6, 29, 79, 129, 224, 111, 41, 182, 154, 130, 148, 218, 115, 206, 1,
        195, 236, 196, 54, 206, 145, 180, 165, 98, 100, 91, 13,
    ];

    /// Wormhole Core Bridge router `HDw2E7...` used by the captured receiver.
    pub const ROUTER_PROGRAM: [u8; 32] = [
        241, 11, 10, 220, 120, 104, 244, 85, 102, 87, 169, 5, 247, 20, 69, 206, 236, 66, 7, 172,
        119, 215, 197, 194, 183, 98, 223, 19, 148, 102, 75, 135,
    ];

    /// Upgradeable-loader ProgramData account decoded from [`ROUTER_PROGRAM`].
    pub const ROUTER_PROGRAMDATA: [u8; 32] = [
        129, 50, 201, 239, 143, 229, 66, 230, 102, 107, 79, 207, 240, 58, 197, 139, 124, 134, 144,
        55, 34, 39, 166, 84, 85, 21, 198, 154, 109, 140, 219, 31,
    ];

    /// Upgrade authority decoded from both captured ProgramData accounts.
    pub const UPGRADE_AUTHORITY: [u8; 32] = [
        13, 136, 27, 159, 103, 200, 203, 61, 82, 253, 46, 178, 125, 19, 194, 9, 81, 209, 153, 33,
        43, 117, 2, 29, 85, 236, 191, 94, 24, 59, 140, 219,
    ];

    /// Deployment slots decoded from the original devnet ProgramData bodies.
    pub const RECEIVER_DEPLOYMENT_SLOT: u64 = 460_336_311;
    /// Router deployment slot decoded from the original devnet ProgramData body.
    pub const ROUTER_DEPLOYMENT_SLOT: u64 = 460_336_290;

    /// SHA-256 of the pinned `receiver-config.account` fixture.
    pub const CONFIG_DIGEST: [u8; 32] = [
        5, 3, 140, 247, 7, 175, 206, 172, 61, 241, 170, 231, 53, 176, 150, 52, 74, 214, 57, 80,
        107, 0, 241, 219, 10, 193, 192, 132, 214, 182, 69, 170,
    ];

    /// Synthetic-local feed id embedded in the deterministic signed test VAA.
    pub const PROVIDER_FEED_ID: [u8; 32] = [0x2a; 32];
    /// SHA-256 domain identities used only by the laboratory market.
    pub const BASE_ASSET_ID: [u8; 32] = [
        0x36, 0x71, 0x7b, 0x91, 0xb8, 0xeb, 0x44, 0xd1, 0x84, 0x82, 0x56, 0x6c, 0x89, 0x6f, 0x47,
        0x05, 0xa1, 0xb6, 0x33, 0x71, 0xf9, 0xa6, 0x96, 0xb5, 0x8f, 0x22, 0x88, 0xee, 0x6e, 0xc3,
        0x4a, 0x28,
    ];
    /// SHA-256 domain identity for the laboratory quote asset.
    pub const QUOTE_ASSET_ID: [u8; 32] = [
        0x8c, 0xc8, 0xa5, 0x2c, 0x68, 0x42, 0xe3, 0xd7, 0x45, 0x23, 0xce, 0xa7, 0xd9, 0x03, 0x9f,
        0x9b, 0x41, 0x50, 0x16, 0x09, 0x87, 0x8f, 0x20, 0x78, 0x69, 0x7e, 0xff, 0x05, 0x67, 0x6d,
        0xaf, 0xe4,
    ];

    /// The one complete immutable spec approved by the local-real laboratory
    /// row.  This row remains absent from the default ELF.
    pub const REGISTERED_SPEC_FIELDS: SourceSpecFieldsV2 = SourceSpecFieldsV2 {
        source_adapter_id: super::fixture::SOURCE_ADAPTER_ID,
        source_adapter_version: super::fixture::SOURCE_ADAPTER_VERSION,
        parser_id: super::fixture::PARSER_ID,
        parser_version: super::fixture::PARSER_VERSION,
        receiver_program: RECEIVER_PROGRAM,
        receiver_programdata: RECEIVER_PROGRAMDATA,
        receiver_config: RECEIVER_CONFIG,
        config_digest: CONFIG_DIGEST,
        provider_feed_id: PROVIDER_FEED_ID,
        programdata_deployment_slot: RECEIVER_DEPLOYMENT_SLOT,
        base_asset_id: BASE_ASSET_ID,
        quote_asset_id: QUOTE_ASSET_ID,
        orientation: ORIENTATION_QUOTE_PER_BASE,
        normalized_decimals: 8,
        grid_family_id: 7,
        grid_version: 1,
        grid_origin_unix_seconds: GRID_ORIGIN_UNIX_SECONDS_V1,
        bucket_seconds: 60,
        boundary_grace_seconds: 5,
        max_staleness_slots: 500,
        max_staleness_seconds: 600,
        max_future_seconds: 15,
        max_confidence_atoms: 1_000_000_000_000,
        max_confidence_bps: 500,
        confidence_multiplier: 3,
        selection_rule: SELECTION_CROSSING_V1,
    };

    /// `SourceSpecV2::feed_id(REGISTERED_SPEC_FIELDS)`.
    pub const REGISTERED_SPEC_ID: [u8; 32] = [
        124, 84, 184, 220, 56, 90, 148, 252, 210, 121, 131, 22, 54, 169, 151, 81, 69, 50, 6, 135,
        236, 107, 31, 204, 31, 212, 74, 117, 179, 89, 106, 241,
    ];

    /// `post_update` discriminator executed against the pinned receiver ELF.
    pub const POST_UPDATE_DISCRIMINATOR: [u8; 8] = [0x85, 0x5f, 0xcf, 0xaf, 0x0b, 0x4f, 0x76, 0x2c];

    /// Exact seven-account ABI executed against the pinned receiver ELF.
    ///
    /// Equal bytes do not make the fabricated writer's declaration evidence
    /// for a deployed provider program, so this release owns its ABI constant.
    pub const POST_ABI: PostAbiV2 = PostAbiV2 {
        discriminator: POST_UPDATE_DISCRIMINATOR,
        account_flags: [
            META_FLAG_IS_SIGNER | META_FLAG_IS_WRITABLE,
            0,
            0,
            META_FLAG_IS_WRITABLE,
            META_FLAG_IS_SIGNER | META_FLAG_IS_WRITABLE,
            0,
            META_FLAG_IS_SIGNER,
        ],
        writable_alias_elevation: Some((0, 6)),
        positions: PostAbiPositionsV1 {
            config: 2,
            update_account: 4,
            write_authority: 6,
        },
    };

    /// The adapter/parser release compiled only into the local-real test ELF.
    pub const RELEASE: PullReleaseV2 = PullReleaseV2 {
        registered_spec_id: REGISTERED_SPEC_ID,
        source_adapter_id: super::fixture::SOURCE_ADAPTER_ID,
        source_adapter_version: super::fixture::SOURCE_ADAPTER_VERSION,
        parser_id: super::fixture::PARSER_ID,
        parser_version: super::fixture::PARSER_VERSION,
        receiver_program: RECEIVER_PROGRAM,
        upgradeable_loader: UPGRADEABLE_LOADER_ID,
        clock_sysvar: CLOCK_SYSVAR_ID,
        activation_unix_timestamp: 1_787_000_000,
        post_abi: POST_ABI,
    };
}

/// The production pins. **Empty, by design, until the E2 freeze act.**
///
/// Every constant below is deliberately absent rather than provisional. A
/// placeholder address would be an identity byte, and
/// `R2_PHASE0_RUNBOOK.md` §6 forbids an identity-byte pin of any kind before
/// the freeze; a *remembered* address would be worse still, because the
/// dossier requires a primary source with a retrieval date and a raw-file
/// SHA-256 (`R2_PHASE0_RUNBOOK.md` §4.4) and a recalled value has neither.
///
/// [`REGISTERED_RELEASES`] therefore carries no production entry, and every
/// production-shaped SourceSpec still refuses `0x79`.
pub mod mainnet {
    use super::PostAbiV2;

    /// Runbook §4.3 item 0 — devnet or mainnet-beta. A frozen SourceSpec v2 is
    /// cluster-specific by construction: it binds a ProgramData deployment
    /// slot and a per-cluster `Config` account, so a second cluster is a
    /// second freeze act and a second feed identity, never a reuse.
    ///
    /// TODO-ember(E2 line 0): name the cluster before the collection runs.
    pub const CLUSTER: Option<&str> = None;

    /// Runbook §4.3 item 1 — the post-cutover receiver program id.
    ///
    /// TODO-ember(E2 item 1): transcribe from the digest-pinned
    /// `address.ts` (runbook §4.1) and confirm the on-cluster account.
    pub const RECEIVER_PROGRAM: Option<[u8; 32]> = None;

    /// Runbook §4.3 item 1 — the ProgramData key, *decoded* from the receiver
    /// program account with [`crate::loader_state::decode_program_state`].
    ///
    /// TODO-ember(E2 item 1): decode, do not assume the PDA derivation.
    pub const RECEIVER_PROGRAMDATA: Option<[u8; 32]> = None;

    /// Runbook §4.3 item 1 — the deployment slot decoded from ProgramData.
    ///
    /// An in-place upgrade rewrites ProgramData, so this is the *cutover's*
    /// slot. Decode with [`crate::loader_state::decode_programdata_state`];
    /// its finding 1 (bytes `[13..45)` of a revoked-authority account still
    /// hold the previous authority) is why eyeballing the body is refused.
    ///
    /// TODO-ember(E2 item 1).
    pub const PROGRAMDATA_DEPLOYMENT_SLOT: Option<u64> = None;

    /// Runbook §4.3 item 2 — the receiver `Config` PDA key.
    ///
    /// TODO-ember(E2 item 2).
    pub const RECEIVER_CONFIG: Option<[u8; 32]> = None;

    /// Runbook §4.3 item 2 — SHA-256 of the **complete** `Config` account
    /// body. This is the governance-generation pin: any later governance
    /// change (fee, `valid_data_sources`, router address, `minimum_signatures`)
    /// is a new feed generation by construction, and there is no field-level
    /// exception.
    ///
    /// Safe to pin only once runbook §3.2's named stability span has closed
    /// with every sample carrying an identical digest.
    ///
    /// TODO-ember(E2 item 2).
    pub const CONFIG_DIGEST: Option<[u8; 32]> = None;

    /// The provider feed id (Pyth's 32-byte `feed_id`) for the pinned market.
    ///
    /// TODO-ember(E2): one release per feed id; a second feed is a second
    /// spec, not a parameter.
    pub const PROVIDER_FEED_ID: Option<[u8; 32]> = None;

    /// Runbook §4.3 item 3 — the parser id/version matching the pinned SDK
    /// release. The named STOP applies: the migration guide said 1.2.0 and the
    /// SDK manifest said 2.0.0, and if that discrepancy has not resolved,
    /// record both and **stop** rather than choosing one.
    ///
    /// TODO-ember(E2 item 3).
    pub const PARSER_ID: Option<u16> = None;

    /// Runbook §4.3 item 3, the version half.
    ///
    /// TODO-ember(E2 item 3).
    pub const PARSER_VERSION: Option<u16> = None;

    /// Runbook §4.3 item 4 — at or after the observed cutover instant.
    ///
    /// TODO-ember(E2 item 4).
    pub const ACTIVATION_UNIX_TIMESTAMP: Option<i64> = None;

    /// Runbook §4.3 item 5, the post-ABI half of E3 gate 4.
    ///
    /// The 134-byte `PriceUpdateV2` layout itself is implemented and tested in
    /// [`crate::pyth_receiver`]; what is *not* pinned is which account-meta
    /// positions the deployed post instruction uses for config, price-update
    /// account, and write authority. Reading them off an SDK context struct is
    /// not a pin — the deployed program is.
    ///
    /// TODO-ember(E2 item 5): re-verify against the deployed post-cutover
    /// program, not against the SDK source.
    pub const POST_ABI: Option<PostAbiV2> = None;
}

/// Select the compiled release a v2 spec names, if this ELF carries one.
///
/// **This is the `0x79` decision.** `SourceReleaseUnavailable` is what a caller
/// sees when this returns `None`, and the whole content of "the registry is
/// closed" is that the match below is byte equality against inert compiled
/// data. Nothing is negotiated, nothing is derived from caller bytes, and no
/// field is matched loosely:
///
/// * `registered_spec_id` — the digest of every canonical SourceSpec byte;
/// * adapter/parser/program fields — redundant operational hardening against
///   an incorrectly transcribed release row.
///
/// The compiled approval and the live account join are distinct checks against
/// one immutable SourceSpec identity.  The registry decides which complete
/// spec this ELF approves; [`crate::source_v2::auth::authenticate_pull_update_v2`]
/// independently proves that the presented ProgramData, Config, provider feed,
/// and update evidence satisfy that approved spec.
///
/// A `Some` here narrows the refusal boundary; it never removes it. Every spec
/// that is not this exact release still refuses, which is the shape
/// `R2_PULL_PROMOTION_PLAN.md` §4 item 2 requires of the flip.
pub fn select_release(spec: crate::source_v2::spec::SourceSpecV2) -> Option<PullReleaseV2> {
    let fields = spec.fields();
    let spec_id = spec.feed_id();
    REGISTERED_RELEASES
        .iter()
        .find(|release| {
            release.registered_spec_id == spec_id
                && release.source_adapter_id == fields.source_adapter_id
                && release.source_adapter_version == fields.source_adapter_version
                && release.parser_id == fields.parser_id
                && release.parser_version == fields.parser_version
                && release.receiver_program == fields.receiver_program
        })
        .copied()
}

/// Every release compiled into this ELF, in registry order.
///
/// The registry is inert data matched by byte equality, never a negotiation:
/// [`select_release`] admits a spec only when one entry matches every identity
/// field exactly, and it is asked at all four v2 source routes, at the
/// collateral boundary, and again at resolution. Adding a row is a new ELF
/// identity and a full reseal cycle by construction
/// (`R2_PULL_PROMOTION_PLAN.md` §4 item 3).
#[cfg(not(feature = "non-production-real-pyth-lab"))]
pub const REGISTERED_RELEASES: &[PullReleaseV2] = &[fixture::RELEASE];

/// The extra row exists only in an unmistakably non-production laboratory ELF.
#[cfg(feature = "non-production-real-pyth-lab")]
pub const REGISTERED_RELEASES: &[PullReleaseV2] = &[fixture::RELEASE, real_pyth_lab::RELEASE];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_sysvar_matches_the_runtime_pin() {
        assert_eq!(
            CLOCK_SYSVAR_ID,
            crate::instructions::artifact::CLOCK_SYSVAR_ID.to_bytes()
        );
    }

    #[test]
    fn the_production_registry_is_empty() {
        // Every mainnet pin is absent, so no production-shaped spec can be
        // registered.  This test is the mechanical form of the E2 gate: it
        // fails the moment a pin is filled in without a registry row, and it
        // must be updated deliberately as part of the freeze act.
        assert!(mainnet::CLUSTER.is_none());
        assert!(mainnet::RECEIVER_PROGRAM.is_none());
        assert!(mainnet::RECEIVER_PROGRAMDATA.is_none());
        assert!(mainnet::PROGRAMDATA_DEPLOYMENT_SLOT.is_none());
        assert!(mainnet::RECEIVER_CONFIG.is_none());
        assert!(mainnet::CONFIG_DIGEST.is_none());
        assert!(mainnet::PROVIDER_FEED_ID.is_none());
        assert!(mainnet::PARSER_ID.is_none());
        assert!(mainnet::PARSER_VERSION.is_none());
        assert!(mainnet::ACTIVATION_UNIX_TIMESTAMP.is_none());
        assert!(mainnet::POST_ABI.is_none());
        #[cfg(not(feature = "non-production-real-pyth-lab"))]
        assert_eq!(REGISTERED_RELEASES.len(), 1);
        #[cfg(feature = "non-production-real-pyth-lab")]
        assert_eq!(REGISTERED_RELEASES.len(), 2);
        assert_eq!(REGISTERED_RELEASES[0], fixture::RELEASE);
    }

    #[test]
    fn the_fixture_release_names_the_loader_and_clock_it_authenticates_against() {
        assert_eq!(fixture::RELEASE.upgradeable_loader, UPGRADEABLE_LOADER_ID);
        assert_eq!(fixture::RELEASE.clock_sysvar, CLOCK_SYSVAR_ID);
    }

    #[test]
    fn fixture_identities_are_pairwise_distinct_and_nonzero() {
        let identities = [
            fixture::RECEIVER_PROGRAM,
            fixture::RECEIVER_PROGRAMDATA,
            fixture::RECEIVER_CONFIG,
            fixture::SOURCE_ADAPTER_ID,
            fixture::CONFIG_DIGEST,
            fixture::PROVIDER_FEED_ID,
            fixture::BASE_ASSET_ID,
            fixture::QUOTE_ASSET_ID,
            fixture::REGISTERED_SPEC_ID,
        ];
        for (at, left) in identities.iter().enumerate() {
            assert_ne!(*left, [0_u8; 32], "identity {at} is zero");
            for right in &identities[at + 1..] {
                assert_ne!(left, right);
            }
        }
    }

    #[cfg(feature = "non-production-real-pyth-lab")]
    #[test]
    fn laboratory_release_spec_ids_are_nonzero_unique_and_recomputed() {
        use crate::source_v2::spec::SourceSpecV2;

        let fixture_spec = SourceSpecV2::new(fixture::REGISTERED_SPEC_FIELDS).unwrap();
        let real_spec = SourceSpecV2::new(real_pyth_lab::REGISTERED_SPEC_FIELDS).unwrap();
        assert_eq!(fixture_spec.feed_id(), fixture::REGISTERED_SPEC_ID);
        assert_eq!(real_spec.feed_id(), real_pyth_lab::REGISTERED_SPEC_ID);
        assert_ne!(fixture::REGISTERED_SPEC_ID, [0_u8; 32]);
        assert_ne!(real_pyth_lab::REGISTERED_SPEC_ID, [0_u8; 32]);
        assert_ne!(
            fixture::REGISTERED_SPEC_ID,
            real_pyth_lab::REGISTERED_SPEC_ID
        );
    }

    /// A spec naming the fixture release exactly.
    fn fixture_spec_fields() -> crate::source_v2::spec::SourceSpecFieldsV2 {
        fixture::REGISTERED_SPEC_FIELDS
    }

    #[test]
    fn the_registry_admits_exactly_the_compiled_release() {
        use crate::source_v2::spec::{SourceSpecFieldsV2, SourceSpecV2};

        let admitted = SourceSpecV2::new(fixture_spec_fields()).expect("valid spec");
        assert_eq!(admitted.feed_id(), fixture::REGISTERED_SPEC_ID);
        assert_eq!(select_release(admitted), Some(fixture::RELEASE));

        // Every structurally flexible field is load-bearing: change any one
        // while preserving a valid canonical SourceSpec and `0x79` stands.
        for mutate in [
            (|c: &mut SourceSpecFieldsV2| c.source_adapter_id[0] ^= 1)
                as fn(&mut SourceSpecFieldsV2),
            |c| c.source_adapter_version += 1,
            |c| c.parser_id += 1,
            |c| c.parser_version += 1,
            |c| c.receiver_program[0] ^= 1,
            |c| c.receiver_programdata[0] ^= 1,
            |c| c.receiver_config[0] ^= 1,
            |c| c.config_digest[0] ^= 1,
            |c| c.provider_feed_id[0] ^= 1,
            |c| c.programdata_deployment_slot += 1,
            |c| c.base_asset_id[0] ^= 1,
            |c| c.quote_asset_id[0] ^= 1,
            |c| c.normalized_decimals += 1,
            |c| c.grid_family_id += 1,
            |c| c.grid_version += 1,
            |c| c.bucket_seconds += 1,
            |c| c.boundary_grace_seconds += 1,
            |c| c.max_staleness_slots += 1,
            |c| c.max_staleness_seconds += 1,
            |c| c.max_future_seconds += 1,
            |c| c.max_confidence_atoms += 1,
            |c| c.max_confidence_bps += 1,
            |c| c.confidence_multiplier += 1,
        ] {
            let mut case = fixture_spec_fields();
            mutate(&mut case);
            let spec = SourceSpecV2::new(case).expect("still structurally valid");
            assert_ne!(spec.feed_id(), fixture::REGISTERED_SPEC_ID);
            assert_eq!(select_release(spec), None);
        }
    }

    #[test]
    fn the_post_abi_positions_do_not_alias() {
        let abi = fixture::POST_ABI;
        assert_ne!(abi.positions.config, abi.positions.update_account);
        assert_ne!(abi.positions.config, abi.positions.write_authority);
        assert_ne!(abi.positions.update_account, abi.positions.write_authority);
        assert_eq!(abi.discriminator, fixture::POST_UPDATE_DISCRIMINATOR);
        assert_eq!(abi.account_flags.len(), 7);
    }
}
