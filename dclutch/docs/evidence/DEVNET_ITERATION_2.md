# Devnet iteration 2

Status: implementation in progress  
Cluster: Solana devnet only  
Decision set: 0012 and 0013

This record starts before the first external write. It will be extended from
the checked source revision through the program update, release activation,
fresh Market lifecycle, public ledger, and site deployment. A local test or a
devnet execution is not mainnet evidence.

## 1. Pre-write public baseline

Observed at `2026-08-28T03:45:47Z` through one bounded finalized
`getMultipleAccounts` read against `https://api.devnet.solana.com`. Both RPC
responses reported context slot **489,182,510**. No signer, wallet file, or
browser session was read.

Every Program account was executable and owned by
`BPFLoaderUpgradeab1e11111111111111111111111`. Every ProgramData account was
non-executable, owned by that same Loader, and carried the retained authority:

```text
4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP
```

| role | program | ProgramData | slot | payload bytes | ProgramData lamports | payload SHA-256 |
|---|---|---|---:|---:|---:|---|
| Registry | `Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj` | `ENRSwrUEymWaXyrNtyD4QXXXk3tsTmcTGPTUFvnpsRVz` | 489,100,383 | 207,072 | 1,442,425,200 | `e1f4a20f0fefb60ad8f809f153c4403363d298d5eb11b88e29abe404048ac6e1` |
| Rent | `DgfYeuorJUmnktxgCmUXy65f6MFBGcc1aMQoauxoJCY3` | `78MW6W4iPzBVLceAwTL51CtyLcpcFM2iGVMDbzZtUFmy` | 489,100,242 | 137,608 | 958,955,760 | `3b857b2236522c29e17b7d73cf27df6e6028fd8298a52df386753638f915ff79` |
| Custody | `34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH` | `EhB7hHJ7vsCW3nCeqbxbJrn5Jsi6gbqwpVhoLMPZ8ENf` | 489,100,460 | 360,328 | 2,509,086,960 | `d171cf742391dcc6ff152171657187d6a62538f38cedc9ce048af457b16746f1` |
| Resolution | `2GHmxBawHTmwDRzqXuqdeC9A9Gj2HzucRd29wGpfgzmd` | `2QFBQJdLBXAnJWTVK8KeeUtWZEFhQqqN2CbkrWjMjY6f` | 489,100,560 | 588,336 | 4,096,022,640 | `03842494bc1604b7f4806962157f93529056848f51499a4e0de771d1b8ab1fbf` |
| Claims | `85hwTeQGabwFRs71Hafvngb1UmHb6dQoumBv3VV4epNN` | `4La2511ddSxUcAQfdhKvEeGEasih3TStbQWVFEQKd34j` | 489,100,803 | 1,010,496 | 7,034,256,240 | `51967830f17ab6ebad074fbaf178482c027910bc9d14a8ade070e17004b84b8a` |
| Trading | `5ywjTNdo6DGTe7bC8p9CgFYWFrBNePx61xeXp8Cdhbkk` | `AE1cWbCvXedE23XH3otSxvDQ7xVx7WLNMYDc8y8rqkrn` | 489,100,942 | 1,325,848 | 9,229,106,160 | `7facb8e58e45843f46b9d3d572ced5e45507bfcbfb2250e865b5427baa1b9d3c` |
| Core | `HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N` | `AD6mb5SP6yqc5GFexf3xhpr1wKaZQhS7Hrt41iZhKxaN` | 489,100,672 | 934,088 | 6,502,456,560 | `e0cc7109da7a7b2b94cfa5a0f00a63c40ce44519f7d0186b6c1fbfe39b68f0ee` |

The ProgramData lamports sum to **31,772,309,520**, exactly DEPLOY_1's parked
rent arithmetic. The raw ProgramData-body hashes, which include Loader
metadata, were also retained in the local pre-write evidence:

| role | full ProgramData body SHA-256 |
|---|---|
| Registry | `f8aaca90165d50f5020fa2f7f3377674813bbc5a5cda7c361fb110d752d653e0` |
| Rent | `acf94e2340067e3ededfeaa9c36d7d877d307a32740539fc2cb5244b844812ee` |
| Custody | `d83020025e037ef42e01cf88a2368219c10b7a2f6011254edbe7a71336a303c2` |
| Resolution | `47082d659047011c046b7c26b5af4b0fe402b4255fbe81a4a3763140cbff734d` |
| Claims | `bff08ee426a9a6c98a1d11b0ab92fb774f70b1cab1aac0735ea6c990dc96e1b8` |
| Trading | `e9b65886b144a556bd68cf201a736d7af64c61ebcbd75a5f71f4543f7c71b5a7` |
| Core | `b76f7a0c2886a32a21986ecf5038b51aa9b28fcf6f37f940c1a11fc35d2ea100` |

### 1.1 Second pre-write admission read

During the `2026-08-28T05:12Z` minute, three bounded, key-free finalized
JSON-RPC reads rechecked the devnet genesis, all seven Program accounts, and
all seven ProgramData headers. The account observation returned context slot
**489,212,834**. Every permanent Program/ProgramData address, Loader owner,
executable flag, deployment slot, ProgramData space, and upgrade authority
still matched the table above and `DEPLOY_1`. Because Loader V3 writes the
current slot into ProgramData on Upgrade, the unchanged seven deployment slots
also establish that no role was upgraded between the two observations. This
second read did not claim a new payload digest; the byte-digest baseline above
remains the pre-write byte authority.

A separate two-call genesis-and-balance read returned finalized context slot
**489,213,774** and exactly **32,185,584,146 lamports**
(**32.185584146 SOL**) for the retained deployer. No transaction was signed or
submitted. This is the arithmetic baseline that must be read again immediately
before the first program-extension or Upgrade transaction.

## 2. Public-site baseline

This section is a timestamped baseline, not a floating description of the
currently deployed site. Each statement below is bound to its named wrapper
commit, source commit, workflow run, and HTTP observation. Later deployments
must add their own coordinates in §8; do not infer current site contents from
this checkpoint.

The same pre-write check requested `https://clutch.dregg.pro/` over HTTPS. It
returned HTTP 200 from GitHub Pages. The response identified the current page
as last modified `2026-08-28T02:30:25Z`. This establishes reachability only;
the post-deployment cold-browser and chain-reconciliation checks remain owed.

The public checkpoint was subsequently rebuilt from wrapper commit
`e7f49c022e5913dbbc62005c3781e58752c1de34`, whose `dclutch/` subtree was
byte-for-byte source commit `3b2c7bdd`. The intentionally manual Pages run
`33143867158` passed the frontend export, documentation assembly, internal-link
check, artifact upload, and deployment. An outside HTTPS read then returned
HTTP 200 with `Last-Modified: Fri, 28 Aug 2026 05:09:20 GMT`. The app root,
Markets, Activity, Portfolio, Smoke, Bounty, and every sampled linked guide,
ABI, refusal, evidence, README, and notices page returned 200. The landing page
states the current external boundary plainly: seven programs are live on
devnet, there is no open/sample market, and there is nothing to buy or place at
risk. This remains a reachability and reader-truth checkpoint, not the owed
post-Market cold-browser acceptance.

At the time of this checkpoint, the wrapper source later advanced to
`9b965990` with the frame and replay-safety checkpoint, but no Pages workflow
was dispatched for that source-only sync. The last deployment observed by this
record was therefore the exact, verified `e7f49c02` artifact. That sentence is
date-bound and must not be read as the current deployment after a later manual
Pages run.

## 3. Implementation gates already landed

- `ee309c25`, amended by `22adb790`: decision 0013 adopts the projected
  founding v2 source-cap and controller-subset funding design.
- `ecf1f2c0`: publication and activation resume exact partial progress;
  substrate preflight checks slot, authority, Loader owner, and privilege.
  The locked successor suite passed 54/54.
- `ebeca73c`: SourceMaterialV3 makes the source-policy graph acyclic and
  derives the complete-set cap at the named integer boundary. Source tests
  passed 67/67; operator Found tests passed 10/10; Lean regenerated
  byte-identically.
- `600998f7`: the checked-release gate enumerates all 13 shipped SBF links and
  refuses zero-diagnostic evidence unless every link freshly compiled in the
  current run. Its focused adversarial suite passed 11/11.

No item in this section is a deployment claim. The source revision, checked
ELFs, all-link frame diagnostic, 20-seed compute evidence, program writes,
release records, market transactions, ledger, and site revision remain to be
recorded below after they exist.

## 4. Checked source and build

Pending.

## 5. Program update and wallet arithmetic

Pending. The opening wallet checkpoint from DEPLOY_1 was independently re-read
at finalized slot 489,213,774 as **32,185,584,146 lamports**; it must be read
again immediately before the first signed transaction. No balance in this
document is a fee, treasury, reserve, bounty, or protocol principal.

## 6. Release generation and activation

Pending.

## 7. Fresh Markets and exterior lifecycle

Pending.

## 8. Public ledger and site

Pending.
