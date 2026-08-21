//! The fabricated pull identity is unreachable on any real cluster.
//!
//! `clutch_sbf::source_identity` claims something load-bearing: the laboratory
//! release compiled into the **default** ELF admits no value anywhere, because
//! every address it pins is program-derived and therefore has no private key.
//! That claim is what makes registering it into the default artifact different
//! in kind from the interim production entry `R2_PULL_PROMOTION_PLAN.md` §6
//! forbids, so it should not rest on prose.
//!
//! It cannot be checked inside the program crate. Off-chain program-address
//! derivation needs the `curve25519` backend, and `programs/clutch-sbf`
//! deliberately cannot enable it — its proc-macro dependency has no archive in
//! this host's offline cache and `cargo-build-sbf` runs `cargo metadata` for
//! every target, which is why `clutch_sbf::seeds::find` is `unimplemented!()`
//! off-chain. Here it resolves, so here is where the check lives.
//!
//! No bank is started: these are derivations over pinned constants.

use clutch_sbf::loader_state::UPGRADEABLE_LOADER_ID;
use clutch_sbf::source_identity::{fixture, mainnet, PullReleaseV2, REGISTERED_RELEASES};
use solana_address::Address;
use solana_keypair::Keypair;
use solana_signer::Signer;

/// The seed the fixture receiver is derived from.
///
/// Restated here rather than exported from the program: the constant that
/// matters is the resulting address, and re-deriving from an independently
/// written seed is what makes this a check rather than a tautology.
const RECEIVER_SEED: &[u8] = b"dc-r2-fixture-receiver";

/// The receiver `Config` PDA seed, as the Pyth receiver derives it.
const CONFIG_SEED: &[u8] = b"config";

fn loader() -> Address {
    Address::new_from_array(UPGRADEABLE_LOADER_ID)
}

#[test]
fn fixture_addresses_are_program_derived_and_therefore_unreachable() {
    let loader = loader();

    let (receiver, receiver_bump) = Address::find_program_address(&[RECEIVER_SEED], &loader);
    assert_eq!(
        receiver.to_bytes(),
        fixture::RECEIVER_PROGRAM,
        "the pinned receiver is not the address its documented derivation yields"
    );

    /* The Upgradeable Loader's own ProgramData derivation, applied to the
     * fixture receiver.  The fixture is therefore shaped like a real
     * deployment rather than like a stand-in for one. */
    let (programdata, programdata_bump) =
        Address::find_program_address(&[&fixture::RECEIVER_PROGRAM], &loader);
    assert_eq!(programdata.to_bytes(), fixture::RECEIVER_PROGRAMDATA);

    /* The receiver-owned `config` PDA, derived under the fixture receiver. */
    let (config, config_bump) = Address::find_program_address(&[CONFIG_SEED], &receiver);
    assert_eq!(config.to_bytes(), fixture::RECEIVER_CONFIG);

    /* `find_program_address` returns only addresses `create_program_address`
     * accepts, and `create_program_address` refuses any result that lands on
     * the ed25519 curve.  Round-tripping each with its bump is therefore a
     * direct proof of off-curve-ness, in the same call the loader itself
     * would make. */
    assert_eq!(
        Address::create_program_address(&[RECEIVER_SEED, &[receiver_bump]], &loader).unwrap(),
        receiver
    );
    assert_eq!(
        Address::create_program_address(
            &[&fixture::RECEIVER_PROGRAM, &[programdata_bump]],
            &loader
        )
        .unwrap(),
        programdata
    );
    assert_eq!(
        Address::create_program_address(&[CONFIG_SEED, &[config_bump]], &receiver).unwrap(),
        config
    );

    /* Stated as the property it buys: an off-curve address has no keypair, a
     * program deploy requires the program account to sign its own
     * `DeployWithMaxDataLen`, and so no executable account can be created at
     * the fixture receiver on any cluster by anyone -- us included. */
    assert!(!receiver.is_on_curve());
    assert!(!programdata.is_on_curve());
    assert!(!config.is_on_curve());
}

#[test]
fn the_default_elf_registers_exactly_one_laboratory_release() {
    assert_eq!(REGISTERED_RELEASES.len(), 1);
    let release: PullReleaseV2 = REGISTERED_RELEASES[0];
    assert_eq!(release, fixture::RELEASE);
    assert_eq!(release.receiver_program, fixture::RECEIVER_PROGRAM);
    assert_eq!(release.upgradeable_loader, UPGRADEABLE_LOADER_ID);
}

#[test]
fn no_production_identity_byte_is_pinned() {
    // The E2 freeze act is what fills these in, and it is reserved to ember.
    // This test is the mechanical form of that reservation: it goes red the
    // moment a pin lands, so the freeze cannot happen as a side effect of
    // unrelated work.
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
}

#[test]
fn a_real_wallet_address_is_on_curve_and_is_none_of_the_fixture_addresses() {
    /* A negative control, and a real one: an address a keypair actually
     * produced.  Without it the off-curve assertions above could pass
     * vacuously if `is_on_curve` ever became a stub, and an arbitrary byte
     * array would not do -- roughly half of those are off-curve too, which is
     * exactly why the fixture addresses had to be *derived* rather than
     * picked. */
    for _ in 0..8 {
        let wallet = Keypair::new().pubkey();
        assert!(
            wallet.is_on_curve(),
            "an address a keypair produced must be on-curve"
        );
        assert_ne!(wallet.to_bytes(), fixture::RECEIVER_PROGRAM);
        assert_ne!(wallet.to_bytes(), fixture::RECEIVER_PROGRAMDATA);
        assert_ne!(wallet.to_bytes(), fixture::RECEIVER_CONFIG);
    }
}
