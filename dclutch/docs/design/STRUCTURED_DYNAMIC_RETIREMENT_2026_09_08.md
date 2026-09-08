# Structured retirement in the selected release

The Structured ProgramSet contains its root activation and nine action entries:
five representation operations, receipt and coordinate creation, and coordinate
and receipt retirement. Retirement does not select another capability root or
commit a coefficient vector before the per-Market descriptor exists.

Receipt retirement uses the existing compact Claims header, wrapped in a
24-byte transport prefix. The prefix declares only an untrusted account-span
width and the exact 400-byte borrowed header width. Profile13 checks the trailing
span's geometry; RequestProfileV3 and EffectV4 borrow that one header. The
maximum span is five vacancy accounts per selected representation coordinate.
The actual support can be sparse and is derived from the finalized descriptor.

Trading authenticates the Registry descriptor and its Claims representation
authority, then asks the Claims kernel to specialize the compact header into
the existing LifecycleRequestV2 child. The kernel enumerates ordered nonzero
coefficients, joins the supplied vacancy keys, and calls the existing lifecycle
prepare operation. The exact specialized bytes are shared by caller-authority
derivation, CPI, and receipt verification. Claims remains the semantic owner of
support, resource vacancy, token supply, and rent retirement. Trading does not
introduce a new dispatched Claims wire or persist another economic fact.

The EffectV4 extension bitset bounds the transport to fewer than 64 extra
accounts. This is a chain transport bound; the current selected Structured
representation profile is already narrower. Lifting it requires the canonical
Effect successor and measured account/packet execution, not truncating support
or changing coefficient authority.

Focused kernel controls cover a sparse `[3, 0, 7]` descriptor, omitted and extra
account groups, malformed transport widths and action substitution, and atomic
output preservation. Nonzero declared receipt supply refuses before compact
encoding. Selected-release tests check the single ProgramSet and publication
closure. These are source tests, not accepted validator lifecycle evidence.
The real-ELF hostile support control and fresh validator lifecycle remain
required before this vertical is complete.
