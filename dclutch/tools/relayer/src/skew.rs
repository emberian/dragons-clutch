//! Cross-cluster clock skew measurement.
//!
//! §6.3 item 9: sample both clusters' `Clock.unix_timestamp` over a bounded
//! period and report the maximum observed `|a_now - b_now|`.  That number is
//! what justifies `RelayedAdapterConfigV1.max_cluster_skew_seconds` and retires
//! its *provisional* label; until it has been measured against the clusters a
//! release actually spans, the bound stays provisional and this subcommand's
//! output is the only thing that can change that.
//!
//! **Read-only.** This signs nothing, submits nothing and writes nothing to any
//! cluster.  Every call it makes is recorded in the RPC read log, and the
//! sample count is bounded so an operator cannot accidentally point a poll loop
//! at a public cluster.

use std::time::Duration;

use crate::chain::{CLOCK_ACCOUNT_BYTES, CLOCK_SYSVAR_ID, clock_unix_timestamp};
use crate::error::{RelayerError, Result};
use crate::publog::wall_unix_seconds;
use crate::rpc::RpcClient;

/// Largest admitted sample count.
///
/// Bounded on purpose: `AGENTS.md` requires public RPC reads to be explicit and
/// bounded, and a skew measurement that can be asked for ten thousand samples
/// is a poll loop with a different name.
pub const MAX_SAMPLES: u32 = 256;

/// One paired reading.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SkewSample {
    /// Zero-based sample index.
    pub index: u32,
    /// Cluster A's `Clock.unix_timestamp`.
    pub a_unix_seconds: i64,
    /// The slot A's read was taken at.
    pub a_slot: u64,
    /// Cluster B's `Clock.unix_timestamp`.
    pub b_unix_seconds: i64,
    /// The slot B's read was taken at.
    pub b_slot: u64,
    /// `a_unix_seconds - b_unix_seconds`.
    pub delta_seconds: i64,
}

/// A completed measurement.
#[derive(Clone, Debug)]
pub struct SkewReport {
    /// Host of cluster A.
    pub a_host: String,
    /// Host of cluster B.
    pub b_host: String,
    /// Every paired reading.
    pub samples: Vec<SkewSample>,
    /// The maximum observed absolute skew.
    pub max_abs_delta_seconds: u64,
    /// Wall time the measurement finished.
    pub wall_unix_seconds: u64,
}

impl SkewReport {
    /// Render the report as JSON.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "schema": "dclutch.relayer.clock-skew.v1",
            "a_endpoint_host": self.a_host,
            "b_endpoint_host": self.b_host,
            "sample_count": self.samples.len(),
            "max_abs_delta_seconds": self.max_abs_delta_seconds,
            "wall_unix_seconds": self.wall_unix_seconds,
            "bound_label": "measured-profile",
            "note":
                "this is the measured maximum over the sampled window only; it bounds nothing \
                 outside it, and max_cluster_skew_seconds stays provisional until a release \
                 states the window this was measured over",
            "samples": self.samples.iter().map(|sample| serde_json::json!({
                "index": sample.index,
                "a_unix_seconds": sample.a_unix_seconds,
                "a_slot": sample.a_slot,
                "b_unix_seconds": sample.b_unix_seconds,
                "b_slot": sample.b_slot,
                "delta_seconds": sample.delta_seconds,
            })).collect::<Vec<serde_json::Value>>(),
        })
    }
}

/// Read one cluster's `Clock.unix_timestamp`.
pub async fn read_clock(rpc: &RpcClient) -> Result<(i64, u64)> {
    let page = rpc
        .get_account_page(
            &CLOCK_SYSVAR_ID,
            0,
            u64::try_from(CLOCK_ACCOUNT_BYTES).unwrap_or(40),
            None,
        )
        .await?;
    let account = page
        .account
        .ok_or_else(|| RelayerError::MalformedRpcResponse {
            endpoint: rpc.host().to_owned(),
            method: "getAccountInfo".to_owned(),
            reason: "the Clock sysvar does not exist on this cluster".to_owned(),
        })?;
    let timestamp =
        clock_unix_timestamp(&account.data).ok_or_else(|| RelayerError::MalformedRpcResponse {
            endpoint: rpc.host().to_owned(),
            method: "getAccountInfo".to_owned(),
            reason: format!(
                "the Clock sysvar returned {} bytes where {CLOCK_ACCOUNT_BYTES} were expected",
                account.data.len()
            ),
        })?;
    Ok((timestamp, page.slot))
}

/// Sample both clusters and report the maximum observed absolute skew.
pub async fn measure_skew(
    a: &RpcClient,
    b: &RpcClient,
    samples: u32,
    interval: Duration,
) -> Result<SkewReport> {
    if samples == 0 || samples > MAX_SAMPLES {
        return Err(RelayerError::config(format!(
            "samples must be between 1 and {MAX_SAMPLES}"
        )));
    }
    let mut readings = Vec::with_capacity(usize::try_from(samples).unwrap_or(0));
    let mut max_abs = 0u64;
    for index in 0..samples {
        if index > 0 && !interval.is_zero() {
            tokio::time::sleep(interval).await;
        }
        let (a_unix, a_slot) = read_clock(a).await?;
        let (b_unix, b_slot) = read_clock(b).await?;
        let delta = a_unix.saturating_sub(b_unix);
        max_abs = max_abs.max(delta.unsigned_abs());
        readings.push(SkewSample {
            index,
            a_unix_seconds: a_unix,
            a_slot,
            b_unix_seconds: b_unix,
            b_slot,
            delta_seconds: delta,
        });
    }
    Ok(SkewReport {
        a_host: a.host().to_owned(),
        b_host: b.host().to_owned(),
        samples: readings,
        max_abs_delta_seconds: max_abs,
        wall_unix_seconds: wall_unix_seconds(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn client(url: &str) -> RpcClient {
        RpcClient::new(url, Duration::from_millis(1), None).expect("client")
    }

    #[tokio::test]
    async fn an_unbounded_sample_count_refuses_before_any_read() {
        let a = client("http://127.0.0.1:1");
        let b = client("http://127.0.0.1:2");
        assert!(
            measure_skew(&a, &b, 0, Duration::ZERO).await.is_err(),
            "zero samples was admitted"
        );
        assert!(
            measure_skew(&a, &b, MAX_SAMPLES + 1, Duration::ZERO)
                .await
                .is_err(),
            "an unbounded sample count was admitted"
        );
    }

    #[test]
    fn the_report_renders_its_maximum_and_labels_its_bound() {
        let report = SkewReport {
            a_host: "a".to_owned(),
            b_host: "b".to_owned(),
            samples: vec![SkewSample {
                index: 0,
                a_unix_seconds: 1_772_000_010,
                a_slot: 1,
                b_unix_seconds: 1_772_000_000,
                b_slot: 2,
                delta_seconds: 10,
            }],
            max_abs_delta_seconds: 10,
            wall_unix_seconds: 0,
        };
        let json = report.to_json();
        assert_eq!(json["max_abs_delta_seconds"], 10);
        assert_eq!(json["bound_label"], "measured-profile");
        assert_eq!(json["samples"][0]["delta_seconds"], 10);
    }

    #[test]
    fn a_negative_skew_is_measured_by_magnitude() {
        let clock = {
            let mut bytes = [0u8; CLOCK_ACCOUNT_BYTES];
            bytes[32..].copy_from_slice(&(-5i64).to_le_bytes());
            bytes
        };
        assert_eq!(clock_unix_timestamp(&clock), Some(-5));
        assert_eq!((-5i64).unsigned_abs(), 5);
    }
}
