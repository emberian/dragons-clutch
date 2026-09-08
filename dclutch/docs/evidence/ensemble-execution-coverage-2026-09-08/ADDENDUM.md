# Ensemble accepted execution: census and capitalization addendum

Date: 2026-09-08. This addendum qualifies
[`ENSEMBLE_ACCEPTED_LOCAL_VALIDATOR_2026_09_08.md`](../ENSEMBLE_ACCEPTED_LOCAL_VALIDATOR_2026_09_08.md)
without changing that dated record.

## Corrected verdict

The retained four-member, quorum-three account graph proves that the sealed
exact-551 Resolution ELF accepted three captures, one fold, and reclaim of the
uncaptured fourth seat. It does not prove a complete fresh founding-to-terminal
producer vertical. The retry host paid the success-certificate coordinate and
the vacant fourth seat after all three captures. The original fresh producer
likewise paid each fragment seat immediately before its capture. Those payments
made the retained execution diagnostic of the deployed Fold and Reclaim routes,
but they did not satisfy the architecture rule that deferred creation be
precommitted and prepaid at founding.

An independent bounded finalized RPC read at source revision `154a2cd5`
confirmed both accepted signatures and compute totals, all four retained
poststate hashes, disappearance of the reclaimed seat, the Fold funding-ledger
delta of -2 lamports with +1 to each of two secondary captors, and the exact
3,062,400-lamport Reclaim credit to the recorded beneficiary. The retained
verification is:

`hbox:/home/hbox/dclutch-ensemble-four-member-q3-551ffc1b-host4d-port43320-20260908/root-accepted-verification-20260908.json`

SHA-256:
`fb7ebaaa2a98689aacfa4ef13029efc39b9dcfdadb5835bce8f45ebb3bcb7376`.

## Exact census evidence

`ledger.json` is the byte-for-byte output of `tools/gate census observe` over
the checked `bindings.json`, `programs.json`, and finalized native transaction
evidence in `evidence.json`. The observations require the deployed Resolution
program identity and the actual native instruction bytes. They admit:

- `resolution/process_ensemble_fold#EnsembleFold`, action 8, exactly 32 bytes,
  slot 20,927, signature `ukYyq4xpZHk3N59NQFBhtVtuQE71Wsjw4JYQwtZRpZa1yezAyU774p9FuNh8BEBshWQnJH4RHACjiVVmv9RZeTt`, 383,676 CU;
- `resolution/process_reclaim_member_seat#ReclaimMemberSeat`, action 9,
  exactly 40 bytes including the seven reserved zero bytes, slot 21,057,
  signature `RQkogBYrJ2oGTmECtZvorAM1rVUg88sMr5uRHrNhSYAXYWycE2tuKWsmUmNZy2chGSDrZDGNeZTd2njYENMAGLN`, 41,986 CU.

The selector also requires `DCLTRIX1`, schema version 1, zero header-reserved
bytes, and a positive terminal sequence. Artifact SHA-256 values are:

| artifact | SHA-256 |
|---|---|
| `evidence.json` | `a66e54215c7fece939ac461a599971514965b1647733db697d02147c1b892864` |
| `bindings.json` | `7853a986a3e0ebb624d541d36aacac9616695e5509822c9839ee1c08857bc6f0` |
| `programs.json` | `440ffc34a743db4903241adc13baa2f36e66bc03a9d6dce1db08df83599be3f4` |
| `ledger.json` | `c9683a66e493fe4d6f0e15d85e9c5633d626506ccc0468ad8ef2475fca3ffffa` |

The three finalized capture transactions remain retained evidence, but this
ledger does not admit them. Their `DCLTPRQ3` provider instruction route is
selected by a predicate, instruction length, and magic conjunction that the
current census binding cannot express. Adding a hand-authored capture row
would overstate what the census checked.

## Capitalization owner and timing

No native founding funding row or escrow currently owns rent for the success
certificate or fragment seats. The demo capability quote gives each selected
Resolution row one lamport each of rent, creation, and bounty. During
`ActivateFund`, the row's rent and creation amounts leave the funding ledger
for the Market beneficiary; only bounty principal remains available for later
release. There is no row keyed to a certificate or fragment-seat coordinate.

The bounded producer repair prepays the sequence-one success certificate and
all `k` deterministic member seats from the campaign payer after Resolution
funding activation and before `core-funding-accept-v1`. A fresh capture now
refuses unless founding already prepaid its member seat, and the fresh fold
path refuses a late terminal top-up. The retained-ledger resume path keeps an
explicitly labelled top-up option so historical accepted captures can still be
diagnosed. The fold receipt remains worker-funded because it exists only when
a worker performs the fold.

This repair prevents the current fresh campaign from discovering missing rent
at capture or fold. It is still after `DCLTGMF3`/Open has created the Core and
claims liabilities, is still funded directly by the campaign payer, and does
not cover a founding path that reports funding as consumed atomically. Full
closure therefore requires a protocol-owned founding funding semantic that
capitalizes the certificate and all member seats before liabilities exist.

## Remaining route scope

The accepted run covers one direct-Pyth configuration with `k = 4`, `q = 3`,
three captured members, and one vacant member. It does not establish other
member/quorum shapes, multiple vacancies, below-quorum ladder behavior and its
fold/crank exclusivity, sponsored-push or relayed provider families, retirement
of written fragment seats, or devnet behavior.
