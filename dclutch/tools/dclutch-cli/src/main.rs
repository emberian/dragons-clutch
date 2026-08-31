#![forbid(unsafe_code)]

//! `dclutch` — read a dClutch market, and the capability root that decides
//! whether it can trade, straight off a Solana cluster.
//!
//! WHAT THIS BINARY IS FOR. A dClutch market is a chain account. Its bytes are
//! the truth; everything else — the website, a screenshot, this paragraph — is
//! a rendering of them. This tool fetches those bytes over ordinary JSON-RPC
//! and hands them to the same decoders the on-chain programs use, so that a
//! stranger can check a market for themselves without trusting our website.
//!
//! WHAT IT IS NOT. It never submits a transaction and never writes to a
//! cluster. Every command that touches a CLUSTER here is a read, and takes no
//! credential of any kind.
//!
//! THE TWO EXCEPTIONS, stated where they cannot be missed, because neither is a
//! cluster write and both would otherwise look like one:
//!
//! - `dclutch ticket author` opens ONE key file — named by an environment
//!   variable, never by a flag — to sign one Direct intent into a portable
//!   ticket on local disk. A local authoring act with no network in it at all.
//! - `dclutch ticket post` sends one such ticket to an offer BOARD, which is a
//!   relay and not a cluster. It builds no transaction and moves no lamport. A
//!   board can lose or hide an offer; it cannot change one, because every field
//!   is covered by the maker's signature.
//!
//! Authoring, publishing, and submitting are three separate acts. This binary
//! does the first two and never the third.
//!
//! SINGLE AUTHORSHIP, which is why this file is short. Three crates own every
//! byte interpreted here:
//!
//! - `dclutch_market_core_codec::CoreState` — the Market Core account, emitted
//!   from `formal/dclutch-semantics/EmitMarketCoreRust.lean`.
//! - `dclutch_capability_program_contract::CapabilityRootHeaderV1` — the
//!   immutable activation projection at the front of a Trading root account.
//! - `dclutch_direct_codec::successor::DirectRootStateV1` — the Direct family
//!   tail behind that header.
//!
//! This crate calls `decode` on each of them and prints what comes back. It
//! does not know a single field offset, and a decoder that refuses is reported
//! as a refusal rather than smoothed over. If a rendering here disagrees with
//! the chain, the decoder is wrong for everyone, not just for this tool.

use std::{fmt, process::ExitCode};

mod capability;
mod market;
mod rpc;
mod ticket;

/// A refusal, carrying the sentence the operator should read.
#[derive(Debug)]
pub struct Error(String);

impl Error {
    /// Build one refusal from anything that can name itself.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

/// This crate's result.
pub type Result<T> = core::result::Result<T, Error>;

/// The cluster this tool reads when nobody names one.
///
/// dClutch is deployed on Solana devnet and nowhere else. Devnet SOL has no
/// value and devnet state is wiped by the cluster's operators without notice,
/// which is exactly why this is a safe default: the worst outcome of running
/// the wrong command is reading the wrong test chain.
pub const DEFAULT_RPC_URL_V1: &str = "https://api.devnet.solana.com";

/// The environment variable that overrides the cluster.
///
/// An RPC endpoint URL is frequently a credential — a provider embeds the API
/// key in the path — so this is an environment variable and a flag, and every
/// refusal in this binary prints the endpoint's ORIGIN and never its path.
pub const RPC_URL_ENV_V1: &str = "DCLUTCH_RPC_URL";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    match run(arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dclutch: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<String>) -> Result<()> {
    let (head, rest) = match arguments.split_first() {
        None => {
            print!("{}", usage());
            return Ok(());
        }
        Some((head, rest)) => (head.as_str(), rest.to_vec()),
    };

    match head {
        "help" | "-h" | "--help" => {
            print!("{}", usage());
            Ok(())
        }
        "--version" | "-V" | "version" => {
            println!("dclutch {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "market" => market::run(rest),
        "capability" => capability::run(rest),
        "ticket" => ticket::run(rest),
        other => Err(Error::new(format!(
            "unknown command `{other}`. Run `dclutch --help` for the four it knows."
        ))),
    }
}

/// The whole surface, in one screen, in the order a reader needs it.
#[must_use]
pub fn usage() -> String {
    format!(
        "dclutch {version} — read dClutch markets off a Solana cluster.\n\
         \n\
         This tool never submits a transaction. Reading takes no credential at\n\
         all; `ticket author` opens one key file, named by an environment\n\
         variable, and signs one intent onto local disk — nothing else here\n\
         touches a key. dClutch runs on Solana DEVNET, where money is not money.\n\
         \n\
         COMMANDS\n\
         \n\
         \x20 dclutch market show <ADDRESS>\n\
         \x20     Fetch a Market Core account and print every field it carries:\n\
         \x20     its phase, whether it has been answered, and the identities it\n\
         \x20     is bound to.\n\
         \n\
         \x20 dclutch market decode (--base64 <DATA> | --file <PATH> | -)\n\
         \x20     The same rendering, from bytes you already have. No network.\n\
         \n\
         \x20 dclutch capability show <ADDRESS>\n\
         \x20     Fetch a Trading capability root — the account that decides\n\
         \x20     whether a market can actually execute a trade — and print the\n\
         \x20     activation it was born with plus its family tail.\n\
         \n\
         \x20 dclutch capability decode (--base64 <DATA> | --file <PATH> | -)\n\
         \x20     The same rendering, from bytes you already have. No network.\n\
         \n\
         \x20 dclutch ticket author --keypair-env VAR ...\n\
         \x20     {ticket_line} Signs with a local key file named by an\n\
         \x20     environment variable, and writes bytes identical to the ones\n\
         \x20     the browser trade panel signs. Submits nothing: run\n\
         \x20     `dclutch ticket` for the arguments and for where it goes next.\n\
         \n\
         \x20 dclutch ticket verify <PATH>\n\
         \x20     Check a ticket's signature and print every field it binds.\n\
         \n\
         \x20 dclutch ticket post --board URL <PATH | AUTHOR ARGUMENTS>\n\
         \x20     Publish a ticket to an offer board so a taker can find it.\n\
         \x20     A board is a relay, not a cluster: nothing is submitted.\n\
         \x20     No key, no network.\n\
         \n\
         OPTIONS (on the `show` commands)\n\
         \n\
         \x20 --rpc <URL>   Cluster to read. Default {default_rpc}.\n\
         \x20               Also settable as {rpc_env}. An endpoint URL is often\n\
         \x20               a credential, so refusals print its origin only.\n\
         \x20 --json        Machine-readable output instead of prose.\n\
         \n\
         dClutch is free software under the GNU AGPL v3 or later. The source for\n\
         this binary, and for every program it reads, is at\n\
         https://github.com/emberian/dragons-clutch\n",
        version = env!("CARGO_PKG_VERSION"),
        ticket_line = ticket::ONE_LINE_STATUS_V1,
        default_rpc = DEFAULT_RPC_URL_V1,
        rpc_env = RPC_URL_ENV_V1,
    )
}

/// Render 32 bytes as the base58 address a cluster would show.
#[must_use]
pub fn address(bytes: [u8; 32]) -> String {
    solana_program::pubkey::Pubkey::new_from_array(bytes).to_string()
}

/// Parse one base58 address, naming the argument that carried it.
pub fn parse_address(value: &str, label: &str) -> Result<[u8; 32]> {
    value
        .parse::<solana_program::pubkey::Pubkey>()
        .map(|pubkey| pubkey.to_bytes())
        .map_err(|_| Error::new(format!("{label} is not a base58 Solana address: `{value}`")))
}

/// Pull `--flag value` pairs and bare flags out of an argument list.
///
/// Deliberately tiny and deliberately strict: an unrecognized argument is a
/// refusal, never a silently ignored typo that makes the caller read the wrong
/// account and believe it was the one they asked for.
pub struct Arguments {
    positional: Vec<String>,
    rpc: Option<String>,
    json: bool,
    base64: Option<String>,
    file: Option<String>,
    stdin: bool,
}

/// Debug is written by hand, not derived, because `rpc` frequently holds an
/// API key. A derived `Debug` would put that key into any panic message, test
/// failure, or log line that ever formatted this struct.
impl fmt::Debug for Arguments {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Arguments")
            .field("positional", &self.positional)
            .field("rpc", &self.rpc.as_deref().map(rpc::origin))
            .field("json", &self.json)
            .field("base64", &self.base64.as_ref().map(|value| value.len()))
            .field("file", &self.file)
            .field("stdin", &self.stdin)
            .finish()
    }
}

impl Arguments {
    /// Parse, refusing anything not named here.
    pub fn parse(arguments: Vec<String>) -> Result<Self> {
        let mut parsed = Self {
            positional: Vec::new(),
            rpc: None,
            json: false,
            base64: None,
            file: None,
            stdin: false,
        };
        let mut iterator = arguments.into_iter();
        while let Some(argument) = iterator.next() {
            match argument.as_str() {
                "--json" => parsed.json = true,
                "-" => parsed.stdin = true,
                "--rpc" => parsed.rpc = Some(Self::value(&mut iterator, "--rpc")?),
                "--base64" => parsed.base64 = Some(Self::value(&mut iterator, "--base64")?),
                "--file" => parsed.file = Some(Self::value(&mut iterator, "--file")?),
                other if other.starts_with('-') => {
                    return Err(Error::new(format!(
                        "unknown option `{other}`. Run `dclutch --help`."
                    )));
                }
                other => parsed.positional.push(other.to_owned()),
            }
        }
        Ok(parsed)
    }

    fn value(iterator: &mut std::vec::IntoIter<String>, flag: &str) -> Result<String> {
        iterator
            .next()
            .ok_or_else(|| Error::new(format!("{flag} needs a value after it")))
    }

    /// The one positional argument this subcommand expects, named.
    pub fn one_positional(&self, label: &str) -> Result<&str> {
        match self.positional.as_slice() {
            [only] => Ok(only.as_str()),
            [] => Err(Error::new(format!("missing {label}"))),
            many => Err(Error::new(format!(
                "expected one {label}, got {}",
                many.len()
            ))),
        }
    }

    /// Whether the caller asked for machine-readable output.
    #[must_use]
    pub const fn json(&self) -> bool {
        self.json
    }

    /// The cluster to read: the flag, else the environment, else devnet.
    #[must_use]
    pub fn rpc_url(&self) -> String {
        self.rpc.clone().unwrap_or_else(|| {
            std::env::var(RPC_URL_ENV_V1).unwrap_or_else(|_| DEFAULT_RPC_URL_V1.to_owned())
        })
    }

    /// The account bytes a `decode` subcommand was handed, from exactly one of
    /// the three sources, refusing if the caller named none or several.
    pub fn offline_bytes(&self) -> Result<Vec<u8>> {
        let named = usize::from(self.base64.is_some())
            + usize::from(self.file.is_some())
            + usize::from(self.stdin);
        if named != 1 {
            return Err(Error::new(
                "name exactly one source of bytes: --base64 <DATA>, --file <PATH>, or - for stdin",
            ));
        }
        if let Some(encoded) = self.base64.as_deref() {
            return decode_base64(encoded.trim());
        }
        if let Some(path) = self.file.as_deref() {
            let raw = std::fs::read_to_string(path)
                .map_err(|error| Error::new(format!("cannot read {path}: {error}")))?;
            return decode_base64(raw.trim());
        }
        let mut raw = String::new();
        std::io::Read::read_to_string(&mut std::io::stdin(), &mut raw)
            .map_err(|error| Error::new(format!("cannot read stdin: {error}")))?;
        decode_base64(raw.trim())
    }
}

/// Decode base64 account data — the encoding `solana account` and every RPC
/// endpoint hand out — naming what was wrong when it is wrong.
pub fn decode_base64(encoded: &str) -> Result<Vec<u8>> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|error| Error::new(format!("account data is not valid base64: {error}")))
}

#[cfg(test)]
mod tests {
    use super::{Arguments, DEFAULT_RPC_URL_V1, address, parse_address, run, usage};

    #[test]
    fn usage_names_every_command_it_dispatches() {
        let text = usage();
        for command in ["market show", "market decode", "capability show", "ticket"] {
            assert!(text.contains(command), "usage never mentions `{command}`");
        }
    }

    #[test]
    fn usage_states_the_license_and_the_source() {
        let text = usage();
        assert!(text.contains("AGPL"));
        assert!(text.contains("https://github.com/emberian/dragons-clutch"));
    }

    #[test]
    fn usage_says_devnet_in_the_first_screen() {
        assert!(usage().contains("DEVNET"));
    }

    #[test]
    fn an_unknown_command_is_a_refusal_not_a_default() {
        let error = run(vec!["trade".to_owned()]).expect_err("must refuse");
        assert!(error.to_string().contains("unknown command `trade`"));
    }

    #[test]
    fn an_unknown_option_is_a_refusal_not_a_silent_typo() {
        let error = Arguments::parse(vec!["--jsonn".to_owned()]).expect_err("must refuse");
        assert!(error.to_string().contains("unknown option `--jsonn`"));
    }

    #[test]
    fn the_default_cluster_is_devnet() {
        // A flag beats the environment; with no flag and no environment the
        // default must be the cluster where the money is not money.
        let parsed = Arguments::parse(Vec::new()).expect("parses");
        assert!(parsed.rpc_url() == DEFAULT_RPC_URL_V1 || std::env::var("DCLUTCH_RPC_URL").is_ok());
        let flagged = Arguments::parse(vec![
            "--rpc".to_owned(),
            "https://example.invalid".to_owned(),
        ])
        .expect("parses");
        assert_eq!(flagged.rpc_url(), "https://example.invalid");
    }

    #[test]
    fn naming_no_byte_source_or_two_is_a_refusal() {
        let none = Arguments::parse(Vec::new()).expect("parses");
        assert!(none.offline_bytes().is_err());
        let two = Arguments::parse(vec![
            "--base64".to_owned(),
            "AA==".to_owned(),
            "-".to_owned(),
        ])
        .expect("parses");
        assert!(two.offline_bytes().is_err());
    }

    #[test]
    fn debug_never_prints_the_endpoint_path_where_the_api_key_lives() {
        let parsed = Arguments::parse(vec![
            "--rpc".to_owned(),
            "https://rpc.example.com/v1/SECRET-KEY-HERE".to_owned(),
        ])
        .expect("parses");
        let rendered = format!("{parsed:?}");
        assert!(!rendered.contains("SECRET-KEY-HERE"), "{rendered}");
        assert!(rendered.contains("https://rpc.example.com"), "{rendered}");
    }

    #[test]
    fn addresses_round_trip_through_the_cluster_encoding() {
        let bytes = [7_u8; 32];
        let rendered = address(bytes);
        assert_eq!(parse_address(&rendered, "test").expect("parses"), bytes);
    }
}
