"""tools/gate twins -- the web/SDK twin trees, held to the class tools/twins/classification.mjs gives each pair.

Runs apps/dclutch-web/lib/twinIdentity.test.ts, the one reader that compares both
trees byte for byte. Refuses: a TWIN pair that differs, an exempted pair that has
become identical (the exemption outlived its reason: delete the line), a REEXPORT
that is no longer a bare `export * from`, a SHIM that adds nothing, a WEB-ONLY
file the package has a copy of. The absorption tool for a red TWIN is
`packages/dclutch-sdk/scripts/sync-from-web.mjs --copy --only <path>`.
"""

from __future__ import annotations

from .common import EXIT_FAIL, EXIT_PASS, EXIT_PREREQ, REPO, have, note, sh

WEB = REPO / "apps" / "dclutch-web"


def check(*, dry_run: bool = False):
    if not have("npx"):
        return EXIT_PREREQ, "node/npx is not on PATH"
    if not (WEB / "node_modules").is_dir():
        return EXIT_PREREQ, "apps/dclutch-web/node_modules is absent (npm ci)"
    args = ["npx", "vitest", "run", "--config", "vitest.config.ts", "lib/twinIdentity.test.ts"]
    if dry_run:
        note(f"$ cd {WEB} && " + " ".join(args))
        return EXIT_PASS, ""
    if sh(args, cwd=WEB).returncode:
        return EXIT_FAIL, "a twin pair diverges from its class in tools/twins/classification.mjs"
    return EXIT_PASS, ""


def main(argv: list[str]) -> int:
    if argv and argv[0] in ("-h", "--help"):
        print(__doc__.strip())
        return EXIT_PASS
    code, detail = check()
    if detail:
        print(f"twins: {detail}")
    return code
