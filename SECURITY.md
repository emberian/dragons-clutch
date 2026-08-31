## Reporting a vulnerability if you really feel like it

**Email `security@ember.software`.**

Please include:

- What you found, and the security impact you believe it has.
- Concrete steps to reproduce. A transaction signature, a program address, a
  market address, or a script.
- The commit or deployed program you tested against, if you know it.

## Hacking the shit out of the devnet deployment

Testing the deployed programs is welcome, within the ordinary courtesies of a
shared public network:

- Use your own keypairs and your own faucet SOL.
- Do not run sustained load against public RPC endpoints; other developers
  are using them.
- Prefer a local validator where you can. This repository's harnesses run the
  full protocol locally: faster, unlimited, doesn't leak your hack to the Watchers.
- Publish a writeup for the street cred.


