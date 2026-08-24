//! `operatord` — the chain-derived Dragon's Clutch operator daemon.
//!
//! The browser never builds a transaction and this process contains no wallet
//! or private-key loading path. `chain-serve` discovers finalized state through
//! an explicit genesis/release/decoder configuration; `launch-local-chain`
//! starts the same release-bound surface on local infrastructure. Historical
//! mock, retained-transcript, and synthetic-source session modes are
//! deliberately absent from this binary.

mod bus;
mod chain_server;
mod compose_chain_config;
mod devnet_deployment;
mod http;
mod index_api;
mod local_validator_launcher;
mod payoff_compiler;
mod processed_ws;

use std::path::PathBuf;
use std::{env, process};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Where this crate sits, so the daemon works from any working directory.
const CRATE_DIR: &str = env!("CARGO_MANIFEST_DIR");

pub(crate) fn repo_path(relative: &str) -> PathBuf {
    std::path::Path::new(CRATE_DIR)
        .join("../../..")
        .join(relative)
}

fn usage() -> ! {
    eprintln!(
        "usage:\n  \
         operatord compiler-serve --compiler-release-sha256 HASH [--port N] [--static DIR]\n  \
         operatord compile-product-exact-market --compiler-release-sha256 HASH < request.json\n  \
         operatord chain-serve --config FILE [--port N] [--static DIR]\n  \
         operatord compose-chain-config --local-release-manifest FILE --capability-manifest FILE \
         --cluster-name NAME --expected-genesis HASH --rpc-http-url URL --rpc-websocket-url URL\n  \
         operatord prepare-local-chain --config FILE --capability-manifest FILE\n  \
         operatord launch-local-chain --config FILE --capability-manifest FILE [--port N] [--static DIR]\n  \
         operatord compose-devnet-chain-config --deployment-manifest FILE \
         --capability-manifest FILE --built-elf FILE"
    );
    process::exit(2)
}

fn parse_devnet_compose(
    mut args: impl Iterator<Item = String>,
) -> Result<devnet_deployment::ComposeDevnetOptions> {
    let mut deployment_manifest = None;
    let mut capability_manifest = None;
    let mut built_elf = None;
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--deployment-manifest" => deployment_manifest = Some(PathBuf::from(value()?)),
            "--capability-manifest" => capability_manifest = Some(PathBuf::from(value()?)),
            "--built-elf" => built_elf = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown compose-devnet-chain-config flag {other}").into()),
        }
    }
    Ok(devnet_deployment::ComposeDevnetOptions {
        deployment_manifest: deployment_manifest
            .ok_or("compose-devnet-chain-config requires --deployment-manifest FILE")?,
        capability_manifest: capability_manifest
            .ok_or("compose-devnet-chain-config requires --capability-manifest FILE")?,
        built_elf: built_elf.ok_or("compose-devnet-chain-config requires --built-elf FILE")?,
    })
}

fn parse_local_launch(
    mut args: impl Iterator<Item = String>,
) -> Result<local_validator_launcher::LocalLaunchOptions> {
    let mut config = None;
    let mut capability_manifest = None;
    let mut server_port = 9130_u16;
    let mut statics = repo_path("apps/static-client");
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--config" => config = Some(PathBuf::from(value()?)),
            "--capability-manifest" => capability_manifest = Some(PathBuf::from(value()?)),
            "--port" => server_port = value()?.parse()?,
            "--static" => statics = PathBuf::from(value()?),
            other => return Err(format!("unknown local-chain flag {other}").into()),
        }
    }
    Ok(local_validator_launcher::LocalLaunchOptions {
        config: config.ok_or("local-chain command requires --config FILE")?,
        capability_manifest: capability_manifest
            .ok_or("local-chain command requires --capability-manifest FILE")?,
        server_port,
        statics,
    })
}

struct CompilerServeOptions {
    port: u16,
    statics: PathBuf,
    compiler_release_sha256: String,
}

fn parse_compiler_serve(mut args: impl Iterator<Item = String>) -> Result<CompilerServeOptions> {
    let mut port = 9130_u16;
    let mut statics = repo_path("apps/static-client");
    let mut compiler_release_sha256 = None;
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--port" => port = value()?.parse()?,
            "--static" => statics = PathBuf::from(value()?),
            "--compiler-release-sha256" => compiler_release_sha256 = Some(value()?),
            other => return Err(format!("unknown compiler-serve flag {other}").into()),
        }
    }
    Ok(CompilerServeOptions {
        port,
        statics,
        compiler_release_sha256: compiler_release_sha256
            .ok_or("compiler-serve requires --compiler-release-sha256 HASH")?,
    })
}

fn parse_compiler_release(mut args: impl Iterator<Item = String>) -> Result<String> {
    let Some(flag) = args.next() else {
        return Err(
            "compile-product-exact-market requires --compiler-release-sha256 HASH".into(),
        );
    };
    if flag != "--compiler-release-sha256" {
        return Err(format!("unknown compile-product-exact-market flag {flag}").into());
    }
    let release = args
        .next()
        .ok_or("--compiler-release-sha256 needs a value")?;
    if let Some(extra) = args.next() {
        return Err(format!("unexpected compile-product-exact-market argument {extra}").into());
    }
    Ok(release)
}

struct ChainServeOptions {
    port: u16,
    statics: PathBuf,
    config: PathBuf,
}

fn parse_compose_chain_config(
    mut args: impl Iterator<Item = String>,
) -> Result<compose_chain_config::ComposeOptions> {
    let mut local_release_manifest = None;
    let mut capability_manifest = None;
    let mut cluster_name = None;
    let mut expected_genesis = None;
    let mut rpc_http_url = None;
    let mut rpc_websocket_url = None;
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--local-release-manifest" => local_release_manifest = Some(PathBuf::from(value()?)),
            "--capability-manifest" => capability_manifest = Some(PathBuf::from(value()?)),
            "--cluster-name" => cluster_name = Some(value()?),
            "--expected-genesis" => expected_genesis = Some(value()?),
            "--rpc-http-url" => rpc_http_url = Some(value()?),
            "--rpc-websocket-url" => rpc_websocket_url = Some(value()?),
            other => return Err(format!("unknown compose-chain-config flag {other}").into()),
        }
    }
    Ok(compose_chain_config::ComposeOptions {
        local_release_manifest: local_release_manifest
            .ok_or("compose-chain-config requires --local-release-manifest FILE")?,
        capability_manifest: capability_manifest
            .ok_or("compose-chain-config requires --capability-manifest FILE")?,
        cluster_name: cluster_name.ok_or("compose-chain-config requires --cluster-name NAME")?,
        expected_genesis: expected_genesis
            .ok_or("compose-chain-config requires --expected-genesis HASH")?,
        rpc_http_url: rpc_http_url.ok_or("compose-chain-config requires --rpc-http-url URL")?,
        rpc_websocket_url: rpc_websocket_url
            .ok_or("compose-chain-config requires --rpc-websocket-url URL")?,
    })
}

fn parse_chain_serve(mut args: impl Iterator<Item = String>) -> Result<ChainServeOptions> {
    let mut port = 9130_u16;
    let mut statics = repo_path("apps/static-client");
    let mut config = None;
    while let Some(flag) = args.next() {
        let mut value = || args.next().ok_or_else(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--port" => port = value()?.parse()?,
            "--static" => statics = PathBuf::from(value()?),
            "--config" => config = Some(PathBuf::from(value()?)),
            other => return Err(format!("unknown chain-serve flag {other}").into()),
        }
    }
    Ok(ChainServeOptions {
        port,
        statics,
        config: config.ok_or("chain-serve requires --config FILE")?,
    })
}

fn main() {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else { usage() };
    let outcome = match command.as_str() {
        "compiler-serve" => parse_compiler_serve(args).and_then(|options| {
            payoff_compiler::serve(
                options.port,
                options.statics,
                options.compiler_release_sha256,
            )
        }),
        "compile-product-exact-market" => {
            parse_compiler_release(args).and_then(payoff_compiler::compile_cli)
        }
        "chain-serve" => parse_chain_serve(args).and_then(|options| {
            chain_server::serve(options.port, options.statics, &options.config)
        }),
        "compose-chain-config" => parse_compose_chain_config(args).and_then(|options| {
            print!("{}", compose_chain_config::compose(&options)?);
            Ok(())
        }),
        "prepare-local-chain" => parse_local_launch(args).and_then(|options| {
            print!("{}", local_validator_launcher::prepare_only(&options)?);
            Ok(())
        }),
        "launch-local-chain" => parse_local_launch(args)
            .and_then(|options| local_validator_launcher::launch_and_serve(&options)),
        "compose-devnet-chain-config" => parse_devnet_compose(args).and_then(|options| {
            print!("{}", devnet_deployment::compose(&options)?);
            Ok(())
        }),
        _ => usage(),
    };
    if let Err(error) = outcome {
        eprintln!("operatord: {error}");
        process::exit(1);
    }
}
