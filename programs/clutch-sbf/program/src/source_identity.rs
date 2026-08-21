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

use crate::instructions_sysvar::PostAbiPositionsV1;
use crate::loader_state::UPGRADEABLE_LOADER_ID;

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
    /// Account-meta positions of the reviewed receiver-post ABI.
    pub post_abi: PostAbiPositionsV1,
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
/// account to exist at [`RECEIVER_PROGRAM`]. That address is
/// `find_program_address(&[b"dc-r2-fixture-receiver"], &UPGRADEABLE_LOADER_ID)`
/// — a program-derived address, hence off the ed25519 curve, hence no private
/// key exists for it. Deploying a program requires the program account to sign
/// its own `DeployWithMaxDataLen`, so no party — including us — can create an
/// executable account there on any cluster. The remaining two addresses follow
/// the *real* derivations a genuine deployment would use
/// ([`RECEIVER_PROGRAMDATA`] is the loader's own ProgramData derivation,
/// [`RECEIVER_CONFIG`] the receiver-owned `config` PDA), so the fixture is
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
    use super::{PostAbiPositionsV1, PullReleaseV2, CLOCK_SYSVAR_ID, UPGRADEABLE_LOADER_ID};

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
    pub const SOURCE_ADAPTER_VERSION: u32 = 2;

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

    /// Earliest Clock time at which the fixture release may be consumed.
    ///
    /// 2023-11-14T22:13:20Z. Bank tests and the validator walk drive the Clock
    /// well past it; the constant exists so the activation gate is exercised
    /// rather than trivially true, and
    /// `r2_pull_hostile.rs::pre_activation_clock_refuses` drives it backwards.
    pub const ACTIVATION_UNIX_TIMESTAMP: i64 = 1_700_000_000;

    /// Account-meta positions the fixture receiver's post instruction uses.
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
    pub const POST_ABI: PostAbiPositionsV1 = PostAbiPositionsV1 {
        config: 2,
        update_account: 4,
        write_authority: 6,
    };

    /// The compiled fixture release triple's identity.
    pub const RELEASE: PullReleaseV2 = PullReleaseV2 {
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
    use super::PostAbiPositionsV1;

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
    pub const POST_ABI: Option<PostAbiPositionsV1> = None;
}

/// Every release compiled into this ELF, in registry order.
///
/// The registry is inert data matched by byte equality, never a negotiation:
/// `instructions::source_ingest_v2::select_release` admits a spec only
/// when one entry matches every identity field exactly. Adding a row is a new
/// ELF identity and a full reseal cycle by construction
/// (`R2_PULL_PROMOTION_PLAN.md` §4 item 3).
pub const REGISTERED_RELEASES: &[PullReleaseV2] = &[fixture::RELEASE];

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
        assert_eq!(REGISTERED_RELEASES.len(), 1);
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
            fixture::PROVIDER_FEED_ID,
            fixture::BASE_ASSET_ID,
            fixture::QUOTE_ASSET_ID,
        ];
        for (at, left) in identities.iter().enumerate() {
            assert_ne!(*left, [0_u8; 32], "identity {at} is zero");
            for right in &identities[at + 1..] {
                assert_ne!(left, right);
            }
        }
    }

    #[test]
    fn the_post_abi_positions_do_not_alias() {
        let abi = fixture::POST_ABI;
        assert_ne!(abi.config, abi.update_account);
        assert_ne!(abi.config, abi.write_authority);
        assert_ne!(abi.update_account, abi.write_authority);
    }
}
