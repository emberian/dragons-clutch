"""The runner must distinguish execution, refusal and an absent measurement.

Fake build tools exercise only orchestration. These are not protocol evidence;
real-ELF runs are recorded separately by the campaign.
"""
import csv
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
RUNNER = ROOT / 'programs/dclutch-claims-sbf/program-test/fractional-atomic/run-program-test.sh'
TOKEN_SHA = next(line.split('=', 1)[1] for line in
                 (ROOT / 'programs/dclutch-claims-sbf/fixtures/token-2022-v11.provenance').read_text().splitlines()
                 if line.startswith('canonical_elf_sha256='))


class FractionalRunnerTests(unittest.TestCase):
    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory(prefix='dclutch-runner-test-')
        self.addCleanup(self.scratch.cleanup)
        self.work = Path(self.scratch.name)
        self.bin = self.work / 'bin'
        self.bin.mkdir()
        self.token = self.work / 'token.so'
        self.token.write_text('fake artifact: runner test only')
        self.env = dict(os.environ, PATH=f'{self.bin}:{os.environ["PATH"]}',
                        TOKEN_2022_SO=str(self.token),
                        DCLUTCH_FRACTIONAL_ATOMIC_WORK=str(self.work / 'runs'),
                        RUNNER_TRACE=str(self.work / 'trace'), RUNNER_CASE='pass')
        self.env.pop('SBF_OUT_DIR', None)
        self.tool('swarm-build', 'exec "$@"\n')
        self.tool('shasum', f"printf '{TOKEN_SHA}  fixture\\n'\n")
        self.tool('cargo', r'''
printf '%s\n' "$*" >> "$RUNNER_TRACE"
case "$1" in
  check)
    if [ "$RUNNER_CASE" = scheduler ]; then
      echo 'Unit run-u99.scope was already loaded'; exit 0
    fi ;;
  build-sbf)
    case "$RUNNER_CASE" in
      frame-call) echo 'overwrites values in the frame'; exit 0 ;;
      frame-locals) echo 'overflows the maximum allowed frame space'; exit 0 ;;
    esac
    while [ "$#" -gt 0 ]; do
      if [ "$1" = --sbf-out-dir ]; then mkdir -p "$2"; touch "$2/fixture.so"; break; fi
      shift
    done ;;
  test)
    target=''
    while [ "$#" -gt 0 ]; do
      if [ "$1" = --test ]; then target="$2"; break; fi
      shift
    done
    if [ "$RUNNER_CASE" = first-fails ] && [ "$target" = claims_founding ]; then
      echo 'test result: FAILED. 0 passed; 1 failed;'; exit 101
    fi
    if [ "$RUNNER_CASE" = empty ]; then
      echo 'test result: ok. 0 passed; 0 failed; 0 ignored;'; exit 0
    fi
    echo 'test result: ok. 1 passed; 0 failed; 0 ignored;' ;;
  *) exit 99 ;;
esac
''')

    def tool(self, name, body):
        path = self.bin / name
        path.write_text('#!/usr/bin/env bash\nset -eu\n' + body)
        path.chmod(0o755)

    def run_case(self, case='pass', *args):
        self.env['RUNNER_CASE'] = case
        return subprocess.run(['bash', str(RUNNER), *args], env=self.env,
                              cwd=ROOT, capture_output=True, text=True, check=False)

    def rows(self):
        paths = list((self.work / 'runs').glob('run.*/results.tsv'))
        self.assertEqual(len(paths), 1)
        with paths[0].open() as stream:
            return list(csv.DictReader(stream, delimiter='\t'))

    def test_selection_runs_only_the_named_targets_serially(self):
        result = self.run_case('pass', '--test', 'claims_founding', '--test', 'claims_conservation')
        self.assertEqual(result.returncode, 0, result.stderr)
        rows = self.rows()
        self.assertEqual([r['target'] for r in rows], ['claims_founding', 'claims_conservation'])
        self.assertEqual([r['verdict'] for r in rows], ['passed', 'passed'])
        trace = (self.work / 'trace').read_text().splitlines()
        self.assertTrue(trace[0].startswith('check --locked -p '))
        tests = [line for line in trace if line.startswith('test ')]
        self.assertEqual(len(tests), 2)
        self.assertTrue(all('--test-threads=1' in line for line in tests))

    def test_a_failed_target_does_not_suppress_later_targets(self):
        result = self.run_case('first-fails', '--test', 'claims_founding', '--test', 'claims_conservation')
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual([r['verdict'] for r in self.rows()], ['failed', 'passed'])
        self.assertEqual([r['exit'] for r in self.rows()], ['101', '0'])

    def test_zero_tests_with_exit_zero_is_not_a_pass(self):
        result = self.run_case('empty', '--test', 'claims_conservation')
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(self.rows()[0]['verdict'], 'failed')

    def test_both_frame_diagnostics_prevent_execution(self):
        for case in ('frame-call', 'frame-locals'):
            with self.subTest(case=case):
                self.env['DCLUTCH_FRACTIONAL_ATOMIC_WORK'] = str(self.work / case)
                result = self.run_case(case, '--test', 'claims_conservation')
                self.assertEqual(result.returncode, 1)
                self.assertIn('SBF frame diagnostics', result.stderr)
                self.assertIn('never-ran', result.stdout)
        self.assertFalse(any(line.startswith('test ') for line in (self.work / 'trace').read_text().splitlines()))

    def test_scheduler_silent_success_is_never_ran(self):
        result = self.run_case('scheduler', '--test', 'claims_conservation')
        self.assertEqual(result.returncode, 2, result.stderr)
        self.assertEqual(self.rows()[0]['verdict'], 'never-ran')
        self.assertEqual(len((self.work / 'trace').read_text().splitlines()), 1)

    def test_missing_fixture_is_a_prerequisite_failure_before_build(self):
        self.env['TOKEN_2022_SO'] = str(self.work / 'absent.so')
        result = self.run_case('pass', '--test', 'claims_conservation')
        self.assertEqual(result.returncode, 2)
        self.assertIn('does not exist', result.stderr)
        self.assertEqual(self.rows()[0]['verdict'], 'never-ran')
        self.assertFalse((self.work / 'trace').exists())

    def test_wrong_fixture_digest_is_a_failure_before_build(self):
        self.tool('shasum', "echo 'wrong-digest  fixture'\n")
        result = self.run_case('pass', '--test', 'claims_conservation')
        self.assertEqual(result.returncode, 1)
        self.assertIn('not the audited v11 fixture', result.stderr)
        self.assertFalse((self.work / 'trace').exists())

    def test_bad_selection_refuses_before_creating_a_run(self):
        for args, phrase in ((('--test', 'typo'), 'unknown test target'),
                             (('--test',), '--test needs a target'),
                             (('--test', 'claims_founding', '--test', 'claims_founding'), 'duplicate test target')):
            with self.subTest(args=args):
                result = self.run_case('pass', *args)
                self.assertEqual(result.returncode, 2)
                self.assertIn(phrase, result.stderr)
                self.assertFalse((self.work / 'runs').exists())

    def test_default_runs_every_registered_target(self):
        listed = self.run_case('pass', '--list')
        self.assertEqual(listed.returncode, 0)
        result = self.run_case()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual([r['target'] for r in self.rows()], listed.stdout.splitlines())
        self.assertTrue(all(r['verdict'] == 'passed' for r in self.rows()))


if __name__ == '__main__':
    unittest.main()
