#!/usr/bin/env python3
"""Merge generation-named crates into one crate per authority.

One group at a time: `merge_crates.py <group>` (or `--all`). Each group names a
target crate, the constituent whose items stay at the target's root, and the
constituents that become `pub mod`s. Every move is a `git mv`; every rewrite is
textual and reversible by reading the diff; the control is `cargo check` on the
target and `cargo metadata --offline` on every workspace afterwards.

What it rewrites, and nothing else:

* Rust paths. `dclutch_<absorbed>::x` becomes `dclutch_<target>::<mod>::x`
  everywhere outside the target crate, and `crate::<mod>::x` inside it; the
  root constituent only changes crate name. `crate::` and `$crate::` inside a
  moved constituent gain the module segment. Lean-emitted files are moved, never
  edited: a group is refused if a generated file would need a text change.
* Relative path literals (`#[path]`, `include!`, `include_str!`,
  `include_bytes!`, `env!("CARGO_MANIFEST_DIR")` joins) are re-resolved from the
  file's new location to the file's new target.
* Test targets of an absorbed constituent are prefixed `<mod>__` so the flat
  `tests/` directory stays flat (the emission guard census discovers only
  `tests/*.rs`); a `mod support;` in one of them gets an explicit `#[path]`.
* Guard scripts move to the target root as `check-…-<mod>.sh` and keep
  `crate_dir/../..` as the repository root; their `src/` and `tests/` paths
  gain the module segment.
* Every `Cargo.toml` in the tree: path dependencies on a constituent become one
  dependency on the target (features unioned, `svm` added for SVM-gated
  modules), with the relative path recomputed from the manifest's location.
* Root workspace members, `tools/ci/root-targets.tsv`, and path strings under
  `tools/`, `.github/` and the client script directories.
"""

from __future__ import annotations

import argparse
import os
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

# target -> (description, root constituent, {absorbed crate: module name},
#            {absorbed crate needing the `svm` feature}, extra feature lines)
GROUPS: dict[str, dict] = {
    "dclutch-sbf-runtime": dict(
        description="The SBF runtime shared by every dClutch executable: the granted-heap bump allocator and the one diagnostic compute checkpoint",
        root="dclutch-sbf-bump-heap",
        mods={"dclutch-cu-checkpoint": "cu_checkpoint"},
        svm=set(),
    ),
    "dclutch-custody": dict(
        description="Collateral custody: the canonical custody contract and the exact SDK-free Token-2022 wire views it settles through",
        root="dclutch-custody-contract",
        mods={"dclutch-token-svm": "token_svm"},
        svm=set(),
    ),
    "dclutch-registry": dict(
        description="The Registry authority chain: release sets, finalized records, admission semantics, the Loader V3 and Registry SBF wires, and CPI-free activation-cache authentication",
        root="dclutch-registry-contract",
        mods={
            "dclutch-registry-svm": "svm",
            "dclutch-registry-activation-auth-v1": "activation_auth_v1",
            "dclutch-record-contract": "record",
            "dclutch-release-set-contract": "release_set",
        },
        svm={"dclutch-registry-activation-auth-v1"},
    ),
    "dclutch-vm": dict(
        description="The Lean-owned interpreter waist every family shares: transition programs, the Effect IR, account and request profiles, and the validated-artifact seal",
        root="dclutch-transition-vm",
        mods={
            "dclutch-effect-kernel": "effect",
            "dclutch-account-profile-contract": "account_profile",
            "dclutch-request-profile-contract": "request_profile",
            "dclutch-capability-seal-contract": "capability_seal",
        },
        svm=set(),
    ),
    "dclutch-market": dict(
        description="The universal Market Core and its capability vocabulary: manifests, capability programs, activation artifacts, execution strategies, realm positions, rent, and governed protocol parameters",
        root="dclutch-market-core-codec",
        mods={
            "dclutch-capability-contract": "capability_manifest",
            "dclutch-capability-program-contract": "capability_program",
            "dclutch-capability-activation-codec": "capability_activation",
            "dclutch-execution-strategy-contract": "execution_strategy",
            "dclutch-realm-contract": "realm",
            "dclutch-rent-contract": "rent",
            "dclutch-protocol-parameters-contract": "protocol_parameters",
        },
        svm=set(),
    ),
    "dclutch-product": dict(
        description="Product truth: runtime-width result domains and portfolios, the provider-neutral contract, admission receipts, the exact payoff codec, the slice kernel, and the SVM graph reader",
        root="dclutch-product-runtime-v2",
        mods={
            "dclutch-product-contract": "contract",
            "dclutch-product-runtime-v2-admission": "admission",
            "dclutch-product-payoff-v2-codec": "payoff",
            "dclutch-economic-slice-kernel": "economic_slice",
        },
        svm=set(),
    ),
    # The SVM graph reader folds in after the Claims merge: its representation
    # reader reaches the Rational kernels, which reach the payoff codec, so it
    # can only join Product once that file has moved to Claims.
    "product-svm-reader": dict(
        target="dclutch-product",
        description=None,
        root="dclutch-product",
        mods={"dclutch-product-runtime-v2-svm-reader": "svm_reader"},
        svm={"dclutch-product-runtime-v2-svm-reader"},
    ),
    "dclutch-source": dict(
        description="Sources and their resolution: source material and recovery policy, the Resolution controller codec, relayed mainnet state, and the Pyth SVM views",
        root="dclutch-source-contract",
        mods={
            "dclutch-resolution-codec": "resolution",
            "dclutch-relay-contract": "relay",
            "dclutch-pyth-svm": "pyth",
        },
        svm=set(),
    ),
    "dclutch-claims": dict(
        description="The single Claims economic owner and every representation over it: the child ABI, conservation, Fractional, Rational V2, composition, Bearer, Structured, and position admission",
        root="dclutch-claims-svm",
        mods={
            "dclutch-claims-conservation-contract": "conservation",
            "dclutch-fractional-claim-kernel": "fractional_kernel",
            "dclutch-fractional-claim-contract": "fractional",
            "dclutch-fractional-claims-kernel": "fractional_lowering",
            "dclutch-rational-representation-v2-kernel": "rational_kernel",
            "dclutch-rational-representation-v2-request-contract": "rational_request",
            "dclutch-rational-representation-v2-contract": "rational",
            "dclutch-rational-representation-v2-lifecycle-contract": "rational_lifecycle",
            "dclutch-representation-composition-v3-kernel": "composition",
            "dclutch-bearer-v2-contract": "bearer",
            "dclutch-structured-v2-kernel": "structured_kernel",
            "dclutch-structured-v2-contract": "structured",
            "dclutch-user-position-admission-contract": "position_admission",
        },
        svm=set(),
    ),
    "dclutch-trading": dict(
        description="The Trading families' layouts and laws: compiled Direct data, Dealer liquidity and scenarios, General clearing, Series, and the Shadow accelerator boundary",
        root="dclutch-direct-codec",
        mods={
            "dclutch-dealer-scenario-kernel": "dealer_scenario",
            "dclutch-dealer-codec": "dealer",
            "dclutch-general-codec": "general_codec",
            "dclutch-general-config-contract": "general_config",
            "dclutch-general-adapter-contract": "general",
            "dclutch-series-v3-kernel": "series",
            "dclutch-shadow-accelerator-auth-v4": "shadow_accelerator_auth",
        },
        svm={"dclutch-shadow-accelerator-auth-v4"},
    ),
}

SKIP_DIRS = {"target", "node_modules", ".git", ".lake", ".claude"}
PATH_TEXT_ROOTS = ["tools", ".github", "apps/dclutch-web/scripts", "packages/dclutch-sdk/scripts",
                   "apps/dclutch-web/package.json", "packages/dclutch-sdk/package.json",
                   "AGENTS.md", "README.md", "ARCHITECTURE.md", "COMPOST.md"]


def sh(*args: str, cwd: pathlib.Path = ROOT, check: bool = True) -> str:
    proc = subprocess.run(args, cwd=cwd, capture_output=True, text=True)
    if check and proc.returncode:
        raise SystemExit(f"{' '.join(args)} failed:\n{proc.stdout}\n{proc.stderr}")
    return proc.stdout


def tracked(prefix: str = "") -> list[str]:
    return [line for line in sh("git", "ls-files", "-z", prefix).split("\0") if line]


def snake(name: str) -> str:
    return name.replace("-", "_")


def walk_files(root: pathlib.Path, suffixes: tuple[str, ...]) -> list[pathlib.Path]:
    out = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        for f in filenames:
            if f.endswith(suffixes):
                out.append(pathlib.Path(dirpath) / f)
    return out


class Merge:
    def __init__(self, target: str, spec: dict):
        self.target = spec.get("target", target)
        self.spec = spec
        self.root_crate: str = spec["root"]
        self.mods: dict[str, str] = spec["mods"]
        self.svm: set[str] = spec["svm"]
        self.constituents = [self.root_crate, *self.mods]
        self.target_dir = ROOT / "crates" / self.target
        # old absolute path -> new absolute path, for files and directories moved
        self.moves: dict[pathlib.Path, pathlib.Path] = {}

    # ---- naming ---------------------------------------------------------
    def module_of(self, crate: str) -> str | None:
        return None if crate == self.root_crate else self.mods[crate]

    def new_rust_prefix(self, crate: str, inside_target_src: bool) -> str:
        mod = self.module_of(crate)
        head = "crate" if inside_target_src else snake(self.target)
        return f"{head}::{mod}::" if mod else f"{head}::"

    # ---- moves ----------------------------------------------------------
    def old_dir(self, crate: str) -> pathlib.Path:
        return ROOT / "crates" / crate

    def new_path_for(self, old: pathlib.Path) -> pathlib.Path:
        """Where a path inside a constituent lands after the merge."""
        for crate in self.constituents:
            base = self.old_dir(crate)
            try:
                rel = old.relative_to(base)
            except ValueError:
                continue
            mod = self.module_of(crate)
            parts = rel.parts
            if mod is None:
                return self.target_dir / rel
            if not parts:
                return self.target_dir / mod
            if parts[0] == "src":
                inner = pathlib.Path(*parts[1:]) if len(parts) > 1 else pathlib.Path()
                if inner == pathlib.Path("lib.rs"):
                    return self.target_dir / "src" / mod / "mod.rs"
                return self.target_dir / "src" / mod / inner
            if parts[0] == "tests" and len(parts) > 1:
                return self.target_dir / "tests" / f"{mod}__{parts[1]}" / pathlib.Path(*parts[2:])
            if parts[0] == "examples" and len(parts) > 1:
                # cargo discovers examples only at the crate root; the name is kept
                # because scripts invoke `--example <name>` by it
                return self.target_dir / "examples" / pathlib.Path(*parts[1:])
            if parts[0] == "Cargo.toml":
                return self.target_dir / "Cargo.toml"
            if len(parts) == 1 and parts[0].endswith(".sh") and "check" in parts[0]:
                stem = parts[0][:-3]
                return self.target_dir / f"{stem}-{mod.replace('_', '-')}.sh"
            return self.target_dir / mod / rel
        return old

    def plan_moves(self) -> None:
        for crate in self.constituents:
            base = self.old_dir(crate)
            for rel in tracked(str(base.relative_to(ROOT))):
                old = ROOT / rel
                if old.name == "Cargo.lock" and old.parent == base:
                    continue  # a member's private lock is junk; deleted below
                if old.name == "Cargo.toml" and old.parent == base and crate != self.root_crate:
                    continue  # merged into the target manifest below
                self.moves[old] = self.new_path_for(old)

    def apply_moves(self) -> None:
        if self.root_crate != self.target:
            sh("git", "mv", str(self.old_dir(self.root_crate)), str(self.target_dir))
        # recompute roots after the rename: the root constituent's files are in place
        for old, new in sorted(self.moves.items()):
            if old.is_relative_to(self.old_dir(self.root_crate)):
                continue
            new.parent.mkdir(parents=True, exist_ok=True)
            sh("git", "mv", str(old), str(new))
        for crate in self.mods:
            base = self.old_dir(crate)
            for junk in ("Cargo.lock", "Cargo.toml"):
                if (base / junk).exists():
                    sh("git", "rm", "-q", str(base / junk))
            leftovers = [p for p in base.rglob("*") if p.is_file()]
            if leftovers:
                raise SystemExit(f"{crate}: untracked leftovers {leftovers[:5]}")
            for dirpath, dirnames, _ in os.walk(base, topdown=False):
                pathlib.Path(dirpath).rmdir()

    # ---- rust text -------------------------------------------------------
    def rewrite_rust(self) -> list[str]:
        notes = []
        target_src = self.target_dir / "src"
        crate_re = {c: re.compile(r"\b" + snake(c) + r"::") for c in self.constituents}
        bare_use_re = {c: re.compile(r"\buse " + snake(c) + r" as ") for c in self.constituents}
        for path in walk_files(ROOT, (".rs",)):
            text = path.read_text(encoding="utf-8", errors="surrogateescape")
            original = text
            inside_src = path.is_relative_to(target_src)
            owner = self.owner_of_moved(path)
            if inside_src and owner is not None and owner != self.root_crate:
                mod = self.mods[owner]
                if self.is_generated(text):
                    if re.search(r"\bcrate::|\$crate::", text):
                        raise SystemExit(f"{path}: a Lean-emitted file needs a crate path change")
                else:
                    text = re.sub(r"(?<![A-Za-z0-9_])crate::", f"crate::{mod}::", text)
                    text = text.replace("$crate::", f"$crate::{mod}::")
                    if path.name == "mod.rs":
                        text = re.sub(r"^#!\[no_std\]\n", "", text, flags=re.M)
            for c in self.constituents:
                prefix = self.new_rust_prefix(c, inside_src)
                text = crate_re[c].sub(prefix, text)
                text = bare_use_re[c].sub(f"use {prefix[:-2]} as ", text)
                mod = self.module_of(c)
                spoken = f"{self.target}::{mod}" if mod else self.target
                text = text.replace(f"`{c}`", f"`{spoken}`")
                text = text.replace(f"`{c}/", f"`crates/{self.target}/{mod + '/' if mod else ''}")
            if text != original:
                path.write_text(text, encoding="utf-8", errors="surrogateescape")
        return notes

    def owner_of_moved(self, new_path: pathlib.Path) -> str | None:
        for old, new in self.moves.items():
            if new == new_path:
                for c in self.constituents:
                    if old.is_relative_to(self.old_dir(c)):
                        return c
        if new_path.is_relative_to(self.target_dir):
            return self.root_crate
        return None

    @staticmethod
    def is_generated(text: str) -> bool:
        return text.startswith("// @generated by formal/")

    # ---- relative path literals ----------------------------------------
    LITERAL_RE = re.compile(
        r'(#\[path\s*=\s*"|include!\(\s*"|include_str!\(\s*"|include_bytes!\(\s*"|\.join\("|CARGO_MANIFEST_DIR"\),\s*")([^"]+)"'
    )

    def rewrite_literals(self) -> None:
        old_of_new = {new: old for old, new in self.moves.items()}
        for new, old in old_of_new.items():
            if new.suffix != ".rs" or not new.exists():
                continue
            text = new.read_text(encoding="utf-8", errors="surrogateescape")
            if self.is_generated(text):
                continue
            manifest_old = self.crate_root_of(old)
            manifest_new = self.target_dir

            def fix(match: re.Match) -> str:
                head, lit = match.group(1), match.group(2)
                if head.startswith(".join(") or head.startswith("CARGO_MANIFEST_DIR"):
                    stripped = lit.lstrip("/")
                    if not (stripped.startswith("src/") or stripped.startswith("tests/")):
                        return match.group(0)
                    target_old = (manifest_old / stripped).resolve()
                    target_new = self.moves.get(target_old, target_old)
                    rel = os.path.relpath(target_new, manifest_new)
                    return f'{head}{"/" if lit.startswith("/") else ""}{rel}"'
                target_old = (old.parent / lit).resolve()
                target_new = self.moves.get(target_old, target_old)
                rel = os.path.relpath(target_new, new.parent)
                return f'{head}{rel}"'

            fixed = self.LITERAL_RE.sub(fix, text)
            # `mod support;` in a moved integration test: the directory is now prefixed
            if new.parent == self.target_dir / "tests" and new.name.split("__")[0] in self.mods.values():
                mod = new.name.split("__")[0]

                def fix_mod(match: re.Match) -> str:
                    name = match.group(2)
                    candidates = [self.target_dir / "tests" / f"{mod}__{name}" / "mod.rs",
                                  self.target_dir / "tests" / f"{mod}__{name}.rs"]
                    for cand in candidates:
                        if cand.exists():
                            rel = os.path.relpath(cand, new.parent)
                            return f'#[path = "{rel}"]\n{match.group(1)}mod {name};'
                    return match.group(0)

                fixed = re.sub(r"^((?:pub(?:\(crate\))? )?)mod (\w+);", fix_mod, fixed, flags=re.M)
            if fixed != text:
                new.write_text(fixed, encoding="utf-8", errors="surrogateescape")

    def crate_root_of(self, old: pathlib.Path) -> pathlib.Path:
        for c in self.constituents:
            if old.is_relative_to(self.old_dir(c)):
                return self.old_dir(c)
        raise AssertionError(old)

    # ---- guard scripts -------------------------------------------------
    def rewrite_scripts(self) -> None:
        for old, new in self.moves.items():
            if new.suffix != ".sh" or new.parent != self.target_dir:
                continue
            crate = self.owner_of_moved(new)
            mod = self.module_of(crate)
            if mod is None:
                continue
            text = new.read_text()
            text = re.sub(r'\$crate_dir/src/', f'$crate_dir/src/{mod}/', text)
            text = re.sub(r'\$crate_dir/src"', f'$crate_dir/src/{mod}"', text)
            text = re.sub(r'\$crate_dir/tests/', f'$crate_dir/tests/{mod}__', text)
            new.write_text(text)

    # ---- manifests -----------------------------------------------------
    DEP_LINE_RE = re.compile(r'^(dclutch-[a-z0-9-]+)\s*=\s*\{([^}]*)\}\s*$', re.M)

    def merge_target_manifest(self) -> None:
        manifest = self.target_dir / "Cargo.toml"
        text = manifest.read_text()
        text = re.sub(r'^name = ".*"$', f'name = "{self.target}"', text, count=1, flags=re.M)
        if self.spec.get("description"):
            text = re.sub(r'^description = ".*"$', f'description = "{self.spec["description"]}"', text, count=1, flags=re.M)
        if "description =" not in text and self.spec.get("description"):
            text = text.replace("[package]\n", f'[package]\ndescription = "{self.spec["description"]}"\n', 1)
        text = re.sub(r'^\[lib\]\npath = "src/lib.rs"\n\n?', "", text, flags=re.M)
        text = re.sub(r'^publish = false\n', "", text, flags=re.M)
        text = re.sub(r'^edition = "20\d\d"$', "edition.workspace = true", text, flags=re.M)
        text = re.sub(r'^license = ".*"$', "license.workspace = true", text, flags=re.M)
        text = re.sub(r'^rust-version = ".*"$', "rust-version.workspace = true", text, flags=re.M)
        sections = parse_sections(text)
        for crate in self.mods:
            other = parse_sections(sh("git", "show", f"HEAD:crates/{crate}/Cargo.toml"))
            for name, body in other.items():
                if name in ("package",) or name.startswith(("lib", "lints")):
                    continue
                if name == "features" and "features" in sections:
                    for line in body:
                        if line.strip() and line not in sections["features"] and not line.startswith("default"):
                            sections["features"].append(line)
                    continue
                sections.setdefault(name, [])
                for line in body:
                    if line.strip() and line not in sections[name]:
                        sections[name].append(line)
        if self.svm:
            feats = sections.setdefault("features", [])
            ungated_sdk = any(
                "solana-program" in sh("git", "show", f"HEAD:crates/{c}/Cargo.toml")
                for c in self.constituents if c not in self.svm
            )
            deps = sections.setdefault("dependencies", [])
            if ungated_sdk:
                feats.append("svm = []")
            else:
                feats.append('svm = ["dep:solana-program", "dep:solana-sdk-ids"]')
                deps[:] = [l for l in deps if not l.startswith("solana-program") and not l.startswith("solana-sdk-ids")]
                deps.append('solana-program = { version = "=3.0.0", default-features = false, optional = true }')
                deps.append('solana-sdk-ids = { version = "=3.1.0", optional = true }')
        for name, body in sections.items():
            if name == "__head__" or name == "package":
                continue
            merged: list[str] = []
            index: dict[str, int] = {}
            for line in body:
                m = re.match(r'^([A-Za-z0-9_-]+)\s*=', line)
                key = m.group(1) if m else None
                if key and key in index:
                    merged[index[key]] = union_dep_lines(merged[index[key]], line)
                    continue
                if key:
                    index[key] = len(merged)
                merged.append(line)
            sections[name] = merged
        manifest.write_text(render_sections(sections))

    def rewrite_manifests(self) -> list[str]:
        notes = []
        consts = set(self.constituents)
        old_of_new = {new: old for old, new in self.moves.items()}
        for path in walk_files(ROOT, ("Cargo.toml",)):
            text = path.read_text()
            if path in old_of_new and path != self.target_dir / "Cargo.toml":
                text = self.fix_moved_manifest_paths(text, old_of_new[path], path)
            new_text = self.rewrite_manifest_text(text, path, consts)
            new_text = self.rewrite_feature_refs(new_text, consts)
            for c in self.constituents:
                mod = self.module_of(c)
                new_text = new_text.replace(f"`{c}`", f"`{self.target}::{mod}`" if mod else f"`{self.target}`")
            if new_text != text:
                path.write_text(new_text)
                notes.append(str(path.relative_to(ROOT)))
        return notes

    def fix_moved_manifest_paths(self, text: str, old: pathlib.Path, new: pathlib.Path) -> str:
        def fix(match: re.Match) -> str:
            target_old = (old.parent / match.group(1)).resolve()
            target_new = self.moves.get(target_old, target_old)
            return f'path = "{os.path.relpath(target_new, new.parent)}"'
        return re.sub(r'path\s*=\s*"([^"]*)"', fix, text)

    def rewrite_manifest_text(self, text: str, path: pathlib.Path, consts: set[str]) -> str:
        out_lines = []
        section = None
        seen_target_in_section: dict[str, int] = {}
        lines = text.split("\n")
        for line in lines:
            if line.startswith("["):
                section = line
                out_lines.append(line)
                continue
            m = self.DEP_LINE_RE.match(line)
            if m and (m.group(1) in consts or m.group(1) == self.target):
                crate, body = m.group(1), m.group(2)
                if path == self.target_dir / "Cargo.toml" and crate == self.target and not (section and section.startswith("[dev-dependencies]")):
                    continue  # a constituent's dependency on the crate it now lives in
                if path == self.target_dir / "Cargo.toml" and crate != self.target:
                    # a constituent depending on a sibling: now the same crate
                    if section and section.startswith("[dev-dependencies]") and "features" in body:
                        body = re.sub(r'path\s*=\s*"[^"]*"', f'path = "."', body)
                        line = f"{self.target} = {{{body}}}"
                    else:
                        continue
                    crate = self.target
                else:
                    line = self.retarget_line(crate, body, path)
                key = f"{section}"
                if key in seen_target_in_section:
                    prev = seen_target_in_section[key]
                    out_lines[prev] = union_dep_lines(out_lines[prev], line)
                    continue
                seen_target_in_section[key] = len(out_lines)
            out_lines.append(line)
        return "\n".join(out_lines)

    def rewrite_feature_refs(self, text: str, consts: set[str]) -> str:
        """`dep:dclutch-<c>`, `dclutch-<c>/f` and bare `dclutch-<c>` inside feature lists."""
        for crate in consts:
            text = re.sub(rf'"dep:{re.escape(crate)}"', f'"dep:{self.target}"', text)
            text = re.sub(rf'"{re.escape(crate)}/', f'"{self.target}/', text)
            text = re.sub(rf'"{re.escape(crate)}"', f'"{self.target}"', text)
        optional = re.search(rf'^{re.escape(self.target)}\s*=\s*\{{[^}}]*optional\s*=\s*true', text, re.M)
        if not optional and f'"dep:{self.target}"' in text:
            def drop(match: re.Match) -> str:
                items = [i for i in re.findall(r'"[^"]*"', match.group(2)) if i != f'"dep:{self.target}"']
                multiline = "\n" in match.group(2)
                if multiline:
                    inner = "\n    " + ",\n    ".join(items) + ",\n" if items else ""
                else:
                    inner = ", ".join(items)
                return f"{match.group(1)}[{inner}]"

            text = re.sub(
                r'^([a-z0-9_-]+\s*=\s*)\[([^\]]*)\]',
                lambda m: drop(m) if f'"dep:{self.target}"' in m.group(2) else m.group(0),
                text,
                flags=re.M,
            )

        # dedupe repeated entries inside one feature list
        def dedupe(match: re.Match) -> str:
            seen: list[str] = []
            for item in re.findall(r'"[^"]*"', match.group(2)):
                if item not in seen:
                    seen.append(item)
            sep = ",\n    " if "\n" in match.group(2) else ", "
            inner = sep.join(seen)
            if "\n" in match.group(2):
                inner = "\n    " + inner + ",\n"
            return f"{match.group(1)}[{inner}]"

        text = re.sub(
            r'^([a-z0-9_-]+\s*=\s*)\[([^\]]*)\]',
            lambda m: dedupe(m) if self.target in m.group(2) else m.group(0),
            text,
            flags=re.M,
        )
        return text

    def retarget_line(self, crate: str, body: str, manifest: pathlib.Path) -> str:
        pm = re.search(r'path\s*=\s*"([^"]*)"', body)
        if not pm:
            return f"{crate} = {{{body}}}"
        manifest_new = self.moves.get(manifest, manifest)
        manifest_old = manifest
        for old, new in self.moves.items():
            if new == manifest:
                manifest_old = old
        rel_new = os.path.relpath(self.target_dir, manifest_new.parent)
        body = body.replace(pm.group(0), f'path = "{rel_new}"')
        if crate in self.svm:
            fm = re.search(r'features\s*=\s*\[([^\]]*)\]', body)
            if fm:
                feats = [f.strip() for f in fm.group(1).split(",") if f.strip()]
                if '"svm"' not in feats:
                    feats.append('"svm"')
                body = body.replace(fm.group(0), f'features = [{", ".join(feats)}]')
            else:
                body = body.rstrip() + ', features = ["svm"] '
        return f"{self.target} = {{{body}}}"

    def rewrite_root_members(self) -> None:
        root = ROOT / "Cargo.toml"
        text = root.read_text()
        for crate in self.mods:
            text = text.replace(f'    "crates/{crate}",\n', "")
        text = text.replace(f'    "crates/{self.root_crate}",\n', f'    "crates/{self.target}",\n')
        root.write_text(text)

    # ---- tsv and path strings ------------------------------------------
    def rewrite_root_targets(self) -> None:
        tsv = ROOT / "tools/ci/root-targets.tsv"
        lines = tsv.read_text().split("\n")
        out = []
        for line in lines:
            parts = line.split("\t")
            if len(parts) >= 3 and parts[1] in self.constituents:
                mod = self.module_of(parts[1])
                parts[1] = self.target
                if mod:
                    parts[2] = f"{mod}__{parts[2]}"
                line = "\t".join(parts)
            out.append(line)
        tsv.write_text("\n".join(out))

    TEXT_SUFFIXES = (".sh", ".py", ".json", ".tsv", ".md", ".mjs", ".ts", ".tsx", ".yml", ".yaml", ".toml", ".txt", ".rs")
    TEXT_SKIP_DIRS = SKIP_DIRS | {"docs", "formal", "generated", "sbom", "simplify", "frameguard"}
    TEXT_SKIP_FILES = {"GOAL.md", "WAVE.md", "SESSION_STATE.md", "Cargo.lock", "CU_BUDGETS.json"}

    def rewrite_path_strings(self) -> list[str]:
        """Path and package-name strings everywhere a live reference can sit.

        Skipped on purpose: dated history (`docs/`, `GOAL.md`, `WAVE.md`, the
        CU budget provenance), measurements recaptured rather than edited
        (`tools/frameguard`), generated outputs re-emitted by their generators
        (`lib/generated`, `tools/sbom`), the Lean tree (its owner's), and this
        tool. In prose, an absorbed crate is spoken as `target::module`; in any
        file where the name is a package identifier it becomes the bare target.
        """
        touched = []
        for dirpath, dirnames, filenames in os.walk(ROOT):
            dirnames[:] = [d for d in dirnames if d not in self.TEXT_SKIP_DIRS]
            for name in filenames:
                if name in self.TEXT_SKIP_FILES or not name.endswith(self.TEXT_SUFFIXES):
                    continue
                path = pathlib.Path(dirpath) / name
                try:
                    text = path.read_text(encoding="utf-8")
                except (UnicodeDecodeError, OSError):
                    continue
                if text.startswith("// @generated by formal/"):
                    continue
                original = text
                provenance = re.findall(r"^/// Formerly the `[^`]+` crate\.$", text, re.M)
                prose = name.endswith((".md", ".rs", ".ts", ".tsx", ".mjs"))
                for crate in self.constituents:
                    mod = self.module_of(crate)
                    if mod:
                        text = re.sub(rf"crates/{crate}/src/lib\.rs", f"crates/{self.target}/src/{mod}/mod.rs", text)
                        text = re.sub(rf"crates/{crate}/src/", f"crates/{self.target}/src/{mod}/", text)
                        text = re.sub(rf"crates/{crate}/tests/", f"crates/{self.target}/tests/{mod}__", text)
                        text = re.sub(rf"crates/{crate}/(check[^/\s\"'`]*)\.sh", lambda m: f"crates/{self.target}/{m.group(1)}-{mod.replace('_', '-')}.sh", text)
                        text = re.sub(rf"crates/{crate}/Cargo\.toml", f"crates/{self.target}/Cargo.toml", text)
                        text = re.sub(rf"crates/{crate}/", f"crates/{self.target}/{mod}/", text)
                        text = re.sub(rf"crates/{crate}(?![A-Za-z0-9_-])", f"crates/{self.target}", text)
                        if prose:
                            text = text.replace(f"`{crate}`", f"`{self.target}::{mod}`")
                    else:
                        text = re.sub(rf"crates/{crate}(?![A-Za-z0-9_-])", f"crates/{self.target}", text)
                    if mod:
                        text = re.sub(rf"(?<![A-Za-z0-9_/-]){re.escape(crate)}::", f"{self.target}::{mod}::", text)
                    text = re.sub(rf"(?<![A-Za-z0-9_/-]){re.escape(crate)}(?![A-Za-z0-9_-])", self.target, text)
                    if not name.endswith(".rs"):
                        text = re.sub(rf"(?<![A-Za-z0-9_]){snake(crate)}::", snake(self.target) + (f"::{mod}::" if mod else "::"), text)
                    text = re.sub(rf"(?<![A-Za-z0-9_:]){snake(crate)}(?![A-Za-z0-9_:])", snake(self.target) + (f"::{mod}" if mod and not name.endswith((".toml", ".json", ".tsv", ".sh", ".py", ".yml", ".yaml")) else ""), text)
                if provenance:
                    # a module's provenance line keeps the old crate's name on purpose
                    rewritten = re.findall(r"^/// Formerly the `[^`]+` crate\.$", text, re.M)
                    for before, after in zip(provenance, rewritten):
                        text = text.replace(after, before, 1)
                if text != original:
                    path.write_text(text, encoding="utf-8")
                    touched.append(str(path.relative_to(ROOT)))
        return touched

    # ---- lib.rs ---------------------------------------------------------
    EXTERN_RE = re.compile(r"^(#\[cfg\(test\)\]\n)?extern crate (std|alloc);$", re.M)

    def lift_extern_crates(self) -> None:
        """A root-level `extern crate std;` binds crate-wide only from the root.

        Moved into a `mod.rs` it binds that module alone, and every sibling
        that spelled `std::vec!` stops resolving. Lift the declaration to the
        target root, keeping `#[cfg(test)]` when the source had it.
        """
        lib = self.target_dir / "src" / "lib.rs"
        text = lib.read_text()
        wanted: dict[str, bool] = {}
        for mod in self.mods.values():
            mod_rs = self.target_dir / "src" / mod / "mod.rs"
            if not mod_rs.exists():
                continue
            for cfg_test, name in self.EXTERN_RE.findall(mod_rs.read_text()):
                wanted[name] = wanted.get(name, True) and bool(cfg_test)
        additions = []
        for name, cfg_test in wanted.items():
            if re.search(rf"^extern crate {name};$", text, re.M):
                continue
            additions.append(("#[cfg(test)]\n" if cfg_test else "") + f"extern crate {name};")
        if additions:
            # after the inner attributes and the `//!` crate doc, which must precede items
            head = re.match(r"(?:(?:#!\[[^\n]*\]|//![^\n]*|)\n)*", text)
            head_end = head.end() if head else 0
            text = text[:head_end] + "\n".join(additions) + "\n\n" + text[head_end:]
            lib.write_text(text)

    def append_modules(self) -> None:
        self.lift_extern_crates()
        lib = self.target_dir / "src" / "lib.rs"
        text = lib.read_text()
        lines = ["", "// Authorities merged into this crate; each module was one crate before."]
        for crate, mod in self.mods.items():
            doc = f"/// Formerly the `{crate}` crate."
            if crate in self.svm:
                lines.append(doc)
                lines.append('#[cfg(feature = "svm")]')
            else:
                lines.append(doc)
            lines.append(f"pub mod {mod};")
        text = text.rstrip("\n") + "\n" + "\n".join(lines) + "\n"
        lib.write_text(text)

    # ---- driver --------------------------------------------------------
    def run(self) -> None:
        for c in self.constituents:
            if not self.old_dir(c).is_dir():
                raise SystemExit(f"{c}: not present (already merged?)")
        self.plan_moves()
        self.apply_moves()
        self.merge_target_manifest()
        self.rewrite_rust()
        self.rewrite_literals()
        self.rewrite_scripts()
        manifests = self.rewrite_manifests()
        self.rewrite_root_members()
        self.rewrite_root_targets()
        paths = self.rewrite_path_strings()
        self.append_modules()
        print(f"{self.target}: {len(self.moves)} paths moved, {len(manifests)} manifests, {len(paths)} text files")


def parse_sections(text: str) -> dict[str, list[str]]:
    sections: dict[str, list[str]] = {}
    current = "__head__"
    sections[current] = []
    pending: list[str] = []
    for line in text.split("\n"):
        if line.startswith("["):
            current = line.strip("[]")
            sections.setdefault(current, [])
            sections[current].extend(pending)
            pending = []
            continue
        if line.startswith("#"):
            pending.append(line)
            continue
        if line.strip() == "":
            sections[current].extend(pending)
            pending = []
            continue
        sections[current].extend(pending)
        pending = []
        sections[current].append(line)
    sections[current].extend(pending)
    return sections


def render_sections(sections: dict[str, list[str]]) -> str:
    order = ["__head__", "package", "features", "dependencies", "dev-dependencies", "build-dependencies"]
    keys = [k for k in order if k in sections] + [k for k in sections if k not in order and k != "lints"] + (["lints"] if "lints" in sections else [])
    out = []
    for key in keys:
        body = [l for l in sections[key] if l.strip()]
        if key == "__head__" and not body:
            continue
        if key != "__head__":
            out.append(f"[{key}]")
        out.extend(body)
        out.append("")
    return "\n".join(out).rstrip("\n") + "\n"


def union_dep_lines(a: str, b: str) -> str:
    ma = re.match(r'^(\S+)\s*=\s*\{(.*)\}\s*$', a)
    mb = re.match(r'^(\S+)\s*=\s*\{(.*)\}\s*$', b)
    if not (ma and mb):
        return a
    feats = set()
    for body in (ma.group(2), mb.group(2)):
        fm = re.search(r'features\s*=\s*\[([^\]]*)\]', body)
        if fm:
            feats |= {f.strip() for f in fm.group(1).split(",") if f.strip()}
    body = re.sub(r',?\s*features\s*=\s*\[[^\]]*\]', "", ma.group(2)).strip()
    if "optional" in ma.group(2) and "optional" not in mb.group(2):
        body = re.sub(r',?\s*optional\s*=\s*true', "", body).strip()
    if feats:
        body = f"{body}, features = [{', '.join(sorted(feats))}]"
    return f"{ma.group(1)} = {{ {body.strip(', ')} }}"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("groups", nargs="*")
    parser.add_argument("--all", action="store_true")
    parser.add_argument("--resweep", action="store_true",
                        help="only re-run the text-reference sweep for groups already merged")
    args = parser.parse_args()
    groups = [g for g in GROUPS if "target" not in GROUPS[g]] if args.all else args.groups
    if not groups:
        parser.error("name a group or pass --all")
    for g in groups:
        merge = Merge(g, GROUPS[g])
        if args.resweep:
            touched = merge.rewrite_path_strings()
            print(f"{g}: resweep touched {len(touched)} files")
        else:
            merge.run()
    return 0


if __name__ == "__main__":
    sys.exit(main())
