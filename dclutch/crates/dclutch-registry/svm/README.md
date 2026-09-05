# dClutch Registry SVM wire boundary

This SDK-free crate owns the exact Registry activation and reauthentication
instruction wires, the authenticated-role CPI receipt, and hostile Loader V3
byte views.

Loader ProgramData parsing always begins the complete deployed ELF byte tail at
the canonical fixed offset 45. Immutable ProgramData must carry zero bytes in
the inactive authority allocation. The view alone conveys no authority: the
SBF adapter separately authenticates keys, Loader ownership, executable flags,
Program→ProgramData linkage, canonical ProgramData PDA, deployment slot, and
the SHA-256 digest of the entire tail.
