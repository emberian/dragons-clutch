import DClutchSemantics.IR
import DClutchSemantics.Direct
import DClutchSemantics.DirectLifecycle
import DClutchSemantics.DirectLifecycleProgram
import DClutchSemantics.DirectLifecycleAbi
import DClutchSemantics.RegisteredPhysical
import DClutchSemantics.DirectProofs
import DClutchSemantics.Codec
import DClutchSemantics.Examples
import DClutchSemantics.SbfProfile
import DClutchSemantics.Physical
import DClutchSemantics.ClaimSbfProfile
import DClutchSemantics.TransitionVM
import DClutchSemantics.DirectProgram
import DClutchSemantics.CompiledPhysical
import DClutchSemantics.DirectControllerCodec
import DClutchSemantics.EconomicKernel
import DClutchSemantics.EconomicExamples
import DClutchSemantics.SourceResolution
import DClutchSemantics.SourceResolutionAbi

/-!
Fresh Lean-owned semantics for dClutch's compact protocol specializer.

This package intentionally has no dependency on the neighboring research
repositories or on the Rust implementation.  The Rust implementation is a
differential oracle, not a source of formal definitions.
-/
