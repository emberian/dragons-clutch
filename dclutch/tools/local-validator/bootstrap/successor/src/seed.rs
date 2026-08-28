//! Deterministic campaign keypairs. **TEST-ONLY, LOOPBACK-ONLY.**
//!
//! # Why this exists
//!
//! Every signing key this campaign generates is normally a fresh
//! `Keypair::new()`. That is correct for a supervisor that must never leave a
//! usable key behind, and it is also the sole source of the compute-unit noise
//! `tools/gauntlet/CU_BUDGETS.md` documents: a different public key changes how
//! many iterations `find_program_address` needs to find an off-curve bump, and
//! every extra iteration is one `sol_create_program_address` syscall at
//! **1,500 CU**. The measured tier-1 band is 58,494 CU on `DCLTGMF1` and 79,500
//! on `DCLTPCB1` *within a single campaign*, which is wide enough to hide a
//! 30,000-CU regression.
//!
//! With a seed the band is zero, so the gauntlet's budget tolerances can drop to
//! their 15,000 floor and a regression smaller than the old band becomes a red
//! row on every run.
//!
//! # The derivation
//!
//! A campaign is strictly sequential and single-threaded: it submits one
//! transaction, waits for it to finalize, and derives the next from the result.
//! So "the n-th key this campaign asked for under this role name" is itself a
//! deterministic coordinate, and that is the whole index.
//!
//! ```text
//! index    = keys already issued for this role in this campaign, u32 little-endian
//! material = SHA-256( DOMAIN || 0x00 || seed[32] || 0x00 || role || 0x00 || index )
//! keypair  = the ed25519 keypair whose 32-byte secret seed is `material`
//! ```
//!
//! `DOMAIN` is [`KEYPAIR_SEED_DOMAIN_V1`]. The `0x00` separators keep the
//! concatenation unambiguous, so no two (seed, role, index) triples can collide
//! by a role name absorbing part of its neighbour. Every 32-byte string is a
//! valid ed25519 secret seed, so the derivation is total: no rejection sampling,
//! no retry loop, no way for a seed to produce fewer keys than it was asked for.
//!
//! Role names are part of the derivation and therefore part of its contract.
//! Renaming one moves every key under it and moves the compute-unit numbers with
//! it; that is a re-pin of `CU_BUDGETS.json`, not a refactor.
//!
//! # The safety gate
//!
//! A deterministic key on a public cluster is a **catastrophic footgun**: the
//! seed is on a command line, in a shell history, and in a checked-in script, so
//! anyone who reads any of those can sign as every account the campaign funded.
//! [`KeyForge::parse`] therefore refuses the flag outright unless the campaign's
//! RPC endpoint is loopback, and it refuses *before* deriving anything.

use std::{cell::RefCell, collections::BTreeMap};

use sha2::{Digest as _, Sha256};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer as _},
};

use crate::{
    Error, Result, cluster::seeded_keys_admissible as cluster_admits_seeded_keys, plan::hex,
};

/// Domain separator for every seeded campaign key.
///
/// Versioned: a change to the derivation gets a new domain rather than silently
/// moving the keys — and therefore the compute-unit numbers — under the same
/// name.
pub(crate) const KEYPAIR_SEED_DOMAIN_V1: &[u8] =
    b"dclutch/local-successor-bootstrap/keypair-seed/v1";

/// Every role name the campaign issues keys under.
///
/// These strings are part of the derivation, so they are named once here rather
/// than spelled at each call site: a typo at a call site would silently become a
/// different key rather than a compile error, and a rename here moves keys and
/// compute-unit numbers together.
pub(crate) mod role {
    /// The ephemeral Core upgrade authority, and the campaign's fee payer.
    pub(crate) const CORE_UPGRADE_AUTHORITY: &str = "core-upgrade-authority";
    /// The wrong authority every refusal probe signs with.
    pub(crate) const HOSTILE_AUTHORITY: &str = "hostile-authority";
    /// The Token-2022 collateral mint.
    pub(crate) const COLLATERAL_MINT: &str = "collateral-mint";
    /// The raw-atom wallet the collateral is minted into.
    pub(crate) const COLLATERAL_WALLET: &str = "collateral-wallet";
    /// The party the founding principal is refundable to. Issued once per
    /// prestate lane, so the index separates Founding from SourceAbort.
    pub(crate) const FOUNDING_BENEFICIARY: &str = "founding-beneficiary";
    /// The founder whose Position the founding mints. Once per lane.
    pub(crate) const FOUNDING_FOUNDER: &str = "founding-founder";
    /// The rent-capacity witness the projection funds. Once per lane.
    pub(crate) const FOUNDING_PROJECTION_WITNESS: &str = "founding-projection-witness";
    /// The Token-2022 account holding the principal before the Lock. Once per
    /// lane.
    pub(crate) const FOUNDING_SOURCE_FUNDER: &str = "founding-source-funder";
    /// The substituted founder in the hostile cross-request join probe. Never
    /// signs and is never funded; only its public key is used.
    pub(crate) const SUBSTITUTED_FOUNDER: &str = "substituted-founder";
}

/// Domain separator for a persisted per-role key's higher indices.
///
/// Distinct from [`KEYPAIR_SEED_DOMAIN_V1`] so that a persisted campaign and a
/// seeded one can never derive the same key even if somebody's file happened to
/// hold the same 32 bytes as somebody's `--keypair-seed`.
pub(crate) const PERSISTED_KEY_DOMAIN_V1: &[u8] =
    b"dclutch/local-successor-bootstrap/persisted-key/v1";

/// Where a campaign's signing keys come from.
enum KeyOriginV1 {
    /// The default. One fresh key per request, unreproducible by anyone,
    /// including this process a moment later.
    Random,
    /// Test-only. Every key is a pure function of this seed, the role name, and
    /// how many keys that role has already been issued.
    Seeded([u8; 32]),
    /// The driver's origin: one keypair FILE per role, held by the operator.
    ///
    /// # Why index 0 is the file's own key, exactly
    ///
    /// The operator has to *fund* these addresses, and funding happens outside
    /// this tool — `solana transfer`, a faucet, a wallet. So the address that
    /// `solana address -k core-upgrade-authority.json` prints must be the
    /// address this campaign pays fees from, with no derivation in between.
    /// Index 0 is therefore the file's key verbatim.
    ///
    /// # Why higher indices are derived rather than demanded
    ///
    /// A campaign asks a role for its n-th key, and n depends on which lanes
    /// the market path takes. Demanding a file per (role, index) would make the
    /// operator's obligation depend on a control-flow detail they cannot see,
    /// and a missing file would surface halfway through a founding ladder. So
    /// index n > 0 is `SHA-256(DOMAIN || 0 || file-secret || 0 || role || 0 ||
    /// n)`: total, reproducible from a file the operator already holds, and
    /// impossible to run out of.
    ///
    /// # Why this is not the `--keypair-seed` footgun
    ///
    /// `--keypair-seed` is refused off loopback because the seed rides on a
    /// command line, into a shell history, into a checked-in script. A file
    /// path is not the secret; the file is, and it is the same file the
    /// operator already trusts with the funded wallet. What this origin does
    /// owe the evidence is honesty: `private_key_persisted` is TRUE for such a
    /// run, because a key that outlives the process is exactly what it is.
    Persisted(BTreeMap<String, [u8; 32]>),
}

/// A fresh address that exists nowhere, for hostile probes and rollback
/// recipients.
///
/// NOT `Pubkey::new_unique()`: that helper is a deterministic global counter
/// meant for unit tests, so every process draws the same low-counter
/// addresses — and on a public cluster with years of history those addresses
/// EXIST. Measured 2026-08-28: three consecutive devnet founding attempts
/// died at the Found31 rollback check because `recipient_exists=true` for a
/// "unique" recipient, while a fresh local ledger — where every such address
/// is empty by construction — could never catch it. A random keypair's
/// address is fresh with cryptographic certainty on every cluster, which is
/// what a probe that asserts absence actually requires.
pub(crate) fn fresh_probe_address() -> Pubkey {
    Keypair::new().pubkey()
}

/// The campaign's sole source of signing keys.
///
/// Interior mutability rather than `&mut`: the forge is threaded through call
/// paths that already hold `&mut Rpc` and `&mut Vec<TransactionEvidence>`, and a
/// second exclusive borrow would reshape half of `market.rs` for no semantic
/// gain. The campaign is single-threaded, so the `RefCell` is never contended.
pub(crate) struct KeyForge {
    origin: KeyOriginV1,
    issued: RefCell<BTreeMap<&'static str, u32>>,
}

impl KeyForge {
    /// The default: fresh, unreproducible keys.
    pub(crate) fn random() -> Self {
        Self {
            origin: KeyOriginV1::Random,
            issued: RefCell::new(BTreeMap::new()),
        }
    }

    /// Read the `--keypair-seed` flag, enforcing the loopback gate.
    ///
    /// `None` is the default and is always admitted. `Some(seed)` is admitted
    /// only for a loopback RPC origin, and the refusal names the endpoint that
    /// caused it.
    pub(crate) fn parse(keypair_seed: Option<&str>, rpc_url: &str) -> Result<Self> {
        let Some(value) = keypair_seed else {
            return Ok(Self::random());
        };
        // The gate, before anything is derived. `crate::cluster` is the one
        // owner of "which cluster is this and what may happen there"; this is
        // the one owner of "may a seeded key exist against it".
        //
        // The question asked is deliberately `may_use_seeded_keys`, not "did
        // the URL parse as loopback". A devnet origin the operator explicitly
        // acknowledged is an *admitted* origin, and admitting it must not
        // quietly admit reproducible private keys with it — the whole footgun
        // below is that the operator meant to reach a real cluster.
        if !cluster_admits_seeded_keys(rpc_url) {
            return Err(Error::new(format!(
                "--keypair-seed REFUSED for RPC endpoint {rpc_url}. Seeded keypairs are a \
                 TEST-ONLY affordance: the seed is a command-line argument, so every private \
                 key it derives is reproducible by anyone who can read a shell history or a \
                 checked-in script. On any cluster but a loopback one that is a catastrophic \
                 footgun -- funded accounts, mint authorities and upgrade authorities all \
                 signable by a stranger. The flag is admitted only for an RPC origin on \
                 localhost or 127.0.0.1."
            )));
        }
        let seed = crate::plan::hex32(value).map_err(|error| {
            Error::new(format!(
                "--keypair-seed must be 64 lowercase hex characters (32 bytes): {error}"
            ))
        })?;
        Ok(Self {
            origin: KeyOriginV1::Seeded(seed),
            issued: RefCell::new(BTreeMap::new()),
        })
    }

    /// Build a forge from per-role secrets the operator holds on disk.
    ///
    /// `secrets` maps a [`role`] name to the 32-byte ed25519 secret seed read
    /// out of that role's keypair file. Every role the campaign will ask for
    /// must be present: this is checked here, once, against the caller's own
    /// required list, so a missing file is a refusal before the first
    /// transaction rather than a surprise inside a ladder.
    pub(crate) fn persisted(
        secrets: BTreeMap<String, [u8; 32]>,
        required: &[&'static str],
    ) -> Result<Self> {
        let missing = required
            .iter()
            .filter(|role| !secrets.contains_key(**role))
            .copied()
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(Error::new(format!(
                "no keypair was supplied for {}. A campaign against a cluster it did not create \
                 has no way to invent a funded signer, so every role it will sign for must be a \
                 file you hold.",
                missing.join(", ")
            )));
        }
        Ok(Self {
            origin: KeyOriginV1::Persisted(secrets),
            issued: RefCell::new(BTreeMap::new()),
        })
    }

    /// Issue the next keypair for `role`.
    ///
    /// Under a seed this is `SHA-256(DOMAIN || 0 || seed || 0 || role || 0 ||
    /// index)` read as an ed25519 secret seed, where `index` counts prior
    /// issues under the same role. Under a persisted store, index 0 is the
    /// file's own key and index n is the same construction under
    /// [`PERSISTED_KEY_DOMAIN_V1`]. The counter advances in every case, so runs
    /// under all three origins ask for keys in exactly the same order.
    pub(crate) fn keypair(&self, role: &'static str) -> Keypair {
        let index = {
            let mut issued = self.issued.borrow_mut();
            let counter = issued.entry(role).or_insert(0);
            let index = *counter;
            *counter = counter.saturating_add(1);
            index
        };
        match &self.origin {
            KeyOriginV1::Random => Keypair::new(),
            KeyOriginV1::Seeded(seed) => {
                Keypair::new_from_array(derive(KEYPAIR_SEED_DOMAIN_V1, seed, role, index))
            }
            KeyOriginV1::Persisted(secrets) => match secrets.get(role) {
                // Unreachable for any role in the caller's required list, which
                // `persisted` checked. A role outside that list is a campaign
                // path the driver has not been taught to fund, and a fresh
                // unfunded key is the honest answer: every transaction it signs
                // for fails visibly at the cluster rather than being paid for
                // by somebody else's balance.
                None => Keypair::new(),
                Some(secret) if index == 0 => Keypair::new_from_array(*secret),
                Some(secret) => {
                    Keypair::new_from_array(derive(PERSISTED_KEY_DOMAIN_V1, secret, role, index))
                }
            },
        }
    }

    /// The public key the NEXT `keypair(role)` call would return, without
    /// issuing it.
    ///
    /// The founding detector derives WHERE a founding will land before
    /// anything is written, and it must look at exactly the key the executor
    /// will draw. Reading through `keypair` CONSUMES an issuance index, which
    /// shifts the executor onto a different key and turns the detector into
    /// the drift it exists to prevent — measured on the first driven founding
    /// (2026-08-27): the preflight consumed `collateral-mint[0]`, the
    /// campaign founded on `collateral-mint[1]`, and the post-execution
    /// verifier then peeked index 2 and reported the freshly opened Market
    /// absent.
    ///
    /// A random forge is refused: its next key does not exist until drawn,
    /// and a detector that pretended otherwise would be lying about the
    /// future.
    pub(crate) fn peek_pubkey(&self, role: &'static str) -> Result<Pubkey> {
        let index = self.issued.borrow().get(role).copied().unwrap_or(0);
        match &self.origin {
            KeyOriginV1::Random => Err(Error::new(format!(
                "peek_pubkey({role}) on a random forge: an unreproducible forge's next key does \
                 not exist until it is drawn"
            ))),
            KeyOriginV1::Seeded(seed) => {
                Ok(Keypair::new_from_array(derive(KEYPAIR_SEED_DOMAIN_V1, seed, role, index))
                    .pubkey())
            }
            KeyOriginV1::Persisted(secrets) => match secrets.get(role) {
                None => Err(Error::new(format!(
                    "peek_pubkey({role}) outside the persisted role set: the campaign has not \
                     been handed a keypair file for it"
                ))),
                Some(secret) if index == 0 => Ok(Keypair::new_from_array(*secret).pubkey()),
                Some(secret) => Ok(Keypair::new_from_array(derive(
                    PERSISTED_KEY_DOMAIN_V1,
                    secret,
                    role,
                    index,
                ))
                .pubkey()),
            },
        }
    }

    /// How the evidence document must describe this campaign's keys.
    pub(crate) fn derivation_label(&self) -> &'static str {
        match self.origin {
            KeyOriginV1::Random => "random-per-run",
            KeyOriginV1::Seeded(_) => "seeded-deterministic",
            KeyOriginV1::Persisted(_) => "persisted-per-role",
        }
    }

    /// Whether any key this forge issues outlives the process.
    ///
    /// The evidence document's `private_key_persisted` field. It is a separate
    /// claim from `derivation_label`, and a driver run must not be able to
    /// inherit the supervisor's constant `false`.
    pub(crate) fn persists_private_keys(&self) -> bool {
        matches!(self.origin, KeyOriginV1::Persisted(_))
    }

    /// SHA-256 of the seed, or `None` for a random or persisted campaign.
    ///
    /// The digest and not the seed: it identifies which seed produced a run's
    /// compute-unit numbers without the evidence file itself becoming a way to
    /// sign as the campaign. A persisted campaign has no single seed, and
    /// digesting the operator's files would put a fingerprint of a live funded
    /// key into a document meant to be published.
    pub(crate) fn seed_sha256(&self) -> Option<String> {
        match self.origin {
            KeyOriginV1::Random | KeyOriginV1::Persisted(_) => None,
            KeyOriginV1::Seeded(seed) => Some(hex(&Sha256::digest(seed))),
        }
    }
}

/// The one derivation both keyed origins share.
fn derive(domain: &[u8], secret: &[u8; 32], role: &str, index: u32) -> [u8; 32] {
    let mut material = Sha256::new();
    material.update(domain);
    material.update([0]);
    material.update(secret);
    material.update([0]);
    material.update(role.as_bytes());
    material.update([0]);
    material.update(index.to_le_bytes());
    material.finalize().into()
}

#[cfg(test)]
mod tests {
    use solana_sdk::signature::Signer as _;

    use super::*;

    const SEED: &str = "0000000000000000000000000000000000000000000000000000000000000001";
    const LOOPBACK: &str = "http://127.0.0.1:20890/";

    fn seeded() -> KeyForge {
        KeyForge::parse(Some(SEED), LOOPBACK).expect("loopback seed")
    }

    #[test]
    fn the_same_seed_reproduces_every_key_in_order() {
        let first = seeded();
        let second = seeded();
        for role in ["core-upgrade-authority", "collateral-mint"] {
            for _ in 0..3 {
                assert_eq!(
                    first.keypair(role).pubkey(),
                    second.keypair(role).pubkey(),
                    "role {role}"
                );
            }
        }
    }

    #[test]
    fn role_and_index_both_separate_keys() {
        let forge = seeded();
        let first = forge.keypair("collateral-mint").pubkey();
        let second = forge.keypair("collateral-mint").pubkey();
        let other = forge.keypair("collateral-wallet").pubkey();
        assert_ne!(first, second, "the index must advance");
        assert_ne!(first, other, "the role must separate");
        assert_ne!(second, other);
    }

    #[test]
    fn a_different_seed_is_a_different_campaign() {
        let other = KeyForge::parse(
            Some("0000000000000000000000000000000000000000000000000000000000000002"),
            LOOPBACK,
        )
        .expect("loopback seed");
        assert_ne!(
            seeded().keypair("core-upgrade-authority").pubkey(),
            other.keypair("core-upgrade-authority").pubkey()
        );
    }

    #[test]
    fn peeking_never_issues_and_names_exactly_the_next_draw() {
        // Persisted: peek == the file's own key at index 0, twice (no issue),
        // then the draw returns exactly the peeked key, and the next peek
        // names the derived index-1 key the next draw returns.
        let secret = [9_u8; 32];
        let forge = KeyForge::persisted(
            BTreeMap::from([("collateral-mint".to_owned(), secret)]),
            &["collateral-mint"],
        )
        .expect("persisted forge");
        let first_peek = forge.peek_pubkey("collateral-mint").expect("peek");
        let second_peek = forge.peek_pubkey("collateral-mint").expect("peek again");
        assert_eq!(first_peek, second_peek, "a peek must not issue");
        assert_eq!(forge.keypair("collateral-mint").pubkey(), first_peek);
        let after_draw = forge.peek_pubkey("collateral-mint").expect("peek after draw");
        assert_ne!(after_draw, first_peek);
        assert_eq!(forge.keypair("collateral-mint").pubkey(), after_draw);
        // Random: refused, because the future key does not exist.
        assert!(KeyForge::random().peek_pubkey("collateral-mint").is_err());
        // Seeded: peekable the same way.
        let seeded = seeded();
        let peek = seeded.peek_pubkey("collateral-mint").expect("seeded peek");
        assert_eq!(seeded.keypair("collateral-mint").pubkey(), peek);
    }

    #[test]
    fn a_random_forge_repeats_nothing() {
        let forge = KeyForge::parse(None, LOOPBACK).expect("no seed");
        assert_ne!(
            forge.keypair("core-upgrade-authority").pubkey(),
            forge.keypair("core-upgrade-authority").pubkey()
        );
        assert_eq!(forge.derivation_label(), "random-per-run");
        assert_eq!(forge.seed_sha256(), None);
    }

    #[test]
    fn the_counter_advances_identically_with_and_without_a_seed() {
        // A random campaign and a seeded one must ask for keys in the same
        // order, or the seed would change the campaign rather than only make
        // it reproducible.
        let random = KeyForge::parse(None, LOOPBACK).expect("no seed");
        let _ = random.keypair("collateral-mint");
        let _ = random.keypair("collateral-mint");
        assert_eq!(random.issued.borrow().get("collateral-mint"), Some(&2));
        let forge = seeded();
        let _ = forge.keypair("collateral-mint");
        let _ = forge.keypair("collateral-mint");
        assert_eq!(forge.issued.borrow().get("collateral-mint"), Some(&2));
    }

    #[test]
    fn the_seed_is_refused_off_loopback() {
        for endpoint in [
            "https://api.mainnet-beta.solana.com/",
            "https://api.devnet.solana.com/",
            "http://example.com:20890/",
            "http://8.8.8.8:20890/",
            "https://127.0.0.1:20890/",
        ] {
            let refusal = KeyForge::parse(Some(SEED), endpoint)
                .err()
                .unwrap_or_else(|| panic!("{endpoint} must refuse a seed"));
            assert!(
                refusal.0.contains("catastrophic footgun"),
                "the refusal must say why, got: {}",
                refusal.0
            );
            assert!(
                refusal.0.contains(endpoint),
                "the refusal must name {endpoint}"
            );
        }
        // An absent seed is unaffected: a random campaign is safe anywhere.
        assert!(KeyForge::parse(None, "https://api.mainnet-beta.solana.com/").is_ok());
    }

    #[test]
    fn localhost_by_name_is_admitted() {
        assert!(KeyForge::parse(Some(SEED), "http://localhost:20890/").is_ok());
        assert!(KeyForge::parse(Some(SEED), "http://[::1]:20890/").is_ok());
    }

    #[test]
    fn a_persisted_roles_first_key_is_the_file_itself() {
        // The property the operator depends on: the address they funded with
        // `solana address -k core-upgrade-authority.json` is the address this
        // campaign pays fees from, with nothing derived in between.
        let secret = [7_u8; 32];
        let file_key = Keypair::new_from_array(secret);
        let forge = KeyForge::persisted(
            BTreeMap::from([(role::CORE_UPGRADE_AUTHORITY.to_owned(), secret)]),
            &[role::CORE_UPGRADE_AUTHORITY],
        )
        .expect("one supplied role");
        assert_eq!(
            forge.keypair(role::CORE_UPGRADE_AUTHORITY).pubkey(),
            file_key.pubkey(),
            "index 0 must be the file's own key"
        );
        // ...and the second key under the same role is derived, not the file
        // again, so two accounts never collapse into one.
        let second = forge.keypair(role::CORE_UPGRADE_AUTHORITY).pubkey();
        assert_ne!(second, file_key.pubkey());
        // Reproducible from the same file, which is what makes a resumed
        // campaign able to sign for what the first attempt created.
        let resumed = KeyForge::persisted(
            BTreeMap::from([(role::CORE_UPGRADE_AUTHORITY.to_owned(), secret)]),
            &[role::CORE_UPGRADE_AUTHORITY],
        )
        .expect("one supplied role");
        let _ = resumed.keypair(role::CORE_UPGRADE_AUTHORITY);
        assert_eq!(
            resumed.keypair(role::CORE_UPGRADE_AUTHORITY).pubkey(),
            second
        );
    }

    #[test]
    fn a_persisted_campaign_refuses_a_role_it_was_given_no_file_for() {
        let refusal = KeyForge::persisted(
            BTreeMap::from([(role::CORE_UPGRADE_AUTHORITY.to_owned(), [7_u8; 32])]),
            &[role::CORE_UPGRADE_AUTHORITY, role::COLLATERAL_MINT],
        )
        .err()
        .expect("a missing role must refuse");
        assert!(refusal.0.contains(role::COLLATERAL_MINT));
        assert!(!refusal.0.contains(role::CORE_UPGRADE_AUTHORITY));
    }

    #[test]
    fn the_two_keyed_origins_are_domain_separated() {
        // The same 32 bytes as a --keypair-seed and as a persisted file must
        // never produce the same key, or a lab seed could sign for a funded
        // devnet account.
        let material = [9_u8; 32];
        let seeded = KeyForge::parse(Some(&hex(&material)), LOOPBACK).expect("loopback seed");
        let persisted = KeyForge::persisted(
            BTreeMap::from([(role::COLLATERAL_MINT.to_owned(), material)]),
            &[role::COLLATERAL_MINT],
        )
        .expect("persisted");
        // Compare at index 1, where both origins derive rather than one of
        // them returning the file verbatim.
        let _ = seeded.keypair(role::COLLATERAL_MINT);
        let _ = persisted.keypair(role::COLLATERAL_MINT);
        assert_ne!(
            seeded.keypair(role::COLLATERAL_MINT).pubkey(),
            persisted.keypair(role::COLLATERAL_MINT).pubkey()
        );
        assert_eq!(persisted.derivation_label(), "persisted-per-role");
        assert!(persisted.persists_private_keys());
        assert!(!seeded.persists_private_keys());
        assert!(!KeyForge::random().persists_private_keys());
        // The evidence must not carry a fingerprint of a live funded key.
        assert_eq!(persisted.seed_sha256(), None);
    }

    #[test]
    fn a_malformed_seed_is_refused() {
        for value in [
            "",
            "01",
            "0000000000000000000000000000000000000000000000000000000000000001a",
            "000000000000000000000000000000000000000000000000000000000000000G",
            // Uppercase hex: the seed is a canonical spelling, so exactly one
            // string names each seed and two scripts cannot disagree about
            // which campaign they pinned.
            "000000000000000000000000000000000000000000000000000000000000000A",
            " 000000000000000000000000000000000000000000000000000000000000001",
        ] {
            assert!(
                KeyForge::parse(Some(value), LOOPBACK).is_err(),
                "must refuse {value:?}"
            );
        }
    }

    #[test]
    fn the_derivation_is_pinned_to_its_stated_formula() {
        // Recomputed here from the documented formula rather than read back out
        // of the implementation: if the concatenation order, the separators or
        // the index width move, this fails.
        let seed = crate::plan::hex32(SEED).expect("seed");
        let mut expected = Sha256::new();
        expected.update(b"dclutch/local-successor-bootstrap/keypair-seed/v1");
        expected.update([0]);
        expected.update(seed);
        expected.update([0]);
        expected.update(b"collateral-mint");
        expected.update([0]);
        expected.update(1_u32.to_le_bytes());
        let expected = Keypair::new_from_array(expected.finalize().into());

        let forge = seeded();
        let _ = forge.keypair("collateral-mint");
        assert_eq!(forge.keypair("collateral-mint").pubkey(), expected.pubkey());
    }
}
