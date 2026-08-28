//! Which cluster a run may talk to, and what it is allowed to do there.
//!
//! # What this replaces, and why the replacement is not a weakening
//!
//! Until this module existed, `runtime::rpc_origin` refused any RPC origin that
//! was not literal `127.0.0.1`. That rail had two distinct jobs wearing one
//! coat:
//!
//! 1. **The supervisor must talk to the validator it started.** The launcher
//!    binds `127.0.0.1` and nothing else, so a spec naming `localhost` or
//!    `[::1]` would pass a loopback test and then fail to reach the process
//!    this campaign owns. That job is real, is unchanged, and still belongs to
//!    the loopback origin alone.
//! 2. **Accidental mainnet must be impossible.** A campaign signs revocations,
//!    funds accounts, and moves principal. One mistyped URL is the whole
//!    disaster. `127.0.0.1` bought that as a side effect of (1).
//!
//! A devnet driver has to give up (1) — it launches nothing and must reach a
//! public endpoint — without giving up (2). So (2) stops being a side effect
//! and becomes the explicit rule this module states:
//!
//! * A **loopback** origin is admitted exactly as before, with no ceremony:
//!   `http`, credential-free, explicit port in the launcher's derivable range,
//!   host literally `127.0.0.1`.
//! * A **non-loopback** origin is refused *unless* the caller passes the named
//!   acknowledgment [`DEVNET_ACKNOWLEDGMENT_FLAG`], whose value must be the
//!   devnet genesis hash spelled out in full. A flag you must type a 44-character
//!   cluster identity into is not a flag you pass by accident, and it is not a
//!   flag that travels usefully in a copied command line to some other cluster.
//! * **Mainnet-beta is refused unconditionally**, at three independent points:
//!   its genesis hash is not a value the acknowledgment accepts; its well-known
//!   host shapes are refused statically before a byte leaves the machine; and
//!   [`ClusterOriginV1::authenticate_genesis`] refuses the observed mainnet
//!   genesis hash **even on a loopback origin**, because a loopback port can be
//!   a tunnel and a tunnel is exactly the accident nobody plans.
//!
//! The static host rule is a heuristic and is deliberately not the gate. The
//! gate is the genesis hash, read off the cluster itself before any write is
//! constructed: a provider endpoint that is silently mainnet fails there, and no
//! spelling of a URL can talk its way past a hash the chain reports about
//! itself.
//!
//! # The other things an origin decides
//!
//! Origin is not only "may I connect". It is also the one honest owner of three
//! questions that used to be answered by re-deriving "is this loopback" at each
//! site:
//!
//! * **May seeded keypairs exist?** ([`ClusterOriginV1::may_use_seeded_keys`])
//!   Loopback only. `seed.rs` states the reason at length; the answer lives
//!   here so there is one of it.
//! * **May this run airdrop?** ([`ClusterOriginV1::may_airdrop`]) Loopback only.
//!   The devnet faucet is rate-limited far below a campaign's needs, and a
//!   driver that quietly begs for lamports mid-ladder fails halfway through a
//!   stage instead of refusing at preflight with an exact shortfall.
//! * **How fast may this run poll?** ([`ClusterOriginV1::pacing`]) SMOKE-0
//!   friction 1 measured one busy writer starving *every other request from the
//!   same IP*, a 1-per-20-second account poll included. The loopback profile is
//!   unpaced because there is no shared budget to starve; the devnet profile
//!   spaces calls and waits longer for finality, because the alternative is a
//!   campaign that generates its own 429s.

use std::time::Duration;

use crate::{Error, Result, rpc::validate_loopback_url};

/// Devnet's genesis hash.
///
/// Chain-derived and measured: `tools/release/devnet-observe.sh` reads
/// `getGenesisHash` live and compares it against this exact string, and
/// `docs/evidence/DEVNET_SMOKE_0.md` §1.1 records the same value observed on
/// 2026-08-27.
pub(crate) const DEVNET_GENESIS_HASH: &str = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";

/// Mainnet-beta's genesis hash. Never admitted by anything in this tree.
pub(crate) const MAINNET_BETA_GENESIS_HASH: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";

/// The flag whose value is a cluster identity rather than a bare `true`.
pub(crate) const DEVNET_ACKNOWLEDGMENT_FLAG: &str = "--i-mean-devnet";

/// The launcher derives a 42-port block from its base (`BASE + 41` is the top
/// of its dynamic range), so a base above this cannot be served at all.
pub(crate) const MAX_RPC_PORT: u16 = 65_494;

/// Below 1024 needs privileges the launcher deliberately never has.
pub(crate) const MIN_RPC_PORT: u16 = 1024;

/// How often a run may speak, and how long it waits for finality.
///
/// One busy process saturates a public endpoint's whole per-IP budget
/// (SMOKE-0 friction 1), so the interval is not a politeness knob: it is the
/// difference between a campaign that finishes and a campaign that spends its
/// stage generating `429 Connection rate limits exceeded` for itself.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PacingV1 {
    /// Minimum wall-clock gap between two RPC calls from this process.
    pub(crate) minimum_call_interval: Duration,
    /// How long a submitted transaction may take to reach finalized history.
    pub(crate) confirm_timeout: Duration,
    /// How often, while awaiting confirmation, to resubmit the same signed
    /// bytes against a devnet drop. Idempotent by signature.
    pub(crate) resubmit_interval: Duration,
}

/// The unpaced profile: a validator this process owns, on a socket nobody else
/// is charged for.
pub(crate) const LOOPBACK_PACING: PacingV1 = PacingV1 {
    minimum_call_interval: Duration::ZERO,
    confirm_timeout: Duration::from_secs(60),
    // Longer than the whole loopback budget: a validator this process owns
    // does not drop its own transactions, so the resubmit never fires locally.
    resubmit_interval: Duration::from_secs(120),
};

/// The public-endpoint profile.
///
/// `minimum_call_interval` is measured-profile, not mathematical: SMOKE-0 read
/// the public endpoint at one call per 20 s alongside a busy writer and got
/// 429s, and at a few calls per second alone without them. 250 ms is four calls
/// a second, which leaves the confirmation loop responsive while keeping a
/// single stage far below the rate one `write-buffer` was measured to consume.
/// `confirm_timeout` is five minutes because devnet finality is not local
/// finality and a driver that gives up early re-sends bytes the chain already
/// has.
const DEVNET_PACING: PacingV1 = PacingV1 {
    minimum_call_interval: Duration::from_millis(250),
    confirm_timeout: Duration::from_secs(300),
    // ~30 s is a few blockhash lifetimes: long enough not to spam the endpoint
    // (a resubmit is one sendTransaction), short enough that a drop is retried
    // several times inside the 300 s deadline.
    resubmit_interval: Duration::from_secs(30),
};

/// The cluster this run is pointed at, already proven admissible.
///
/// Constructing one of these is the whole gate. There is no way to reach the
/// devnet variant without having passed the acknowledgment, and no way to reach
/// either variant with a mainnet host shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ClusterOriginV1 {
    /// The historical rail, unchanged: the guarded validator this process
    /// starts, on the one interface the launcher binds.
    Loopback {
        /// Normalized origin, always `http://127.0.0.1:<port>/`.
        url: String,
        /// The port, which the launcher derives its whole block from.
        port: u16,
    },
    /// An external devnet endpoint the operator named and acknowledged.
    AcknowledgedDevnet {
        /// The endpoint as given. May carry a provider path or query; see
        /// [`ClusterOriginV1::redacted_url`] before printing it anywhere.
        url: String,
    },
}

impl ClusterOriginV1 {
    /// Admit an origin, or refuse it and say exactly why.
    ///
    /// `acknowledgment` is the value of [`DEVNET_ACKNOWLEDGMENT_FLAG`], absent
    /// for every run that does not mean to leave this machine.
    pub(crate) fn parse(rpc_url: &str, acknowledgment: Option<&str>) -> Result<Self> {
        let loopback = validate_loopback_url(rpc_url);
        match (loopback, acknowledgment) {
            (Ok(url), Some(_)) => {
                // Not an error we can silently absorb: the operator asked for
                // devnet and got a loopback socket, which means one of the two
                // is a typo and we cannot tell which.
                Err(Error::new(format!(
                    "{DEVNET_ACKNOWLEDGMENT_FLAG} was given for the loopback origin {url}. A \
                     loopback origin needs no acknowledgment, so one of the two is a mistake and \
                     this refuses rather than guessing which."
                )))
            }
            (Ok(url), None) => {
                let host = url.host_str().unwrap_or_default();
                if host != "127.0.0.1" {
                    return Err(Error::new(format!(
                        "successor RPC origin must be on 127.0.0.1, which is the only interface \
                         the launcher binds; the spec names {host}"
                    )));
                }
                let port = url
                    .port()
                    .ok_or_else(|| Error::new("successor RPC origin must name an explicit port"))?;
                if !(MIN_RPC_PORT..=MAX_RPC_PORT).contains(&port) {
                    return Err(Error::new(format!(
                        "successor RPC port {port} is outside {MIN_RPC_PORT}-{MAX_RPC_PORT}; the \
                         launcher derives a 42-port block from it and the block must fit under \
                         65535"
                    )));
                }
                Ok(Self::Loopback {
                    url: url.as_str().to_owned(),
                    port,
                })
            }
            (Err(_), acknowledgment) => Self::parse_external(rpc_url, acknowledgment),
        }
    }

    /// The non-loopback half, kept separate so the loopback rail above reads as
    /// the unchanged thing it is.
    fn parse_external(rpc_url: &str, acknowledgment: Option<&str>) -> Result<Self> {
        let url = reqwest::Url::parse(rpc_url)
            .map_err(|error| Error::new(format!("RPC URL: {error}")))?;
        // Refused before the acknowledgment is even read: a mainnet host shape
        // is never a thing this tool does, so it is not a thing an operator can
        // acknowledge their way into.
        refuse_mainnet_host(&url)?;
        // A loopback HOST that failed the loopback SHAPE rules is a typo in the
        // URL, not a request to leave the machine. Saying "pass the devnet
        // acknowledgment" to somebody who wrote `https://127.0.0.1:20890/`
        // would be advice toward the wrong fix.
        if url
            .host_str()
            .map(|host| host.trim_start_matches('[').trim_end_matches(']'))
            .is_some_and(|host| {
                host.eq_ignore_ascii_case("localhost")
                    || host
                        .parse::<std::net::IpAddr>()
                        .is_ok_and(|address| address.is_loopback())
            })
        {
            return Err(Error::new(format!(
                "RPC origin {rpc_url} names a loopback host but is not a credential-free \
                 explicit-port `http://127.0.0.1:PORT/` origin, which is the only shape the \
                 launcher answers on. This is a spelling to fix, not a cluster to acknowledge."
            )));
        }
        let Some(acknowledgment) = acknowledgment else {
            return Err(Error::new(format!(
                "RPC origin {rpc_url} is not loopback, and this campaign signs revocations, funds \
                 accounts and moves principal. Pass `{DEVNET_ACKNOWLEDGMENT_FLAG} \
                 {DEVNET_GENESIS_HASH}` to target devnet deliberately. Mainnet-beta is refused \
                 unconditionally and no flag admits it."
            )));
        };
        if acknowledgment == MAINNET_BETA_GENESIS_HASH {
            return Err(Error::new(format!(
                "{DEVNET_ACKNOWLEDGMENT_FLAG} was given mainnet-beta's genesis hash. Mainnet is \
                 refused unconditionally by this tool: there is no flag, no environment variable \
                 and no spelling of a URL that admits it."
            )));
        }
        if acknowledgment != DEVNET_GENESIS_HASH {
            return Err(Error::new(format!(
                "{DEVNET_ACKNOWLEDGMENT_FLAG} must be devnet's genesis hash \
                 {DEVNET_GENESIS_HASH}, spelled in full; it was given {acknowledgment:?}. The \
                 flag names a cluster identity rather than a boolean so that a command line \
                 copied to another cluster stops being true."
            )));
        }
        if url.scheme() != "https" {
            return Err(Error::new(format!(
                "external RPC origin {rpc_url} must be https. A campaign's transactions and \
                 account reads are not something to hand to a plaintext hop on a public network."
            )));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(Error::new(
                "external RPC origin must not carry URL credentials; a provider key belongs in \
                 the path or query, which this redacts, not in userinfo, which every proxy logs",
            ));
        }
        if url.fragment().is_some() {
            return Err(Error::new(
                "external RPC origin must not carry a fragment; a fragment is never sent to the \
                 server and its presence means the URL was pasted from a browser",
            ));
        }
        if url.host_str().is_none() {
            return Err(Error::new("external RPC origin omitted a host"));
        }
        Ok(Self::AcknowledgedDevnet {
            url: url.as_str().to_owned(),
        })
    }

    /// The endpoint to actually call.
    pub(crate) fn url(&self) -> &str {
        match self {
            Self::Loopback { url, .. } | Self::AcknowledgedDevnet { url } => url,
        }
    }

    /// The endpoint as it may appear in evidence, a log line, or a refusal.
    ///
    /// A paid devnet endpoint carries its API key in the path or the query
    /// (SMOKE-0 §6.4 asks for exactly such an endpoint), and an evidence
    /// document that publishes one has published a credential. Scheme, host and
    /// port identify the endpoint for every purpose evidence has; the secret
    /// part is replaced rather than trimmed, so a reader can see that something
    /// was there.
    pub(crate) fn redacted_url(&self) -> String {
        match self {
            Self::Loopback { url, .. } => url.clone(),
            Self::AcknowledgedDevnet { url } => match reqwest::Url::parse(url) {
                Err(_) => "https://<unparseable>/".into(),
                Ok(parsed) => {
                    let host = parsed.host_str().unwrap_or("<no-host>");
                    let port = parsed
                        .port()
                        .map(|port| format!(":{port}"))
                        .unwrap_or_default();
                    let tail = if parsed.path() == "/" && parsed.query().is_none() {
                        "/".to_owned()
                    } else {
                        "/<redacted>".to_owned()
                    };
                    format!("https://{host}{port}{tail}")
                }
            },
        }
    }

    /// A short name for the cluster, for evidence and refusals.
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::Loopback { .. } => "loopback",
            Self::AcknowledgedDevnet { .. } => "devnet",
        }
    }

    /// The loopback port, for the launcher and the port-free proof.
    pub(crate) fn loopback_port(&self) -> Option<u16> {
        match self {
            Self::Loopback { port, .. } => Some(*port),
            Self::AcknowledgedDevnet { .. } => None,
        }
    }

    /// Whether this run may start and own a validator.
    pub(crate) fn may_launch_validator(&self) -> bool {
        matches!(self, Self::Loopback { .. })
    }

    /// Whether this run may ask the cluster for lamports.
    ///
    /// Loopback only. Devnet's faucet is rate-limited far below what a campaign
    /// needs, so a driver that airdrops on demand fails in the middle of a
    /// ladder; the driver instead proves the payer's balance at preflight and
    /// names the shortfall in lamports.
    pub(crate) fn may_airdrop(&self) -> bool {
        matches!(self, Self::Loopback { .. })
    }

    /// How fast this run may speak to the cluster.
    pub(crate) fn pacing(&self) -> PacingV1 {
        match self {
            Self::Loopback { .. } => LOOPBACK_PACING,
            Self::AcknowledgedDevnet { .. } => DEVNET_PACING,
        }
    }

    /// Check the cluster's own account of which chain it is.
    ///
    /// This is the gate the URL rules only approximate. It runs before any
    /// transaction is constructed, and it refuses mainnet-beta on **either**
    /// variant, loopback included: a loopback port can be an SSH tunnel or a
    /// proxy, and "I was sure it was local" is the shape every one of these
    /// accidents has.
    pub(crate) fn authenticate_genesis(&self, observed: &str) -> Result<()> {
        if observed == MAINNET_BETA_GENESIS_HASH {
            return Err(Error::new(format!(
                "the endpoint at {} reports MAINNET-BETA's genesis hash {observed}. Refusing \
                 unconditionally. A loopback port can be a tunnel and a provider URL can be \
                 mislabelled, which is why this is checked against the chain's own answer rather \
                 than against the spelling of a URL.",
                self.redacted_url()
            )));
        }
        match self {
            // A fresh local ledger mints a new genesis hash every run, so there
            // is nothing to compare it against. Everything above still applies.
            Self::Loopback { .. } => Ok(()),
            Self::AcknowledgedDevnet { .. } => {
                if observed != DEVNET_GENESIS_HASH {
                    return Err(Error::new(format!(
                        "the endpoint at {} reports genesis hash {observed}, which is not \
                         devnet's {DEVNET_GENESIS_HASH}. The acknowledgment named devnet; this \
                         endpoint is some other chain.",
                        self.redacted_url()
                    )));
                }
                Ok(())
            }
        }
    }
}

/// May a run against this endpoint derive its private keys from a seed?
///
/// The seed's question is "can this endpoint leave the machine", which is a
/// slightly *wider* admission than [`ClusterOriginV1::Loopback`]: the supervisor
/// needs the literal dotted quad because that is the one interface the launcher
/// binds, while `localhost` and `[::1]` are equally incapable of carrying a key
/// off this host. Both sets are stated here rather than re-derived at the call
/// site, so widening the origin allowlist can never widen this by accident.
///
/// The property that matters is proved by
/// `tests::no_acknowledged_origin_can_ever_admit_a_seed`: an acknowledged origin
/// is never loopback, because [`ClusterOriginV1::parse`] refuses the
/// acknowledgment on a loopback URL outright.
pub(crate) fn seeded_keys_admissible(rpc_url: &str) -> bool {
    validate_loopback_url(rpc_url).is_ok()
}

/// Refuse the well-known mainnet host shapes before a byte leaves the machine.
///
/// Deliberately a heuristic and deliberately not the gate — the genesis hash is
/// the gate. This exists so the common accident (a URL pasted from the wrong
/// line of a runbook) dies locally, with a message about mainnet, instead of
/// dying after a DNS lookup and a TLS handshake to a mainnet validator.
fn refuse_mainnet_host(url: &reqwest::Url) -> Result<()> {
    let Some(host) = url.host_str() else {
        return Ok(());
    };
    let lowered = host.to_ascii_lowercase();
    let named_mainnet = lowered
        .split(['.', '-'])
        .any(|label| label == "mainnet" || label == "mainnetbeta");
    if named_mainnet {
        return Err(Error::new(format!(
            "RPC host {host} names mainnet. Mainnet is refused unconditionally by this tool. \
             (This is the cheap check; the cluster's own genesis hash is the real one, and it \
             refuses mainnet however the host is spelled.)"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACK: Option<&str> = Some(DEVNET_GENESIS_HASH);

    #[test]
    fn the_loopback_rail_is_byte_for_byte_the_rail_it_replaced() {
        // These are `runtime::rpc_origin`'s own cases, moved here with it. The
        // point of the move is that nothing about loopback changed.
        assert_eq!(
            ClusterOriginV1::parse("http://127.0.0.1:20890/", None).expect("default origin"),
            ClusterOriginV1::Loopback {
                url: "http://127.0.0.1:20890/".into(),
                port: 20890
            }
        );
        assert_eq!(
            ClusterOriginV1::parse("http://127.0.0.1:31890/", None).expect("nonstandard origin"),
            ClusterOriginV1::Loopback {
                url: "http://127.0.0.1:31890/".into(),
                port: 31890
            }
        );
        for value in [
            "http://8.8.8.8:20890/",
            "https://127.0.0.1:20890/",
            "http://127.0.0.1:20890/path",
            "http://user@127.0.0.1:20890/",
            "http://127.0.0.1/",
            // Honest loopback origins the launcher would never answer on.
            "http://localhost:20890/",
            "http://[::1]:20890/",
        ] {
            assert!(
                ClusterOriginV1::parse(value, None).is_err(),
                "must refuse {value}"
            );
        }
        assert!(
            ClusterOriginV1::parse(&format!("http://127.0.0.1:{}/", MAX_RPC_PORT + 1), None)
                .is_err()
        );
        assert!(
            ClusterOriginV1::parse(&format!("http://127.0.0.1:{}/", MIN_RPC_PORT - 1), None)
                .is_err()
        );
    }

    #[test]
    fn a_public_origin_without_the_acknowledgment_is_refused_and_says_what_to_type() {
        let refusal = ClusterOriginV1::parse("https://api.devnet.solana.com/", None)
            .err()
            .expect("must refuse");
        assert!(refusal.0.contains(DEVNET_ACKNOWLEDGMENT_FLAG));
        assert!(refusal.0.contains(DEVNET_GENESIS_HASH));
    }

    #[test]
    fn the_acknowledgment_admits_devnet_and_nothing_else() {
        assert_eq!(
            ClusterOriginV1::parse("https://api.devnet.solana.com/", ACK).expect("devnet"),
            ClusterOriginV1::AcknowledgedDevnet {
                url: "https://api.devnet.solana.com/".into()
            }
        );
        // A provider endpoint with a keyed path or query is admitted, because
        // SMOKE-0 §6.4 asks for a dedicated endpoint and that is how they are
        // keyed.
        assert!(
            ClusterOriginV1::parse("https://rpc.example.net/dclutch-key-abc", ACK).is_ok(),
            "a keyed provider path is a devnet endpoint like any other"
        );
        assert!(ClusterOriginV1::parse("https://rpc.example.net/?api-key=abc", ACK).is_ok());
        for value in [
            // Plaintext on a public network.
            "http://api.devnet.solana.com/",
            // Credentials every proxy in the path logs.
            "https://user:pass@rpc.example.net/",
            // Pasted from a browser.
            "https://rpc.example.net/#frag",
        ] {
            assert!(
                ClusterOriginV1::parse(value, ACK).is_err(),
                "must refuse {value}"
            );
        }
    }

    #[test]
    fn a_wrong_acknowledgment_value_is_refused() {
        for value in [
            "",
            "true",
            "yes",
            "devnet",
            // Close but not it: one character short of the real hash.
            "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZB",
            TESTNET_GENESIS_HASH_FOR_TEST,
        ] {
            assert!(
                ClusterOriginV1::parse("https://api.devnet.solana.com/", Some(value)).is_err(),
                "must refuse acknowledgment {value:?}"
            );
        }
    }

    /// Testnet's genesis hash, used only to prove the acknowledgment is not a
    /// "some cluster hash" check.
    const TESTNET_GENESIS_HASH_FOR_TEST: &str = "4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY";

    #[test]
    fn mainnet_is_refused_by_url_by_acknowledgment_and_by_genesis_hash() {
        // 1. By host shape, with and without an acknowledgment.
        for value in [
            "https://api.mainnet-beta.solana.com/",
            "https://solana-mainnet.g.alchemy.example/v2/key",
            "https://mainnet.helius-rpc.example/?api-key=k",
        ] {
            for acknowledgment in [None, ACK] {
                let refusal = ClusterOriginV1::parse(value, acknowledgment)
                    .err()
                    .unwrap_or_else(|| panic!("{value} must refuse"));
                assert!(
                    refusal.0.contains("mainnet"),
                    "the refusal must name mainnet, got {}",
                    refusal.0
                );
            }
        }
        // 2. By acknowledgment value: mainnet's own genesis hash does not
        //    unlock anything, even at a host that is not obviously mainnet.
        let refusal =
            ClusterOriginV1::parse("https://rpc.example.net/", Some(MAINNET_BETA_GENESIS_HASH))
                .err()
                .expect("must refuse");
        assert!(refusal.0.contains("refused unconditionally"));
        // 3. By the chain's own answer -- including on loopback, which is the
        //    tunnel case no URL rule can see.
        let loopback = ClusterOriginV1::parse("http://127.0.0.1:20890/", None).expect("loopback");
        assert!(
            loopback
                .authenticate_genesis(MAINNET_BETA_GENESIS_HASH)
                .is_err(),
            "a loopback port that answers with mainnet's genesis hash is a tunnel, not a test"
        );
        let devnet = ClusterOriginV1::parse("https://api.devnet.solana.com/", ACK).expect("devnet");
        assert!(
            devnet
                .authenticate_genesis(MAINNET_BETA_GENESIS_HASH)
                .is_err()
        );
        assert!(devnet.authenticate_genesis(DEVNET_GENESIS_HASH).is_ok());
        assert!(
            devnet
                .authenticate_genesis(TESTNET_GENESIS_HASH_FOR_TEST)
                .is_err(),
            "an acknowledged devnet origin that answers as testnet is some other chain"
        );
        // A fresh local ledger has a genesis hash nobody can predict, so
        // loopback compares against nothing -- except mainnet, above.
        assert!(
            loopback
                .authenticate_genesis("11111111111111111111111111111111")
                .is_ok()
        );
    }

    #[test]
    fn the_acknowledgment_on_a_loopback_origin_is_a_mistake_not_a_no_op() {
        let refusal = ClusterOriginV1::parse("http://127.0.0.1:20890/", ACK)
            .err()
            .expect("must refuse");
        assert!(refusal.0.contains("one of the two is a mistake"));
    }

    #[test]
    fn a_devnet_origin_grants_no_loopback_affordance() {
        let devnet = ClusterOriginV1::parse("https://api.devnet.solana.com/", ACK).expect("devnet");
        assert!(!devnet.may_launch_validator());
        assert!(!seeded_keys_admissible(devnet.url()));
        assert!(!devnet.may_airdrop());
        assert_eq!(devnet.loopback_port(), None);
        assert_eq!(devnet.label(), "devnet");
        assert!(devnet.pacing().minimum_call_interval > Duration::ZERO);

        let loopback = ClusterOriginV1::parse("http://127.0.0.1:20890/", None).expect("loopback");
        assert!(loopback.may_launch_validator());
        assert!(seeded_keys_admissible(loopback.url()));
        assert!(loopback.may_airdrop());
        assert_eq!(loopback.loopback_port(), Some(20890));
        assert_eq!(loopback.pacing().minimum_call_interval, Duration::ZERO);
    }

    #[test]
    fn no_acknowledged_origin_can_ever_admit_a_seed() {
        // The seed set is the wider one, and deliberately so.
        for value in [
            "http://127.0.0.1:20890/",
            "http://localhost:20890/",
            "http://[::1]:20890/",
        ] {
            assert!(seeded_keys_admissible(value), "{value}");
        }
        // Only the first of those is an origin the supervisor will accept...
        assert!(ClusterOriginV1::parse("http://127.0.0.1:20890/", None).is_ok());
        assert!(ClusterOriginV1::parse("http://localhost:20890/", None).is_err());
        // ...and no URL that reaches the acknowledged-devnet variant is in the
        // seed set, because the acknowledgment is refused on loopback outright.
        for value in [
            "https://api.devnet.solana.com/",
            "https://rpc.example.net/v2/key",
        ] {
            assert!(
                ClusterOriginV1::parse(value, ACK).is_ok(),
                "{value} must be an admissible devnet origin"
            );
            assert!(
                !seeded_keys_admissible(value),
                "{value} must never admit a reproducible private key"
            );
        }
    }

    #[test]
    fn a_provider_key_never_reaches_evidence() {
        let keyed = ClusterOriginV1::parse("https://rpc.example.net/?api-key=SECRET", ACK)
            .expect("keyed provider endpoint");
        let redacted = keyed.redacted_url();
        assert!(!redacted.contains("SECRET"), "got {redacted}");
        assert_eq!(redacted, "https://rpc.example.net/<redacted>");
        let path_keyed = ClusterOriginV1::parse("https://rpc.example.net/v2/SECRET", ACK)
            .expect("keyed provider path");
        assert!(!path_keyed.redacted_url().contains("SECRET"));
        // An unkeyed endpoint is shown whole; there is nothing to hide and a
        // redaction marker on it would be a lie about the shape of the URL.
        let plain = ClusterOriginV1::parse("https://api.devnet.solana.com/", ACK).expect("devnet");
        assert_eq!(plain.redacted_url(), "https://api.devnet.solana.com/");
        // ...and the refusal path redacts too.
        let refusal = keyed
            .authenticate_genesis(MAINNET_BETA_GENESIS_HASH)
            .err()
            .expect("mainnet refusal");
        assert!(!refusal.0.contains("SECRET"), "got {}", refusal.0);
    }
}
