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
use solana_sdk::signature::Keypair;

use crate::{Error, Result, plan::hex, rpc::validate_loopback_url};

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

/// Where a campaign's signing keys come from.
enum KeyOriginV1 {
    /// The default. One fresh key per request, unreproducible by anyone,
    /// including this process a moment later.
    Random,
    /// Test-only. Every key is a pure function of this seed, the role name, and
    /// how many keys that role has already been issued.
    Seeded([u8; 32]),
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
        // The gate, before anything is derived. `validate_loopback_url` is the
        // one owner of "is this a loopback origin"; this is the one owner of
        // "may a seeded key exist against it".
        if validate_loopback_url(rpc_url).is_err() {
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

    /// Issue the next keypair for `role`.
    ///
    /// Under a seed this is `SHA-256(DOMAIN || 0 || seed || 0 || role || 0 ||
    /// index)` read as an ed25519 secret seed, where `index` counts prior
    /// issues under the same role. The counter advances either way, so a run
    /// with and a run without a seed ask for keys in exactly the same order.
    pub(crate) fn keypair(&self, role: &'static str) -> Keypair {
        let index = {
            let mut issued = self.issued.borrow_mut();
            let counter = issued.entry(role).or_insert(0);
            let index = *counter;
            *counter = counter.saturating_add(1);
            index
        };
        match self.origin {
            KeyOriginV1::Random => Keypair::new(),
            KeyOriginV1::Seeded(seed) => {
                let mut material = Sha256::new();
                material.update(KEYPAIR_SEED_DOMAIN_V1);
                material.update([0]);
                material.update(seed);
                material.update([0]);
                material.update(role.as_bytes());
                material.update([0]);
                material.update(index.to_le_bytes());
                Keypair::new_from_array(material.finalize().into())
            }
        }
    }

    /// How the evidence document must describe this campaign's keys.
    pub(crate) fn derivation_label(&self) -> &'static str {
        match self.origin {
            KeyOriginV1::Random => "random-per-run",
            KeyOriginV1::Seeded(_) => "seeded-deterministic",
        }
    }

    /// SHA-256 of the seed, or `None` for a random campaign.
    ///
    /// The digest and not the seed: it identifies which seed produced a run's
    /// compute-unit numbers without the evidence file itself becoming a way to
    /// sign as the campaign.
    pub(crate) fn seed_sha256(&self) -> Option<String> {
        match self.origin {
            KeyOriginV1::Random => None,
            KeyOriginV1::Seeded(seed) => Some(hex(&Sha256::digest(seed))),
        }
    }
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
            assert!(refusal.0.contains(endpoint), "the refusal must name {endpoint}");
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
