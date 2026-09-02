import DClutchSemantics.DirectLifecycleAbi
import DClutchSemantics.RegisteredControllerAbi
import DClutchSemantics.TsEmit

/-! Emit the registered Direct ABI as TypeScript.

`generate-registered-direct.mjs` scraped two Lean-emitted Rust modules for
sixty-one values, and it had to name all sixty-one to find them.  The five
record widths, the five magics, the thirty-eight offsets and the six encoded
fixtures are all printed here from the same schemas the Rust emitters print
from, so the browser reads the Lean object rather than a regular expression's
view of a rendering of it.

The six fixtures matter more than the offsets.  A fixture is a whole encoded
record: the browser's decoder is checked against bytes Lean produced by running
its own encoder, not against a shape someone believed.  The scraper could copy
those bytes only because the Rust emitter had already written them behind
`#[cfg(test)]`, which meant the browser's only fixtures lived in a Rust test
attribute -- an arrangement that survives exactly as long as nobody tidies the
attribute away.

`REGISTERED_TERMINAL_CANCEL` and `REGISTERED_TERMINAL_EXPIRE` are the two
values this conversion had to repair rather than move.  `Terminal.actionTag`
is what `Terminal.encode` writes into the action byte, but
`EmitRegisteredControllerAbiRust.lean` printed `0` and `1` as its own literals,
so the tag the encoder used and the constant the decoders compared against were
two independent authors that happened to agree.  Both now derive from
`actionTag`. -/

open DClutch.AbiSchema
open DClutch.TsEmit
open DClutch.DirectLifecycleAbi hiding version
open DClutch.Direct.RegisteredControllerAbi hiding magic version bytes

def main : IO Unit :=
  emit (header "EmitRegisteredDirectTs.lean" "abi:registered") [
    [ nat "REGISTERED_STATE_BYTES_VALUE" stateBytes,
      nat "REGISTERED_STATE_ABI_VERSION" DClutch.DirectLifecycleAbi.version ]
      ++ offsets StateField.rustName stateLayout
      ++ [ nat "REGISTERED_CONTROLLER_BYTES_VALUE"
             DClutch.Direct.RegisteredControllerAbi.bytes,
           nat "REGISTERED_CONTROLLER_ABI_VERSION"
             DClutch.Direct.RegisteredControllerAbi.version ]
      ++ offsets Field.rustName layout
      ++ [ nat "REGISTERED_CREATE_BYTES_VALUE" Registration.bytes,
           nat "REGISTERED_CREATE_ABI_VERSION" Registration.version ]
      ++ offsets Registration.Field.rustName Registration.layout
      ++ [ nat "REGISTERED_TERMINAL_BYTES_VALUE" Terminal.bytes,
           nat "REGISTERED_TERMINAL_ABI_VERSION" Terminal.version ]
      ++ offsets Terminal.Field.rustName Terminal.layout
      ++ [ nat "REGISTERED_TERMINAL_CANCEL" (Terminal.actionTag .cancel).toNat,
           nat "REGISTERED_TERMINAL_EXPIRE" (Terminal.actionTag .expire).toNat,
           nat "REGISTERED_RETIRE_BYTES_VALUE" Retirement.bytes,
           nat "REGISTERED_RETIRE_ABI_VERSION" Retirement.version ]
      ++ offsets Retirement.Field.rustName Retirement.layout,
    bytesBlock "REGISTERED_STATE_MAGIC_BYTES" stateMagic
      ++ bytesBlock "REGISTERED_STATE_EXAMPLE"
           (encodeRegisteredIntentStateV1
             DClutch.DirectLifecycleAbi.Examples.registeredState)
      ++ bytesBlock "REGISTERED_CONTROLLER_MAGIC_BYTES"
           DClutch.Direct.RegisteredControllerAbi.magic
      ++ bytesBlock "REGISTERED_CONTROLLER_EXAMPLE" (encode exampleInstruction)
      ++ bytesBlock "REGISTERED_CREATE_MAGIC_BYTES" Registration.magic
      ++ bytesBlock "REGISTERED_CREATE_EXAMPLE"
           (Registration.encode Registration.exampleInstruction)
      ++ bytesBlock "REGISTERED_TERMINAL_MAGIC_BYTES" Terminal.magic
      ++ bytesBlock "REGISTERED_TERMINAL_CANCEL_EXAMPLE"
           (Terminal.encode Terminal.exampleCancel)
      ++ bytesBlock "REGISTERED_TERMINAL_EXPIRE_EXAMPLE"
           (Terminal.encode Terminal.exampleExpire)
      ++ bytesBlock "REGISTERED_RETIRE_MAGIC_BYTES" Retirement.magic
      ++ bytesBlock "REGISTERED_RETIRE_EXAMPLE"
           (Retirement.encode Retirement.exampleInstruction)
  ]
