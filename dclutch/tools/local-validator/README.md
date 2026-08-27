# Local-validator profiles

`dclutch-successor-validator` is the one launcher in this tree. It is a keyless
wrapper around the pinned `solana-test-validator` and starts a fresh localhost
ledger carrying the seven-role successor release set.

Two earlier launchers -- `dclutch-local-validator` (the single-ELF gen-2
profile) and `dclutch-integrated-validator` -- were banished to
~/dev/dclutch-legacy/local-validator/ with the DCLTCAT1 stratum, along with the
old `bootstrap/` host client that initialized the Pyth provider profile they
made. They started validators for a Market representation nothing in this tree
writes; `bootstrap/successor/` is the live one and stays.

The pinned Pyth fixture verification those launchers owned did NOT go with
them: `verify_fixtures` is ported into `dclutch-successor-validator`, which was
already its only live caller, and still enforces the two-way cover over
`fixture-sha256.txt` (every pin resolves to a matching file, and the directory
holds no unpinned file).

## Immutable multi-program successor profile

`dclutch-successor-validator` uses a fresh ledger on RPC `20890` — or on the
base `--rpc-port` / `$DCLUTCH_RPC_PORT` names, from which the whole block is
derived (`faucet BASE+2`, `gossip BASE+3`, `dynamic BASE+10..BASE+41`; BASE
20890 reproduces the historical `20890-20931` byte for byte). `status` and
`stop` read the base back out of the ledger's own profile, so a ledger started
on a nonstandard base is still addressable by path alone. Given
`--supervisor-pid PID` the validator is bound to that process's lifetime and is
killed when it dies **even if that process is SIGKILLed and never runs its own
cleanup**, which is how a finished campaign once left a validator with PPID 1
holding the pinned port. It requires
separately attested, actual Registry, Core, Claims, Trading, Resolution,
Custody, and RentCredit ELFs under seven pairwise-distinct program IDs. It also
loads the committed real Pyth router and receiver ELFs. Unlike
`--upgradeable-program`, the prepared ProgramData accounts preserve the
canonical fixed 45-byte Loader V3 metadata span: variant `3`, slot `0`,
authority `None`, zero authority padding, then the exact ELF.

The standalone [`successor`](bootstrap/successor/README.md) package prepares a
hash-pinned infrastructure plan containing all seven distinct ArtifactRelease
bodies, the five-role execution release set, the captured local-Pyth release,
and the exact expected 144-byte Core-owned Registry/Rent infrastructure
profile. Genesis contains six immutable Loader programs plus an explicitly
pre-init authority-bearing Core ProgramData account; Core is not recognized as
the immutable release until its authority is revoked to `None`. Finalized
Registry record bodies are the only other genesis fixtures.

Core `d6d5f2d` now provides verifier-clean infrastructure-init and Found31. The
remaining gate is one same-process supervisor retaining an ephemeral Core
upgrade authority only in memory across init, Loader revocation, immutable
release activation, and Found. The bootstrap `run` command currently validates
the complete plan and provider evidence, then fails before opening an RPC
connection or signing. It does not retain the obsolete direct Resolution V1
path or genesis-prepare Market/Source/Funding state. No checked production
release or captured Pyth deployment identity is claimed.
