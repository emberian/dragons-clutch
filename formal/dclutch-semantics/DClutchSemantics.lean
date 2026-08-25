import DClutchSemantics.IR
import DClutchSemantics.Direct
import DClutchSemantics.DirectProofs
import DClutchSemantics.Codec
import DClutchSemantics.Examples
import DClutchSemantics.SbfProfile
import DClutchSemantics.Physical

/-!
Fresh Lean-owned semantics for dClutch's compact protocol specializer.

This package intentionally has no dependency on the neighboring research
repositories or on the Rust implementation.  The Rust implementation is a
differential oracle, not a source of formal definitions.
-/
