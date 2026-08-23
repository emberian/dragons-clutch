import argparse
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "public_transcript.py"
SPEC = importlib.util.spec_from_file_location("public_transcript", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
public_transcript = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(public_transcript)


VALIDATOR_SHA256 = "ab" * 32
VALIDATOR_LOG_SHA256 = "cd" * 32


def raw_probe(socket_ip: str = "127.0.0.1") -> str:
    return f"""loopback listener probe: PASS
pid: 4321
binary: /Users/alice/work/validator/bin/solana-test-validator
rpc: 127.0.0.1:18537
websocket: 127.0.0.1:18538
faucet: 127.0.0.1:18539
non_loopback_addresses_tested: 10.0.0.7
tcp listeners:
solana-te 4321 alice 10u IPv4 0x111 0t0 TCP {socket_ip}:18537 (LISTEN)
solana-te 4321 alice 11u IPv4 0x112 0t0 TCP 127.0.0.1:18538 (LISTEN)
solana-te 4321 alice 12u IPv4 0x113 0t0 TCP 127.0.0.1:18539 (LISTEN)
solana-te 4321 alice 13u IPv4 0x114 0t0 TCP 127.0.0.1:18540 (LISTEN)
udp sockets:
solana-te 4321 alice 14u IPv4 0x115 0t0 UDP 127.0.0.1:18540
solana-te 4321 alice 15u IPv4 0x116 0t0 UDP 127.0.0.1:18541
solana-te 4321 alice 16u IPv4 0x117 0t0 UDP 127.0.0.1:18541
"""


class PublicTranscriptTests(unittest.TestCase):
    def build_args(self, work: Path, output: Path) -> argparse.Namespace:
        return argparse.Namespace(
            work=work,
            output=output,
            validator_sha256=VALIDATOR_SHA256,
            validator_log_sha256=VALIDATOR_LOG_SHA256,
            rpc_port="18537",
            websocket_port="18538",
            faucet_port="18539",
            gossip_port="18540",
            dynamic_port_range="18541-18640",
        )

    def seed_work(self, work: Path, probe: str | None = None) -> None:
        campaign = {
            "claim": public_transcript.CLAIM,
            "validator_binary": "/Users/alice/work/validator/bin/solana-test-validator",
            "validator_binary_sha256": VALIDATOR_SHA256,
        }
        result = {"claim": public_transcript.CLAIM, "terminal": {"all_zero": True}}
        (work / "campaign.json").write_text(json.dumps(campaign), encoding="utf-8")
        (work / "result.json").write_text(json.dumps(result), encoding="utf-8")
        for moment in ("before", "after"):
            (work / f"probe-{moment}.txt").write_text(
                probe if probe is not None else raw_probe(), encoding="utf-8"
            )

    def test_build_strips_local_identity_but_retains_exact_loopback_counts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            work = root / "work"
            output = root / "public"
            work.mkdir()
            self.seed_work(work)
            public_transcript.build_public_transcript(self.build_args(work, output))
            public_transcript.check_public_directory(output, username="alice")

            combined = b"\n".join((output / name).read_bytes() for name in public_transcript.PUBLIC_FILES)
            self.assertNotIn(b"/Users/", combined)
            self.assertNotIn(b"alice", combined.lower())
            self.assertNotIn(b"pid:", combined.lower())
            self.assertNotIn(b"10.0.0.7", combined)
            campaign = json.loads((output / "campaign.json").read_text(encoding="utf-8"))
            self.assertEqual(campaign["validator_binary"], "solana-test-validator")
            evidence = json.loads((output / "probe-evidence.json").read_text(encoding="utf-8"))
            self.assertEqual(evidence["selected_validator_sha256"], VALIDATOR_SHA256)
            self.assertEqual(
                evidence["probe_before_sha256"],
                evidence["probe_before"]["ephemeral_raw_sha256"],
            )
            self.assertEqual(evidence["probe_before"]["tcp_socket_observation_count"], 4)
            self.assertEqual(evidence["probe_before"]["udp_socket_observation_count"], 3)
            summary = (output / "probe-before.txt").read_text(encoding="utf-8")
            self.assertIn("tcp_endpoint: 127.0.0.1:18537 observations=1", summary)
            self.assertIn("udp_endpoint: 127.0.0.1:18541 observations=2", summary)
            self.assertIn("ephemeral_raw_probe_retained: false", summary)

    def test_checker_rejects_each_public_safety_class(self) -> None:
        hostile = {
            "home path": b'{"path":"/Users/alice/key.json"}',
            "username": b"operator: alice\n",
            "non-loopback IP": b"endpoint: 192.168.1.7:9000\n",
            "non-loopback IPv6": b"endpoint: [2001:db8::1]:9000\n",
            "PID/FD row": b"solana 4321 alice 10u IPv4 0x1 0t0 TCP 127.0.0.1:1\n",
            "secret field": b'{"keypair":[1,2,3]}',
            "private key": b"-----BEGIN PRIVATE KEY-----\n",
        }
        for role, body in hostile.items():
            with self.subTest(role=role):
                self.assertTrue(
                    public_transcript.public_safety_violations("hostile.json", body, "alice")
                )

    def test_probe_parser_refuses_a_non_loopback_socket_even_with_pass_header(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            probe = Path(temporary) / "probe.txt"
            probe.write_text(raw_probe("0.0.0.0"), encoding="utf-8")
            with self.assertRaises(public_transcript.PublicTranscriptError):
                public_transcript.parse_raw_probe(probe, 18537, 18538, 18539, 18540)

    def test_checker_refuses_tampered_summary_hash(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            work = root / "work"
            output = root / "public"
            work.mkdir()
            self.seed_work(work)
            public_transcript.build_public_transcript(self.build_args(work, output))
            with (output / "probe-before.txt").open("a", encoding="utf-8") as stream:
                stream.write("tcp_endpoint: 127.0.0.1:19999 observations=1\n")
            with self.assertRaises(public_transcript.PublicTranscriptError):
                public_transcript.check_public_directory(output, username="alice")

    def test_build_refuses_a_broken_target_symlink(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            work = root / "work"
            output = root / "public"
            work.mkdir()
            output.mkdir()
            self.seed_work(work)
            (output / "campaign.json").symlink_to(root / "absent-target")
            with self.assertRaises(public_transcript.PublicTranscriptError):
                public_transcript.build_public_transcript(self.build_args(work, output))


if __name__ == "__main__":
    unittest.main()
