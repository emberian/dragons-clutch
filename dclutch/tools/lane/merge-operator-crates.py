#!/usr/bin/env python3
"""Absorb the sixteen *-operator crates (and three host helpers) into dclutch-operator.
Usage: merge.py ROOT [--apply]. Mechanical: git mv, path rewrites, manifest rewrites."""
import os, re, sys, subprocess, shutil, collections
ROOT = sys.argv[1]; APPLY = "--apply" in sys.argv
OP = "crates/dclutch-operator"
ABSORB = collections.OrderedDict([
    ("dclutch-bearer-v2-operator", "bearer"),
    ("dclutch-fractional-claim-operator", "fractional"),
    ("dclutch-general-successor-operator", "general_successor"),
    ("dclutch-market-founding-v1-operator", "market_founding"),
    ("dclutch-market-open-v1-operator", "market_open"),
    ("dclutch-market-retirement-v1-operator", "market_retirement"),
    ("dclutch-product-runtime-v2-operator", "product_runtime"),
    ("dclutch-provider-transport-v3-operator", "provider_transport"),
    ("dclutch-rational-representation-v2-operator", "rational_representation"),
    ("dclutch-representation-composition-v3-operator", "representation_composition"),
    ("dclutch-resolution-core-v3-operator", "resolution_core"),
    ("dclutch-source-readiness-operator", "source_readiness"),
    ("dclutch-structured-v2-operator", "structured"),
    ("dclutch-versioned-message-operator", "versioned_message"),
    ("dclutch-wallet-terminal-input-operator", "wallet_terminal_input"),
    ("dclutch-wallet-terminal-payout-operator", "wallet_terminal_payout"),
    ("dclutch-rational-lifecycle-hot-v3", "rational_lifecycle_hot"),
    ("dclutch-hot-bump-miner-v1", "hot_bump_miner"),
    ("dclutch-fractional-cubic-life-evidence", "fractional_cubic_life_evidence"),
])
DOCS = {
    "bearer": "Bearer specialization of the Rational Representation V2 actions.",
    "fractional": "Fractional Claims family construction, lowering and retirement planning.",
    "general_successor": "Read-only General V5 successor-plan production over a route document.",
    "market_founding": "Artifact-derived construction for generic Market founding.",
    "market_open": "Chain-derived Registry continuation for canonical Core market opening.",
    "market_retirement": "Chain-derived construction for the aggregate Market retirement.",
    "product_runtime": "Product Runtime V2 compilation, publication, and unsigned admission plans.",
    "provider_transport": "Chain-derived real Pyth Receiver submission and permissionless reclaim.",
    "rational_representation": "Unsigned instruction construction for exact Rational Representation V2 actions.",
    "representation_composition": "Product-to-representation composition and its unsigned workflows.",
    "resolution_core": "Chain-derived Core effects for the complete funded Resolution lifecycle.",
    "source_readiness": "One chain-authenticated selector for the Source funding-readiness walk.",
    "structured": "Effect planning for shard-backed Structured receipts.",
    "versioned_message": "Address-table lifecycle and versioned-message construction.",
    "wallet_terminal_input": "Wallet-terminal payout input derivation, callable from a browser.",
    "wallet_terminal_payout": "Wallet-terminal payout derivation, callable from a browser.",
    "rational_lifecycle_hot": "Rational lifecycle Hot request, selected-set and bundle construction.",
    "hot_bump_miner": "Bump-hint mining over the decodable Hot corpus.",
    "fractional_cubic_life_evidence": "Evidence bridge for the Fractional cubic-life campaign.",
}
FEATURE_GATE = {"general_successor": "successor"}
crate_us = {c: c.replace("-", "_") for c in ABSORB}

def run(*args):
    if APPLY:
        subprocess.run(args, cwd=ROOT, check=True)
    else:
        print("  $", " ".join(args))

def rewrite_text(text, rules):
    for pat, rep in rules:
        text = re.sub(pat, rep, text)
    return text

def rs_files(d):
    for dp, _, fns in os.walk(d):
        for fn in fns:
            if fn.endswith(".rs"): yield os.path.join(dp, fn)

log = []
# ---------- A. move sources ----------
dep_union = collections.OrderedDict(); dev_union = collections.OrderedDict()
moved_test_dirs = {}  # M -> [dirname]
for C, M in ABSORB.items():
    cdir = os.path.join(ROOT, "crates", C)
    src = os.path.join(cdir, "src")
    dst_mod = os.path.join(ROOT, OP, "src", M)
    entries = sorted(os.listdir(src))
    if APPLY: os.makedirs(dst_mod, exist_ok=True)
    for e in entries:
        p = os.path.join(src, e)
        if e == "lib.rs":
            run("git", "mv", os.path.relpath(p, ROOT), f"{OP}/src/{M}.rs")
        elif e == "bin":
            if APPLY: os.makedirs(os.path.join(ROOT, OP, "src", "bin"), exist_ok=True)
            for b in sorted(os.listdir(p)):
                run("git", "mv", os.path.relpath(os.path.join(p, b), ROOT), f"{OP}/src/bin/{b}")
        else:
            run("git", "mv", os.path.relpath(p, ROOT), f"{OP}/src/{M}/{e}")
    tdir = os.path.join(cdir, "tests")
    moved_test_dirs[M] = []
    if os.path.isdir(tdir):
        for e in sorted(os.listdir(tdir)):
            p = os.path.join(tdir, e)
            if os.path.isdir(p):
                moved_test_dirs[M].append(e)
                run("git", "mv", os.path.relpath(p, ROOT), f"{OP}/tests/{M}_{e}")
            else:
                run("git", "mv", os.path.relpath(p, ROOT), f"{OP}/tests/{M}_{e}")
    # manifest: collect deps
    ct = open(os.path.join(cdir, "Cargo.toml")).read()
    def section(name):
        m = re.search(r"^\[" + name + r"\]\n(.*?)(?=^\[|\Z)", ct, re.M | re.S)
        return m.group(1) if m else ""
    for line in section("dependencies").splitlines():
        m = re.match(r"^([A-Za-z0-9_-]+)\s*=\s*(.*)$", line.strip())
        if m and m.group(1) not in ABSORB and m.group(1) != "dclutch-operator":
            dep_union.setdefault(m.group(1), []).append(m.group(2))
    for line in section("dev-dependencies").splitlines():
        m = re.match(r"^([A-Za-z0-9_-]+)\s*=\s*(.*)$", line.strip())
        if m and m.group(1) not in ABSORB and m.group(1) != "dclutch-operator":
            dev_union.setdefault(m.group(1), []).append(m.group(2))
    # remove the rest of the crate
    run("git", "rm", "-q", "-r", f"crates/{C}")
    if APPLY and os.path.isdir(cdir):
        shutil.rmtree(cdir, ignore_errors=True)

if not APPLY:
    print("dep union:")
    for k, v in dep_union.items(): print("  ", k, set(v))
    print("dev union:")
    for k, v in dev_union.items(): print("  ", k, set(v))
    sys.exit(0)

# ---------- B. rewrite references ----------
sib = "|".join(re.escape(crate_us[c]) for c in ABSORB)
def mod_of(us):
    for c, m in ABSORB.items():
        if crate_us[c] == us: return m
# inside dclutch-operator src (moved + existing)
for p in rs_files(os.path.join(ROOT, OP, "src")):
    rel = os.path.relpath(p, os.path.join(ROOT, OP, "src"))
    text = open(p).read(); orig = text
    top = rel.split("/")[0]
    M = top[:-3] if top.endswith(".rs") else top
    is_moved = M in ABSORB.values() and (rel == M + ".rs" or rel.startswith(M + "/")) and not rel.startswith("bin/")
    if is_moved:
        # the moved crate's own root is now crate::M
        text = re.sub(r"\bcrate::", "crate::" + M + "::", text)
        text = re.sub(r"\b" + re.escape(crate_us[[c for c, m in ABSORB.items() if m == M][0]]) + r"::", "crate::" + M + "::", text)
        text = re.sub(r"\bdclutch_operator::", "crate::", text)
    if rel.startswith("bin/"):
        text = re.sub(r"\b(" + sib + r")::", lambda m: "dclutch_operator::" + mod_of(m.group(1)) + "::", text)
    else:
        text = re.sub(r"\b(" + sib + r")::", lambda m: "crate::" + mod_of(m.group(1)) + "::", text)
        text = re.sub(r"\b(" + sib + r")\b(?!::)", lambda m: "crate::" + mod_of(m.group(1)), text)
    if text != orig:
        open(p, "w").write(text); log.append(rel)
# tests of dclutch-operator (moved + existing): crate paths become dclutch_operator::M
for p in rs_files(os.path.join(ROOT, OP, "tests")):
    text = open(p).read(); orig = text
    text = re.sub(r"\b(" + sib + r")::", lambda m: "dclutch_operator::" + mod_of(m.group(1)) + "::", text)
    text = re.sub(r"\b(" + sib + r")\b(?!::)", lambda m: "dclutch_operator::" + mod_of(m.group(1)), text)
    rel = os.path.relpath(p, os.path.join(ROOT, OP, "tests"))
    M = rel.split("_")[0]
    for Mx, dirs in moved_test_dirs.items():
        if rel.startswith(Mx + "_"):
            for d in dirs:
                text = re.sub(r"^mod " + d + r";", f'#[path = "{Mx}_{d}/mod.rs"]\nmod {d};', text, flags=re.M)
                text = text.replace(f'"{d}/', f'"{Mx}_{d}/')
    if text != orig: open(p, "w").write(text)
# tree-wide consumers
for dp, dns, fns in os.walk(ROOT):
    dns[:] = [d for d in dns if d not in ("target", "node_modules", ".git") and not (dp == ROOT and d == "crates" and False)]
    if dp.startswith(os.path.join(ROOT, OP)): continue
    for fn in fns:
        if not fn.endswith(".rs"): continue
        p = os.path.join(dp, fn)
        text = open(p, errors="replace").read(); orig = text
        text = re.sub(r"\b(" + sib + r")::", lambda m: "dclutch_operator::" + mod_of(m.group(1)) + "::", text)
        text = re.sub(r"\b(" + sib + r")\b(?!::)(?![-\w])", lambda m: "dclutch_operator::" + mod_of(m.group(1)), text)
        if text != orig: open(p, "w").write(text); log.append(os.path.relpath(p, ROOT))
# ---------- C. manifests tree-wide ----------
names = "|".join(re.escape(c) for c in ABSORB)
for dp, dns, fns in os.walk(ROOT):
    dns[:] = [d for d in dns if d not in ("target", "node_modules", ".git")]
    if "Cargo.toml" not in fns: continue
    p = os.path.join(dp, "Cargo.toml")
    if os.path.abspath(p) == os.path.abspath(os.path.join(ROOT, OP, "Cargo.toml")): continue
    text = open(p).read(); orig = text
    is_wasm = "cdylib" in text
    if os.path.abspath(p) == os.path.abspath(os.path.join(ROOT, "Cargo.toml")):
        for C in ABSORB:
            text = re.sub(r'^\s*"crates/' + re.escape(C) + r'",\n', "", text, flags=re.M)
        if text != orig: open(p, "w").write(text)
        continue
    out = []; section = None; has_op = {}; pending = {}
    for line in text.split("\n"):
        ms = re.match(r"^\[(.+)\]", line)
        if ms: section = ms.group(1)
        m = re.match(r"^(" + names + r")\s*=\s*\{(.*)\}\s*$", line)
        if m and section:
            path = re.search(r'path\s*=\s*"([^"]*)"', m.group(2))
            if path:
                newpath = re.sub(r"crates/" + re.escape(m.group(1)) + r"$", "crates/dclutch-operator", path.group(1))
                pending.setdefault(section, newpath)
            continue
        if re.match(r"^dclutch-operator\s*=", line) and section:
            has_op[section] = True
        out.append(line)
    text2 = "\n".join(out)
    for sec, newpath in pending.items():
        if has_op.get(sec): continue
        extra = ', default-features = false' if is_wasm else ""
        insert = f'dclutch-operator = {{ path = "{newpath}"{extra} }}'
        text2 = re.sub(r"^\[" + re.escape(sec) + r"\]\n", f"[{sec}]\n{insert}\n", text2, count=1, flags=re.M)
    if text2 != orig: open(p, "w").write(text2); log.append(os.path.relpath(p, ROOT))
# ---------- D. dclutch-operator lib.rs and Cargo.toml ----------
lib = os.path.join(ROOT, OP, "src", "lib.rs")
text = open(lib).read()
adds = ""
for C, M in ABSORB.items():
    gate = f'#[cfg(feature = "{FEATURE_GATE[M]}")]\n' if M in FEATURE_GATE else ""
    adds += f"/// {DOCS[M]}\n{gate}pub mod {M};\n"
text = text.replace("#![deny(missing_docs)]\n", "#![deny(missing_docs)]\n\n" + adds, 1)
open(lib, "w").write(text)
ct = os.path.join(ROOT, OP, "Cargo.toml")
text = open(ct).read()
existing = set(re.findall(r"^([A-Za-z0-9_-]+)\s*=", re.search(r"\[dependencies\](.*?)(?=^\[|\Z)", text, re.M | re.S).group(1), re.M))
def pick(specs):
    specs = [re.sub(r",\s*optional\s*=\s*true", "", s) for s in specs]
    specs = sorted(set(specs), key=lambda s: (("=" not in s), -len(s)))
    return specs[0]
dep_lines = ""
for k, v in sorted(dep_union.items()):
    if k in existing: continue
    spec = pick(v)
    if k in ("solana-sdk", "bincode"):
        spec = spec.rstrip()
        spec = spec[:-1].rstrip() + ", optional = true }" if spec.endswith("}") else "{ version = " + spec + ", optional = true }"
    dep_lines += f"{k} = {spec}\n"
text = re.sub(r"^\[dependencies\]\n", "[dependencies]\n" + dep_lines, text, count=1, flags=re.M)
dev_existing = set()
mdev = re.search(r"\[dev-dependencies\](.*?)(?=^\[|\Z)", text, re.M | re.S)
if mdev: dev_existing = set(re.findall(r"^([A-Za-z0-9_-]+)\s*=", mdev.group(1), re.M))
dev_lines = "".join(f"{k} = {pick(v)}\n" for k, v in sorted(dev_union.items()) if k not in dev_existing and k not in existing and k not in dep_union)
if mdev:
    text = re.sub(r"^\[dev-dependencies\]\n", "[dev-dependencies]\n" + dev_lines, text, count=1, flags=re.M)
else:
    text = text.replace("[lints]", "[dev-dependencies]\n" + dev_lines + "\n[lints]", 1)
text = text.replace('default = ["dealer-series"]', 'default = ["dealer-series", "successor"]')
text = text.replace('dealer-series = ["dep:dclutch-trading-sbf"]', 'dealer-series = ["dep:dclutch-trading-sbf"]\n# The General successor-plan reader: a route document, `solana-sdk` and a\n# bincode plan file. Off for a browser consumer.\nsuccessor = ["dep:solana-sdk", "dep:bincode"]')
open(ct, "w").write(text)
print("rewrote", len(log), "files")
