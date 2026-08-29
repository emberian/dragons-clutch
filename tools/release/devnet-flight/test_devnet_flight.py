import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


MODULE = Path(__file__).with_name("devnet_flight.py")
SPEC = importlib.util.spec_from_file_location("devnet_flight", MODULE)
flight = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(flight)


def command(ident, mutation=False):
    argv = ["fake", ident]
    if mutation:
        argv += ["--i-mean-devnet", flight.DEVNET_GENESIS, "--i-accept-test", "accepted", "--execute"]
    return {"id": ident, "mutation": mutation, "argv": argv}


def fixture():
    commands = [command(ident, ident not in {"candidate", "reconcile"}) for ident in flight.REQUIRED]
    for row in commands:
        if row["id"].startswith("buffer:"):
            row["argv"].append("--stop-after-buffer-ready")
    return {"schema": flight.SCHEMA, "target": {"cluster": "devnet", "genesis": flight.DEVNET_GENESIS}, "commands": commands}


class Result:
    def __init__(self, code): self.returncode = code


class FlightTests(unittest.TestCase):
    def write_fixture(self, root):
        source = root / "flight.json"
        source.write_text(json.dumps(fixture()))
        return source

    def test_resume_skips_finalized_command(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, journal = self.write_fixture(root), root / "journal.json"
            document = fixture(); commands, _ = flight.validate_flight(document)
            state = flight.journal_for(journal, source, document, commands)
            calls = []
            def first(argv, **kwargs):
                calls.append(argv); return Result(1 if len(calls) == 2 else 0)
            with self.assertRaises(flight.FlightError): flight.execute(commands, journal, state, first)
            state = flight.journal_for(journal, source, document, commands)
            calls = []
            def second(argv, **kwargs): calls.append(argv); return Result(0)
            flight.execute(commands, journal, state, second)
            self.assertNotEqual(calls[0][1], "candidate")
            self.assertEqual(json.loads(journal.read_text())["commands"][-1]["state"], "finalized")

    def test_journal_is_written_before_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); source, journal = self.write_fixture(root), root / "journal.json"
            document = fixture(); commands, _ = flight.validate_flight(document)
            state = flight.journal_for(journal, source, document, commands)
            observed = []
            def fake(argv, **kwargs):
                observed.append(json.loads(journal.read_text())["events"][-1]["event"]); return Result(0)
            flight.execute(commands[1:2], journal, state, fake)
            self.assertEqual(observed[-1], "before-external-mutation")

    def test_buffer_requires_existing_boundary(self):
        document = fixture()
        document["commands"][1]["argv"].remove("--stop-after-buffer-ready")
        with self.assertRaisesRegex(flight.FlightError, "buffer:custody"):
            flight.validate_flight(document)

    def test_extension_is_immediately_before_its_role_buffer(self):
        document = fixture()
        extension = command("extend:resolution", True)
        document["commands"].insert(3, extension)
        flight.validate_flight(document)
        document["commands"].pop(3)
        document["commands"].insert(1, extension)
        with self.assertRaisesRegex(flight.FlightError, "per-role"):
            flight.validate_flight(document)


if __name__ == "__main__":
    unittest.main()
