#![forbid(unsafe_code)]

//! `dclutch-ticket-board` — run one Direct ticket board.
//!
//! WHAT IT DOES. Accepts `POST /tickets`, lists `GET /tickets?market=&outcome=`,
//! and answers `GET /health`. Every ticket is admitted through the one shared
//! reader in `dclutch-direct-ticket`, signature included.
//!
//! WHAT IT CANNOT DO, structurally rather than by promise:
//!
//! - **Sign.** `dclutch-direct-ticket` is taken with `default-features = false`,
//!   so the `author` feature is off and no signer crate is linked in. This
//!   binary cannot mint a ticket because it does not contain the code to.
//! - **Submit.** It builds no transaction and talks to no cluster.
//! - **Forge.** A tampered field changes the signing preimage and the detached
//!   signature stops verifying, at admission.
//!
//! WHAT IT DOES NOT CHECK: chain state. It cannot tell whether the maker's
//! Position covers the offer, whether the generation is current, or whether the
//! fee rate matches the Market's immutable config. Those are decided against
//! finalized state by the code that builds the transaction. An offer here is
//! WELL-FORMED and CORRECTLY SIGNED, never "valid".
//!
//! NO AUTHENTICATION, on purpose. Tickets are bearer-signed, so a credential
//! would gate nothing that the signature does not already bind, while adding a
//! secret this service would then have to hold. See the README for the limits
//! that follow — rate limiting above all.

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{Arc, Mutex},
};

use dclutch_ticket_board::{
    board::{BoardStateV1, MAXIMUM_BODY_BYTES_V1},
    http::handle_v1,
    snapshot::{load_snapshot_v1, write_snapshot_v1},
};
use http_body_util::{BodyExt as _, Full, Limited};
use hyper::{Request, Response, body::Bytes, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

/// Where the board listens when nobody names an address.
///
/// LOOPBACK, deliberately. A board is reachable by anyone who can reach its
/// port and has no authentication and no rate limiting, so binding every
/// interface is a decision an operator makes explicitly with `--bind`, never a
/// default that arrives by surprise.
const DEFAULT_BIND_V1: &str = "127.0.0.1:8787";

/// The snapshot path used when nobody names one.
const DEFAULT_SNAPSHOT_V1: &str = "ticket-board-snapshot.json";

/// Everything the process was told to do.
struct ArgumentsV1 {
    bind: SocketAddr,
    snapshot: PathBuf,
    market: Option<String>,
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments
        .iter()
        .any(|argument| matches!(argument.as_str(), "-h" | "--help" | "help"))
    {
        print!("{}", usage());
        return ExitCode::SUCCESS;
    }
    match parse_arguments_v1(arguments).and_then(serve) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dclutch-ticket-board: {error}");
            ExitCode::FAILURE
        }
    }
}

fn parse_arguments_v1(arguments: Vec<String>) -> Result<ArgumentsV1, String> {
    let mut bind = DEFAULT_BIND_V1.to_owned();
    let mut snapshot = DEFAULT_SNAPSHOT_V1.to_owned();
    let mut market: Option<String> = None;

    let mut rest = arguments.into_iter();
    while let Some(flag) = rest.next() {
        if !matches!(flag.as_str(), "--bind" | "--snapshot" | "--market") {
            return Err(format!(
                "unknown argument `{flag}`. Run `dclutch-ticket-board --help`."
            ));
        }
        let Some(supplied) = rest.next() else {
            return Err(format!("`{flag}` needs a value"));
        };
        match flag.as_str() {
            "--bind" => bind = supplied,
            "--snapshot" => snapshot = supplied,
            _ => market = Some(supplied),
        }
    }

    if let Some(pinned) = market.as_deref()
        && let Err(error) = dclutch_direct_ticket::canonical_ticket_pubkey_v1(pinned, "`--market`")
    {
        return Err(error.to_string());
    }

    let bind: SocketAddr = bind
        .parse()
        .map_err(|error| format!("`--bind` is not one socket address: {error}"))?;
    Ok(ArgumentsV1 {
        bind,
        snapshot: PathBuf::from(snapshot),
        market,
    })
}

fn serve(arguments: ArgumentsV1) -> Result<(), String> {
    let mut state = BoardStateV1::new(arguments.market.clone());
    let load = load_snapshot_v1(&arguments.snapshot, &mut state)?;
    for refusal in &load.refused {
        eprintln!("dclutch-ticket-board: snapshot row refused and skipped: {refusal}");
    }

    println!("dclutch-ticket-board");
    println!("  listening        http://{}", arguments.bind);
    println!("  snapshot         {}", arguments.snapshot.display());
    println!(
        "  market           {}",
        arguments.market.as_deref().unwrap_or("every Market")
    );
    println!(
        "  restored         {} offers ({} snapshot rows refused)",
        load.restored,
        load.refused.len()
    );
    println!(
        "\n  This board holds no keys, takes no custody, and has no authority. It\n  \
         checks a ticket's shape and its signature; it reads no chain, so an offer\n  \
         listed here is WELL-FORMED, never verified. Only the chain verifies.\n"
    );

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("could not start the async runtime: {error}"))?;
    runtime.block_on(run_v1(arguments, state))
}

async fn run_v1(arguments: ArgumentsV1, state: BoardStateV1) -> Result<(), String> {
    let board = Arc::new(Mutex::new(state));
    let snapshot = Arc::new(arguments.snapshot);
    let listener = TcpListener::bind(arguments.bind)
        .await
        .map_err(|error| format!("could not bind {}: {error}", arguments.bind))?;

    loop {
        let (stream, _peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            // One refused connection is not a reason to stop serving everyone.
            Err(error) => {
                eprintln!("dclutch-ticket-board: accept failed: {error}");
                continue;
            }
        };
        let board = Arc::clone(&board);
        let snapshot = Arc::clone(&snapshot);
        tokio::task::spawn(async move {
            let service = service_fn(move |request| {
                let board = Arc::clone(&board);
                let snapshot = Arc::clone(&snapshot);
                async move {
                    Ok::<_, std::convert::Infallible>(answer_v1(request, &board, &snapshot).await)
                }
            });
            if let Err(error) = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await
            {
                eprintln!("dclutch-ticket-board: connection ended: {error}");
            }
        });
    }
}

async fn answer_v1(
    request: Request<hyper::body::Incoming>,
    board: &Mutex<BoardStateV1>,
    snapshot: &Path,
) -> Response<Full<Bytes>> {
    let method = request.method().as_str().to_owned();
    let target = request
        .uri()
        .path_and_query()
        .map_or_else(|| request.uri().path().to_owned(), ToString::to_string);

    // Bounded BEFORE the body is read into memory: the limit is the ticket's
    // own 4096-byte bound, so an oversized body is refused rather than buffered.
    let body = match Limited::new(request.into_body(), MAXIMUM_BODY_BYTES_V1)
        .collect()
        .await
    {
        Ok(collected) => collected.to_bytes(),
        Err(_) => {
            return wire(
                413,
                format!(
                    "{{\"accepted\":false,\"refusal\":\"BODY_TOO_LARGE\",\"reason\":\"the request \
                     body is above the {MAXIMUM_BODY_BYTES_V1}-byte bound a Direct ticket has by \
                     its own codec\"}}"
                ),
            );
        }
    };

    let response = handle_v1(&method, &target, &body, board);
    // Persist only what changed the board, and never while holding its lock.
    if response.status == 201
        && let Err(error) = persist_v1(board, snapshot)
    {
        eprintln!("dclutch-ticket-board: snapshot not written: {error}");
    }
    wire(response.status, response.body)
}

fn persist_v1(board: &Mutex<BoardStateV1>, snapshot: &Path) -> Result<(), String> {
    let Ok(state) = board.lock() else {
        return Err("the board's state lock was poisoned by an earlier panic".into());
    };
    write_snapshot_v1(snapshot, &state)
}

fn wire(status: u16, body: String) -> Response<Full<Bytes>> {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        // A board is read by a browser on another origin; it serves only public
        // bearer artifacts and holds no cookie or credential, so there is
        // nothing for an origin check to protect here.
        .header("access-control-allow-origin", "*")
        .body(Full::new(Bytes::from(body)))
        .unwrap_or_else(|_| Response::new(Full::new(Bytes::from(String::new()))))
}

fn usage() -> String {
    format!(
        "dclutch-ticket-board — hold Direct intent tickets so a taker can find one.\n\
         \n\
         A Direct ticket is bearer-signed self-authenticating data, so a relay is a\n\
         permitted transport rather than a concession: it supplies candidates, and\n\
         the chain checks every signature at execution. This board holds no keys,\n\
         takes no custody, and has no authority. Losing it loses availability and\n\
         nothing else.\n\
         \n\
         ROUTES\n\
         \n\
         \x20 POST /tickets[?slot=SLOT]\n\
         \x20     Accept one ticket. The body is the ticket JSON, at most\n\
         \x20     {MAXIMUM_BODY_BYTES_V1} bytes. `slot` is the poster's own current\n\
         \x20     slot and is used only to refuse a ticket that is already expired.\n\
         \n\
         \x20 GET /tickets?market=PUBKEY[&outcome=U32][&slot=SLOT]\n\
         \x20     List live offers, newest first. `slot` drops offers whose validity\n\
         \x20     window has closed. Supply it: the board has no clock of its own.\n\
         \n\
         \x20 GET /health\n\
         \n\
         OPTIONS\n\
         \n\
         \x20 --bind ADDR        Default {DEFAULT_BIND_V1}. Loopback by default because\n\
         \x20                    this service has no authentication and no rate\n\
         \x20                    limiting; exposing it is an explicit act.\n\
         \x20 --snapshot PATH    Default {DEFAULT_SNAPSHOT_V1}. Written after every\n\
         \x20                    accepted post, re-validated row by row on load.\n\
         \x20 --market PUBKEY    Serve exactly one Market and refuse every other with\n\
         \x20                    MARKET_NOT_SERVED. Default: every Market.\n\
         \n\
         WHAT IT DOES NOT CHECK\n\
         \n\
         \x20 Chain state. It cannot see whether the maker's Position covers the\n\
         \x20 offer, whether the generation is current, or whether the fee rate\n\
         \x20 matches the Market's immutable config. An offer listed here is\n\
         \x20 WELL-FORMED and CORRECTLY SIGNED. It is not valid, and this service\n\
         \x20 never says it is.\n"
    )
}
