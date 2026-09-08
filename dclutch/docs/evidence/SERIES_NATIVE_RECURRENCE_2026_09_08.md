# Series native recurrence — 2026-09-08

Owner: correctness_architecture. Native component evidence only.

Exact source `7ce009694e472971d49e89b51230101a5dac1420` passes the
retained five-action recurrence test on hbox using a new checkout and its own
workspace target. The integration owner independently checked all 3,242 source
entries against the commit, with zero mismatches. One filtered test executed and
passed; 949 were filtered. The scheduler reuse marker was absent. Exact command,
toolchain, source inventory digest and full log digest are in
[the machine-readable result](series-native-recurrence-2026-09-08/result.json).

The test retains occurrence zero's complete compiled publication through native
Prepare, Consume, next Prepare, Expire, both terminal Ticket retirements and
Close selection. Independently compiling occurrence one must produce identical
five-action artifact and ProgramSet bytes. Actual emitted RequestProfile,
Transition and Effect execution produces the native child requests and complete
Root/Ticket data successors, preserving the immutable root header. Projected
Custody Lock, source-replay closure, Realize and Abort execute their native state
owners. Stale family/root-bound request banks refuse without changing the output.

The account-profile inputs and account memory are scratch projections. Terminal
Rent observations are component fixtures. Core/Claims CPI, token movement,
AccountInfo ownership/PDA authentication, FundingV5 account creation/deletion and
atomic transaction rollback have not been established by this test. Selected
SBF stack/heap/CU acceptance and the full local-validator sequence remain open.
The implementation map is
[the Series parameterization note](../design/SERIES_RECURRING_REQUEST_PARAMETERIZATION_2026_09_08.md).
