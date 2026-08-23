/*
 * Reviewed implementation inventory for the static protocol console.
 *
 * This is contract metadata, not chain state, a release manifest, or an
 * executable-capability claim. The source revisions name isolated protocol
 * lanes which still require an accepted joined release.
 */
(function (root) {
  "use strict";

  const inventory = {
    schema: "dragons-clutch.static-protocol-inventory.v1",
    boundary: {
      official: false,
      live: false,
      networkAccess: false,
      walletAccess: false,
      signing: false,
      submission: false,
      statement: "This client validates user-supplied projections and constructs unsigned bytes locally. It neither reads nor authenticates chain state."
    },
    clusters: [
      {
        id: "localnet",
        label: "Local validator",
        rpcEndpoint: "http://127.0.0.1:8899",
        genesisHash: null,
        deployment: null,
        note: "A conventional local target only. No validator or deployment is discovered or started by this page."
      },
      {
        id: "devnet",
        label: "Solana Devnet",
        rpcEndpoint: "https://api.devnet.solana.com",
        genesisHash: null,
        deployment: null,
        note: "A construction target only. No checked Dragon's Clutch deployment is named."
      },
      {
        id: "testnet",
        label: "Solana Testnet",
        rpcEndpoint: "https://api.testnet.solana.com",
        genesisHash: null,
        deployment: null,
        note: "A construction target only. No checked Dragon's Clutch deployment is named."
      },
      {
        id: "mainnet-beta",
        label: "Solana Mainnet Beta",
        rpcEndpoint: "https://api.mainnet-beta.solana.com",
        genesisHash: null,
        deployment: null,
        note: "Configuration vocabulary only. This is not an official client or mainnet deployment claim."
      }
    ],
    releaseRequirements: [
      "exact cluster genesis hash",
      "base program and ProgramData addresses",
      "deployment slot",
      "complete ELF SHA-256",
      "release-manifest SHA-256",
      "source commit",
      "capability-profile identity"
    ],
    components: [
      {
        id: "general-v2-owner-settlement",
        label: "General V2 owner settlement",
        sourceCommit: "85a11121b1e89640976aa4fb77fdebd42559f27d",
        contract: "clutch-owner-settlement",
        state: "pure-contract",
        facts: ["one canonical row per participating owner", "buyer ceil and seller floor occur once at TerminalOwnerFloor", "selected owner fees debit signed buy reservation cash", "seller-only owners carry explicit zero-fee rows"],
        wire: { semanticBodyBytes: 288, outerAccount: null, outerAction: null }
      },
      {
        id: "nonzero-fee-runtime",
        label: "Exact nonzero fees",
        sourceCommit: "681537db67c80d1e03b535c3aa96e84200d668c2",
        contract: "clutch-fee-runtime-contract",
        state: "pure-contract",
        facts: ["u128 owner-scoped carry", "selected candidate-wide fee record", "payer and recipient allocation", "ordinary treasury Position credit", "future fee revenue cannot capitalize liveness"],
        wire: {
          innerAccounts: [
            ["DCFEESEL", 336], ["DCFEECRY", 128], ["DCFEEPAY", 2680],
            ["DCFEEREC", 2640], ["DCFEETRY", 144]
          ],
          outerAccount: null,
          outerAction: null
        }
      },
      {
        id: "source-plane-v3",
        label: "SourcePlane V3",
        sourceCommit: "20dee65005c8953a6c72e432195bcf87a7717c09",
        contract: "clutch-source-plane-v3-runtime",
        state: "pure-runtime-contract",
        facts: ["reviewed adapter/parser deployment binding", "immutable generation request", "bucket-close and lateness gates", "page/head/window/seal/result lineage", "durable reopen partition", "segregated source work funding"],
        wire: { outerAccount: null, outerAction: null }
      },
      {
        id: "prepaid-liveness-v1",
        label: "Prepaid liveness runtime",
        sourceCommit: "d5d76c39327928be2aeeeb190de8a91734159580",
        contract: "clutch-liveness::runtime_v1",
        state: "pure-runtime-contract",
        facts: ["seven ordered lamport compartments", "four bounded terminal paths", "principal, donation, work and rent remain disjoint", "fees, collateral and Hoard are not funding sources"],
        wire: { policyMagic: "DCLPOL01", policyBytes: 1132, accountMagic: "DCLACC01", accountBytes: 464, intentMagic: "DCLINT01", intentBytes: 272, outerAction: null }
      },
      {
        id: "product-series-v2",
        label: "Product compiler and Series V2",
        sourceCommit: "cd51aba556a38fcba5326007f4f70e28cd825835",
        contract: "clutch-product-series + Source/Series successor family",
        state: "reserved-disabled-sbf-laboratory",
        facts: ["V5 Series and V2 price-owning market identity", "five segregated funding components", "created/lapsed ordinal lifecycle", "exact six-action successor wire"],
        wire: { familyTag: 77, familyVersion: 2, actions: ["RegisterSeries", "ActivateFunding", "AdvanceOccurrence", "LapseOccurrence", "ObserveDonation", "CloseFunding"], runtimeEnabled: false }
      },
      {
        id: "structured-claim-runtime",
        label: "Structured claims",
        sourceCommit: "8838df3b3e52d372b317a7e51df9fd3034e8bc43",
        contract: "clutch-structured-claim-runtime-contract",
        state: "pure-runtime-contract",
        facts: ["exact 384-byte descriptor", "canonical and full-vector wrap/unwind", "donation compaction", "terminal redemption", "zero-supply permanent retirement tombstone"],
        wire: { proposedAccountTag: 136, accountVersion: 1, accountBytes: 384, familyTag: 75, familyVersion: 1, localActions: 8, centralActionsAllocated: false }
      }
    ],
    capabilities: [
      {
        id: "projection-import",
        label: "Import account projection",
        enabled: true,
        reason: "Local JSON parsing and exact-integer reconciliation are available. The projection remains untrusted until independently authenticated against its named release and accounts."
      },
      {
        id: "general-settlement-view",
        label: "Owner settlement / fee view",
        enabled: true,
        reason: "The pure owner-level equations and fee joins are frozen and can be recomputed from a supplied projection."
      },
      {
        id: "series-wire-export",
        label: "Series unsigned wire export",
        enabled: true,
        reason: "The Source/Series V2 extension envelope and all six action payloads have exact codecs. The emitted instruction is expected to refuse because the executable capability set is empty."
      },
      {
        id: "liveness-inner-export",
        label: "Liveness inner-intent export",
        enabled: true,
        reason: "DCLINT01 has an exact 272-byte codec. No central outer action or account-meta route is allocated, so only the inner contract bytes are emitted."
      },
      {
        id: "structured-inner-export",
        label: "Structured-claim payload export",
        enabled: true,
        reason: "The family-local payloads are exact. The central registry has not allocated the local actions or descriptor account, so no outer request is emitted."
      },
      {
        id: "general-transaction-export",
        label: "General V2 transaction export",
        enabled: false,
        reason: "General V2 action names are reserved, but owner settlement, fee and terminal-pot outer payload/account codecs are not frozen together."
      },
      {
        id: "source-transaction-export",
        label: "SourcePlane transaction export",
        enabled: false,
        reason: "SourcePlane V3 still needs central dispatcher tags, exact account-meta tables and SBF construction of PDA/CPI/sysvar attestations."
      },
      {
        id: "rpc-read",
        label: "RPC account reads",
        enabled: false,
        reason: "This static build has connect-src none. Import bounded account observations produced by an external authenticated collector."
      },
      {
        id: "wallet-sign-submit",
        label: "Wallet, signing and submission",
        enabled: false,
        reason: "No wallet session, signer, signature, sendTransaction or submit path exists in this build."
      }
    ],
    orders: {
      livenessCompartments: ["source", "candidate", "clearing", "settlement", "resolution", "retirement", "recovery"],
      seriesComponents: ["market-core", "recovery-reserve", "source-work", "liquidity-facility", "wrapper-set"]
    }
  };

  root.GlassProtocolContracts = Object.freeze(inventory);
})(typeof globalThis === "object" ? globalThis : this);
