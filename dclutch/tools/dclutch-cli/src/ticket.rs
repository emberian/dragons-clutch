//! `dclutch ticket` — the named seam for the Direct intent-ticket author.
//!
//! WHY THIS IS A REFUSAL AND NOT AN IMPLEMENTATION. A Direct inline fill
//! settles two independently signed intents, and the signed message is owned by
//! `dclutch_direct_codec::intent_v2::CompactIntentV2`, emitted from
//! `formal/dclutch-semantics/EmitDirectIntentV2Rust.lean`. There is exactly one
//! author of a ticket per language and there must go on being exactly one: a
//! second implementation of a signing preimage is a signature that verifies
//! nowhere, discovered at the refused trade.
//!
//! At the version this binary was cut, the Rust author is
//! `direct-intent-ticket-author-v1`, a subcommand of the operator binary
//! `dclutch-local-successor-bootstrap` under
//! `tools/local-validator/bootstrap/successor/`. It is `pub(crate)` there and
//! coupled to that crate's argument, RPC and plan modules, so it is not yet a
//! thing a second binary can call. When it becomes one, this module calls it —
//! it does not grow its own copy.
//!
//! What the seam owes the reader in the meantime is the truth about where the
//! capability is, which is what the refusal below says.

use crate::{Error, Result};

/// The command that actually authors a ticket today.
pub const AUTHOR_COMMAND_V1: &str = "direct-intent-ticket-author-v1";

/// The binary that carries it.
pub const AUTHOR_BINARY_V1: &str = "dclutch-local-successor-bootstrap";

/// The one line the usage screen prints for this seam.
pub const ONE_LINE_STATUS_V1: &str =
    "Not in this release. Authoring a trade ticket needs a key; see below.";

/// Refuse, and say exactly where the capability lives.
pub fn run(_arguments: Vec<String>) -> Result<()> {
    Err(Error::new(refusal()))
}

/// The refusal sentence, kept as a function so a test can read it.
#[must_use]
pub fn refusal() -> String {
    format!(
        "`dclutch ticket` is a seam, not a command, in this release.\n\
         \n\
         This binary is read-only on purpose: it opens no key file, so it cannot\n\
         sign a Direct intent. The Rust ticket author does exist — it is\n\
         `{AUTHOR_COMMAND_V1}` in the operator binary `{AUTHOR_BINARY_V1}`, built\n\
         from `dclutch/tools/local-validator/bootstrap/successor/` in the source\n\
         tree — but it is private to that crate today, and copying it here would\n\
         make a second author of a signing preimage. There is one author of a\n\
         ticket per language and it stays that way.\n\
         \n\
         To trade on the devnet deployment right now, use the web trade panel at\n\
         https://clutch.dregg.pro, which signs with your wallet and never sees a\n\
         key. This subcommand takes over when the author becomes callable."
    )
}

#[cfg(test)]
mod tests {
    use super::{AUTHOR_BINARY_V1, AUTHOR_COMMAND_V1, refusal, run};

    #[test]
    fn the_seam_refuses_rather_than_pretending() {
        let error = run(Vec::new()).expect_err("a seam must refuse");
        assert!(error.to_string().contains("is a seam"));
    }

    #[test]
    fn the_refusal_names_the_command_that_can_actually_do_it() {
        let text = refusal();
        assert!(text.contains(AUTHOR_COMMAND_V1), "{text}");
        assert!(text.contains(AUTHOR_BINARY_V1), "{text}");
    }

    #[test]
    fn the_refusal_names_a_route_the_reader_can_take_today() {
        assert!(refusal().contains("https://clutch.dregg.pro"));
    }

    #[test]
    fn the_seam_never_claims_to_hold_a_key() {
        let text = refusal();
        assert!(text.contains("opens no key file"), "{text}");
    }
}
