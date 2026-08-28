use crate::model::MarketProfileV1;

pub(crate) const OUTCOME_COUNT: usize = 4;
pub(crate) const PRICE_SCALE: u64 = 1_000;

#[derive(Clone, Copy)]
pub(crate) struct WalletDefinition {
    pub(crate) id: &'static str,
    pub(crate) initial_collateral: u64,
    pub(crate) complete_sets: u64,
}

#[derive(Clone, Copy)]
pub(crate) struct TradeDefinition {
    pub(crate) id: &'static str,
    pub(crate) seller: &'static str,
    pub(crate) buyer: &'static str,
    pub(crate) outcome: usize,
    pub(crate) fill: u64,
    pub(crate) remaining_before: u64,
    pub(crate) execution_price: u64,
}

#[derive(Clone, Copy)]
pub(crate) enum ResolutionDefinition {
    Categorical {
        selector: usize,
    },
    GradedFailure {
        payout_scale: u64,
        payouts: [u64; OUTCOME_COUNT],
    },
}

impl ResolutionDefinition {
    pub(crate) const fn payout_scale(self) -> u64 {
        match self {
            Self::Categorical { .. } => 1,
            Self::GradedFailure { payout_scale, .. } => payout_scale,
        }
    }

    pub(crate) const fn selector(self) -> usize {
        match self {
            Self::Categorical { selector } => selector,
            Self::GradedFailure { .. } => OUTCOME_COUNT - 1,
        }
    }

    pub(crate) const fn payouts(self) -> [u64; OUTCOME_COUNT] {
        match self {
            Self::Categorical { selector: 0 } => [1, 0, 0, 0],
            Self::Categorical { selector: 1 } => [0, 1, 0, 0],
            Self::Categorical { selector: 2 } => [0, 0, 1, 0],
            Self::Categorical { selector: 3 } => [0, 0, 0, 1],
            Self::Categorical { .. } => [0; OUTCOME_COUNT],
            Self::GradedFailure { payouts, .. } => payouts,
        }
    }
}

pub(crate) struct ScenarioDefinition {
    pub(crate) id: &'static str,
    pub(crate) filename: &'static str,
    pub(crate) title: &'static str,
    pub(crate) description: &'static str,
    pub(crate) profile: MarketProfileV1,
    pub(crate) wallets: &'static [WalletDefinition],
    pub(crate) trades: &'static [TradeDefinition],
    pub(crate) resolution: ResolutionDefinition,
}

const FLAGSHIP_WALLETS: &[WalletDefinition] = &[
    WalletDefinition {
        id: "ash",
        initial_collateral: 50_000,
        complete_sets: 1_000,
    },
    WalletDefinition {
        id: "birch",
        initial_collateral: 50_000,
        complete_sets: 700,
    },
    WalletDefinition {
        id: "cobalt",
        initial_collateral: 50_000,
        complete_sets: 400,
    },
    WalletDefinition {
        id: "dahlia",
        initial_collateral: 50_000,
        complete_sets: 250,
    },
];

const FLAGSHIP_TRADES: &[TradeDefinition] = &[
    TradeDefinition {
        id: "flagship-direct-ash-birch-o0-partial",
        seller: "ash",
        buyer: "birch",
        outcome: 0,
        fill: 400,
        remaining_before: 800,
        execution_price: 250,
    },
    TradeDefinition {
        id: "flagship-direct-birch-cobalt-o1-full",
        seller: "birch",
        buyer: "cobalt",
        outcome: 1,
        fill: 700,
        remaining_before: 700,
        execution_price: 500,
    },
    TradeDefinition {
        id: "flagship-direct-cobalt-dahlia-o2-full",
        seller: "cobalt",
        buyer: "dahlia",
        outcome: 2,
        fill: 400,
        remaining_before: 400,
        execution_price: 750,
    },
    TradeDefinition {
        id: "flagship-direct-dahlia-ash-o3-partial",
        seller: "dahlia",
        buyer: "ash",
        outcome: 3,
        fill: 100,
        remaining_before: 200,
        execution_price: 900,
    },
];

const GRADUATION_WALLETS: &[WalletDefinition] = &[
    WalletDefinition {
        id: "gaia",
        initial_collateral: 60_000,
        complete_sets: 600,
    },
    WalletDefinition {
        id: "helios",
        initial_collateral: 60_000,
        complete_sets: 900,
    },
    WalletDefinition {
        id: "iris",
        initial_collateral: 60_000,
        complete_sets: 500,
    },
    WalletDefinition {
        id: "juniper",
        initial_collateral: 60_000,
        complete_sets: 300,
    },
];

const GRADUATION_TRADES: &[TradeDefinition] = &[
    TradeDefinition {
        id: "graduation-direct-gaia-helios-o0-full",
        seller: "gaia",
        buyer: "helios",
        outcome: 0,
        fill: 600,
        remaining_before: 600,
        execution_price: 400,
    },
    TradeDefinition {
        id: "graduation-direct-helios-iris-o1-partial",
        seller: "helios",
        buyer: "iris",
        outcome: 1,
        fill: 300,
        remaining_before: 900,
        execution_price: 600,
    },
    TradeDefinition {
        id: "graduation-direct-iris-juniper-o2-full",
        seller: "iris",
        buyer: "juniper",
        outcome: 2,
        fill: 500,
        remaining_before: 500,
        execution_price: 800,
    },
    TradeDefinition {
        id: "graduation-direct-juniper-gaia-o3-partial",
        seller: "juniper",
        buyer: "gaia",
        outcome: 3,
        fill: 200,
        remaining_before: 300,
        execution_price: 500,
    },
];

const ABANDONED_WALLETS: &[WalletDefinition] = &[
    WalletDefinition {
        id: "kite",
        initial_collateral: 100_000,
        complete_sets: 800,
    },
    WalletDefinition {
        id: "luna",
        initial_collateral: 100_000,
        complete_sets: 500,
    },
    WalletDefinition {
        id: "moss",
        initial_collateral: 100_000,
        complete_sets: 350,
    },
    WalletDefinition {
        id: "nova",
        initial_collateral: 100_000,
        complete_sets: 200,
    },
];

const ABANDONED_TRADES: &[TradeDefinition] = &[
    TradeDefinition {
        id: "abandoned-direct-kite-luna-o0-partial",
        seller: "kite",
        buyer: "luna",
        outcome: 0,
        fill: 300,
        remaining_before: 800,
        execution_price: 200,
    },
    TradeDefinition {
        id: "abandoned-direct-luna-moss-o1-full",
        seller: "luna",
        buyer: "moss",
        outcome: 1,
        fill: 500,
        remaining_before: 500,
        execution_price: 400,
    },
    TradeDefinition {
        id: "abandoned-direct-moss-nova-o2-partial",
        seller: "moss",
        buyer: "nova",
        outcome: 2,
        fill: 150,
        remaining_before: 350,
        execution_price: 600,
    },
    TradeDefinition {
        id: "abandoned-direct-nova-kite-o3-full",
        seller: "nova",
        buyer: "kite",
        outcome: 3,
        fill: 200,
        remaining_before: 200,
        execution_price: 800,
    },
];

pub(crate) const DEFINITIONS: &[ScenarioDefinition] = &[
    ScenarioDefinition {
        id: "flagship-four-outcome",
        filename: "flagship.json",
        title: "Flagship SOL/USD range market",
        description: "Four wallets split complete sets, cross buyer/seller roles through partial and full Direct fills, then resolve categorically. Every positive claim balance is burned, so both winners and zero-payout losers are visible and the projected Market is retirement-eligible.",
        profile: MarketProfileV1::Flagship,
        wallets: FLAGSHIP_WALLETS,
        trades: FLAGSHIP_TRADES,
        resolution: ResolutionDefinition::Categorical { selector: 2 },
    },
    ScenarioDefinition {
        id: "graduation-four-outcome",
        filename: "graduation.json",
        title: "Relayed mainnet graduation market",
        description: "A four-state graduation observation exercises different wallet sizes, both fill completion classes, independent 50-bps floors, categorical redemption, and full projected retirement. The observation transport remains a runtime adapter concern.",
        profile: MarketProfileV1::Graduation,
        wallets: GRADUATION_WALLETS,
        trades: GRADUATION_TRADES,
        resolution: ResolutionDefinition::Categorical { selector: 0 },
    },
    ScenarioDefinition {
        id: "abandoned-graded-failure",
        filename: "abandoned.json",
        title: "Abandoned relayer funded-failure market",
        description: "The relayer deliberately stays silent. The funded failure walk selects a graded exact-complement failure partition [0,2,3,5] at scale ten, producing zero and positive payouts across varied portfolios before projected retirement.",
        profile: MarketProfileV1::Abandoned,
        wallets: ABANDONED_WALLETS,
        trades: ABANDONED_TRADES,
        resolution: ResolutionDefinition::GradedFailure {
            payout_scale: 10,
            payouts: [0, 2, 3, 5],
        },
    },
];
