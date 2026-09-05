# dclutch-accelerator-sbf

The one readonly, stateless accelerator. Trading composes every invocation,
CPIs this program with the admitted frame, and reads back one typed
acknowledgement; this program writes nothing it does not own, invokes no
child, and holds no protocol state.

Three arms, one program id, one refusal band (`0xC000`):

| arm | module | transport | refusals |
| --- | --- | --- | --- |
| General clearing | `src/general.rs` | `AdmittedAcceleratorRequestV2` (chunked bank, output page) | `GeneralAcceleratorSbfErrorV3`, `0xC000..` (unchanged from the standalone General accelerator) |
| Dealer LP / equity | `src/dealer.rs` | `AdmittedAcceleratorRequestV2` | `DealerAcceleratorSbfErrorV4`, sub-band `0xC100..` |
| Series shadow | `src/series/` | `ShadowRequestV3` | `SeriesShadowSbfErrorV4`, sub-band `0xC200..` |

`src/lib.rs` selects the arm from the same fact each arm authenticates: the
Shadow request names itself by its leading magic; the two admitted transports
carry the family request in the top-level Trading instruction, which the
Instructions sysvar exposes at one fixed coordinate of the admitted frame,
and its leading eight bytes are the family magic (Dealer magics select the
Dealer arm, anything else the General arm). The selection allocates nothing
and refuses nothing; the arm re-reads the same bytes under its own conjuncts
and refuses by its own name.

The Series arm's artifact bundle is selected at build time through
`DCLUTCH_SERIES_SHADOW_GENERATED_INCLUDE` (see `build.rs` and `generator/`);
an ELF built without one refuses every Shadow request as `NoSelectedRelease`.
`SERIES_SHADOW.md` is that arm's design note.

Program-tests, one per arm, each loading `dclutch_accelerator_sbf.so`:
`program-test/` (General, with `test-programs/general-caller`),
`dealer-program-test/` (Dealer, with `test-programs/dealer-caller`), and
`series-program-test/`.
