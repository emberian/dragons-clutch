//! The Direct intent ticket, as this operator binary offers it.
//!
//! THE AUTHOR NO LONGER LIVES HERE. It was `pub(crate)` in this crate, which
//! has no `[lib]` and fifty path dependencies, so nothing else could call it --
//! and the released `dclutch` CLI had to ship a REFUSAL where `dclutch ticket`
//! should have been, because the alternative was a second author of a signing
//! preimage. It now lives in `crates/dclutch-direct-ticket`, whose real
//! dependencies are the Lean-emitted intent codec, an Ed25519 signer and serde,
//! and both binaries call that one.
//!
//! What is left here is the adapter: this crate's `Error`, this crate's
//! subcommand name, and this binary's usage line. Every name this module
//! exported before it moved is still exported from here with the same
//! signature, so the producer and `main.rs` did not change when it moved.
//!
//! THE KEY PATH IS STILL NEVER AN ARGUMENT. `--keypair-env` names an
//! ENVIRONMENT VARIABLE that holds the absolute path; `--keypair`,
//! `--keypair-path` and `--secret-key` are refused at parse. That behaviour is
//! the shared crate's and is tested there against both invocations.

use dclutch_trading::intent_v2::CompactIntentV2;
use dclutch_direct_ticket::{
    DIRECT_TICKET_AUTHOR_COMMAND_V1 as SHARED_AUTHOR_COMMAND_V1, SignedDirectIntentV3,
};
use solana_sdk::signature::Keypair;

use crate::{Error, Result};

/// The subcommand this binary answers to, unchanged since the author moved.
pub(crate) const DIRECT_TICKET_AUTHOR_COMMAND_V1: &str = SHARED_AUTHOR_COMMAND_V1;

/// How this binary is invoked, as the shared usage screen needs to name it.
const INVOCATION_V1: &str = concat!(
    "dclutch-local-successor-bootstrap ",
    "direct-intent-ticket-author-v1"
);

/// Lift a shared-crate refusal into this crate's error, text unchanged.
///
/// The sentence is the whole contract with the operator, so nothing is
/// reworded, prefixed, or wrapped on the way through.
fn lift(error: dclutch_direct_ticket::Error) -> Error {
    Error::new(error.to_string())
}

/// The usage screen for this binary's subcommand.
pub(crate) fn usage() -> String {
    dclutch_direct_ticket::usage_v1(INVOCATION_V1)
}

/// CLI integration hook. `main.rs` only needs to dispatch arguments here.
pub(crate) fn run(arguments: Vec<String>) -> Result<()> {
    dclutch_direct_ticket::run_v1(INVOCATION_V1, arguments).map_err(lift)
}

/// Emit the exact bytes `encodeDirectIntentTicketV1` emits for a signed intent.
pub(crate) fn encode_portable_direct_ticket_v1(signed: &SignedDirectIntentV3) -> Result<String> {
    dclutch_direct_ticket::encode_portable_direct_ticket_v1(signed).map_err(lift)
}

/// Hostile-read one portable ticket back into the signed intent it carries.
pub(crate) fn parse_portable_direct_ticket_v1(
    bytes: &[u8],
    label: &str,
) -> Result<SignedDirectIntentV3> {
    dclutch_direct_ticket::parse_portable_direct_ticket_v1(bytes, label).map_err(lift)
}

/// Sign one exact intent with the standard `solana-sdk` Ed25519 signer.
pub(crate) fn sign_direct_intent_v1(
    keypair: &Keypair,
    intent: CompactIntentV2,
) -> Result<SignedDirectIntentV3> {
    dclutch_direct_ticket::sign_direct_intent_v1(keypair, intent).map_err(lift)
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use dclutch_direct_ticket::{author_with_keypair_path_v1, parse_arguments_v1};
    use solana_sdk::{
        pubkey::Pubkey,
        signature::{Keypair, Signer as _},
    };

    use super::{DIRECT_TICKET_AUTHOR_COMMAND_V1, INVOCATION_V1, parse_portable_direct_ticket_v1};

    /// One 64-byte Solana CLI keypair file: the 32-byte seed then the 32-byte
    /// public key it expands to, exactly as `solana-keygen` writes it.
    fn write_keypair_file_v1(directory: &Path, name: &str, seed: [u8; 32]) -> PathBuf {
        let keypair = Keypair::new_from_array(seed);
        let mut bytes = seed.to_vec();
        bytes.extend_from_slice(&keypair.pubkey().to_bytes());
        let path = directory.join(name);
        std::fs::write(&path, serde_json::to_vec(&bytes).expect("keypair json")).expect("write");
        path
    }

    fn scratch_directory_v1(name: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("dclutch-ticketcli-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch directory");
        path
    }

    /// THE POINT OF THE SUBCOMMAND, end to end and offline.
    ///
    /// Two keypair files on disk, two authoring runs through the real argument
    /// parser, and the resulting pair is then put through the EXACT two
    /// functions `devnet-direct-trade-produce-v1` runs on `--seller-ticket` and
    /// `--buyer-ticket` before it opens a socket:
    /// `parse_portable_direct_ticket_v1` and `exact_ticket_pair_terms_v1`. What
    /// that proves is the ticket admission of the produce path, not the fill --
    /// the fill needs a chain, a founded market, and an activated Direct root.
    ///
    /// This test STAYS in this crate, while the author's own tests moved with
    /// it: `exact_ticket_pair_terms_v1` is the producer's, and a ticket crate
    /// that could see it would be a ticket crate that depends on the operator.
    #[test]
    fn a_cli_authored_pair_passes_the_producer_ticket_admission() -> crate::Result<()> {
        let scratch = scratch_directory_v1("pair");
        let market = Keypair::new_from_array([0x21; 32]).pubkey();
        let seller_key = write_keypair_file_v1(&scratch, "seller.json", [0x31; 32]);
        let buyer_key = write_keypair_file_v1(&scratch, "buyer.json", [0x32; 32]);
        let seller_maker = Keypair::new_from_array([0x31; 32]).pubkey();
        let buyer_maker = Keypair::new_from_array([0x32; 32]).pubkey();
        let seller_collateral = Keypair::new_from_array([0x41; 32]).pubkey();
        let buyer_collateral = Keypair::new_from_array([0x42; 32]).pubkey();

        let author = |name: &str,
                      key: &PathBuf,
                      maker: Pubkey,
                      side: &str,
                      nonce: &str,
                      collateral: Pubkey|
         -> crate::Result<PathBuf> {
            let out = scratch.join(name);
            let arguments = parse_arguments_v1(
                INVOCATION_V1,
                [
                    ("--keypair-env", "DCLUTCH_TICKET_KEYPAIR"),
                    ("--maker", &maker.to_string()),
                    ("--market", &market.to_string()),
                    ("--collateral-account", &collateral.to_string()),
                    ("--side", side),
                    ("--lifecycle", "fok"),
                    ("--outcome", "3"),
                    ("--generation", "2"),
                    ("--nonce", nonce),
                    ("--valid-from", "11"),
                    ("--valid-through", "432011"),
                    ("--maximum-fill", "100000000"),
                    ("--limit-price", "500000"),
                    ("--fee-basis-points", "50"),
                    ("--out", &out.display().to_string()),
                ]
                .into_iter()
                .flat_map(|(flag, value)| [flag.to_string(), value.to_string()])
                .collect(),
            )
            .map_err(super::lift)?;
            author_with_keypair_path_v1(arguments, key).map_err(super::lift)?;
            Ok(out)
        };

        let seller_path = author(
            "seller-ticket.json",
            &seller_key,
            seller_maker,
            "sell",
            "0",
            seller_collateral,
        )?;
        let buyer_path = author(
            "buyer-ticket.json",
            &buyer_key,
            buyer_maker,
            "buy",
            "0",
            buyer_collateral,
        )?;

        // The producer's own two gates, unmodified.
        let seller = parse_portable_direct_ticket_v1(&std::fs::read(&seller_path)?, "seller")?;
        let buyer = parse_portable_direct_ticket_v1(&std::fs::read(&buyer_path)?, "buyer")?;
        let terms = crate::direct_trade_producer::exact_ticket_pair_terms_v1(&seller, &buyer)?;
        assert_eq!(terms.outcome, 3);
        assert_eq!(terms.fill, 100_000_000);
        assert_eq!(terms.execution_price, 500_000);
        assert_eq!(terms.fee_basis_points, 50);
        let _ = std::fs::remove_dir_all(&scratch);
        Ok(())
    }

    /// This binary's own usage line still names this binary's own subcommand.
    #[test]
    fn the_usage_screen_names_this_binary_and_no_key_flag() {
        let usage = super::usage();
        assert!(usage.starts_with(&format!(
            "dclutch-local-successor-bootstrap {DIRECT_TICKET_AUTHOR_COMMAND_V1} "
        )));
        assert!(usage.contains("--keypair-env"));
        for forbidden in ["--keypair ", "--keypair-path", "--secret-key", "--seed"] {
            assert!(!usage.contains(forbidden), "usage exposed {forbidden}");
        }
    }
}
