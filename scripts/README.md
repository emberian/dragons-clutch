# Planned offline scripts

Future scripts may reproduce proofs/builds, run trust and provenance audits,
generate canonical fixtures, measure local program-test resources, and build the
static release artifact.

Scripts must default to offline/local operation, avoid wallet and secret paths,
record exact inputs and tool versions, and refuse any remote mutation or network
deployment action. A convenience command may not weaken an invariant or proof
gate to obtain a green result.
