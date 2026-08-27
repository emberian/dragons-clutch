# Primary-source inventory

Retrieved 2026-08-18. This laboratory used only official Solana/SPL
documentation and the official `solana-program/token-2022` source repository.

| Source | Pin / observed version | Used for |
| --- | --- | --- |
| [Token-2022 overview](https://www.solana-program.com/docs/token-2022) | retrieved 2026-08-18; page lists Token-2022 program `TokenzQd...` and the then-current extension families | Program identity, base-layout relationship, extension model |
| [Token-2022 extension guide](https://www.solana-program.com/docs/token-2022/extensions) | retrieved 2026-08-18 | Transfer-fee withholding, mutable default state, non-transferability, permanent-delegate powers, CPI Guard, and transfer-hook CPI/extra-account behavior |
| [Transfer Hook interface](https://www.solana-program.com/docs/transfer-hook-interface) | retrieved 2026-08-18 | Token-2022 invokes the configured hook program during transfers |
| [Token-2022 status](https://www.solana-program.com/docs/token-2022/status) | retrieved 2026-08-18 | Confidential-transfer runtime dependency and official statement that the deployment remained upgradeable on the retrieved page |
| [Solana mint-account guide](https://solana.com/docs/tokens/basics/create-mint) | retrieved 2026-08-18 | `u64` supply, `u8` decimals, initialization, mint authority, and freeze authority fields |
| [Solana authority guide](https://solana.com/docs/tokens/basics/set-authority) | retrieved 2026-08-18 | Revocation of mint/freeze authority and token-account close-authority model |
| [Solana Default State guide](https://solana.com/docs/tokens/extensions/default-state) | retrieved 2026-08-18 | `DefaultAccountState` is a mint extension; freeze authority can update the default for future accounts |
| [Official Token-2022 source](https://github.com/solana-program/token-2022/tree/426400f29d5f1e299be8b353fdf13f22358fbd68) | commit `426400f29d5f1e299be8b353fdf13f22358fbd68`, committed 2026-08-17; package `spl-token-2022` 11.0.0; interface dependency 3.1.0 | Exact production `ExtensionType` discriminants and mint/account location classification |
| [`ExtensionType` source at the pin](https://github.com/solana-program/token-2022/blob/426400f29d5f1e299be8b353fdf13f22358fbd68/interface/src/extension/mod.rs) | same commit | Enum order `0..28`, location mapping, and required account-extension relationships |

The repository HEAD is a reproducibility pin for this research snapshot, not a
claim that it is a deployed cluster binary or the release a future adapter must
use. Before implementation, the adapter lane must separately pin and authenticate
the deployed Token and Token-2022 program identities, loader/upgrade state,
interface crate, extension parser, and runtime version it actually targets.

