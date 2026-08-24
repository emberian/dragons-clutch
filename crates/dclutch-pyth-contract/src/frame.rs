//! Exact SDK-free account frames for categorical Pyth resolution.
//!
//! These frames own only ordered role, privilege, and alias rules.  An SVM
//! adapter must separately authenticate every account key, owner, executable
//! program, and bytes.  In particular, it must hash immutable
//! [`ResolutionMaterial`](ResolutionRoleV1::ResolutionMaterial) and
//! [`CapabilityManifest`](ResolutionRoleV1::CapabilityManifest) bytes against
//! the Market identity, and select the raw [`FundingState`](ResolutionRoleV1::FundingState)
//! from that manifest using the material's policy configuration.  No caller
//! account supplies a policy or funding quote.

/// Semantic identity class an SVM adapter must authenticate for one role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionAccountClassV1 {
    /// A signer authorized to cause a provider update and receive success pay.
    ResolverAuthority,
    /// A mutable provider-owned temporary update account.
    ProviderMutableAccount,
    /// Mutable program-owned Market state.
    MarketState,
    /// Mutable program-owned raw capability funding state.
    FundingState,
    /// Immutable program-owned record authenticated by a Market content ID.
    ImmutableProtocolRecord,
    /// A writable system account receiving a fixed payout.
    PayoutAccount,
    /// An executable provider program.
    ProviderProgram,
    /// Immutable provider ProgramData or configuration state.
    ProviderReadonlyState,
    /// Immutable provider message bytes.
    ProviderMessage,
    /// A writable provider treasury account.
    ProviderTreasury,
    /// The executable System Program.
    SystemProgram,
}

/// Semantic name of one ordered categorical-resolution account role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionRoleV1 {
    /// Price resolver; signs and receives the success bounty.
    Resolver,
    /// Mutable Pyth receiver temporary price-update account.
    Update,
    /// Mutable categorical Market state.
    Market,
    /// Mutable raw funding state selected from the immutable manifest.
    FundingState,
    /// Immutable Pyth policy plus feed-semantics materialization.
    ResolutionMaterial,
    /// Immutable capability manifest that selects the funding state.
    CapabilityManifest,
    /// Immutable Fund refund recipient; may receive a closure residual.
    Sponsor,
    /// Executable Pyth receiver program.
    Receiver,
    /// Readonly Pyth receiver ProgramData account.
    ReceiverProgramData,
    /// Readonly Pyth receiver configuration account.
    ReceiverConfig,
    /// Readonly encoded VAA/message account consumed by the receiver.
    EncodedVaa,
    /// Executable Pyth router program.
    Router,
    /// Readonly Pyth router ProgramData account.
    RouterProgramData,
    /// Writable Pyth receiver treasury account.
    Treasury,
    /// Executable System Program.
    SystemProgram,
    /// Permissionless failure bounty recipient.
    BountyRecipient,
}

/// Required role and the minimum/exact executable privileges for one account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionAccountRoleV1 {
    role: ResolutionRoleV1,
    class: ResolutionAccountClassV1,
    signer: bool,
    writable: bool,
    executable: bool,
}

impl ResolutionAccountRoleV1 {
    const fn new(
        role: ResolutionRoleV1,
        class: ResolutionAccountClassV1,
        signer: bool,
        writable: bool,
        executable: bool,
    ) -> Self {
        Self {
            role,
            class,
            signer,
            writable,
            executable,
        }
    }

    /// Return the ordered semantic role.
    pub const fn role(self) -> ResolutionRoleV1 {
        self.role
    }

    /// Return the identity class requiring adapter authentication.
    pub const fn class(self) -> ResolutionAccountClassV1 {
        self.class
    }

    /// Return whether this role requires a signer privilege.
    pub const fn is_signer(self) -> bool {
        self.signer
    }

    /// Return whether this role requires a writable privilege.
    pub const fn is_writable(self) -> bool {
        self.writable
    }

    /// Return whether this role must have exactly this executable privilege.
    pub const fn is_executable(self) -> bool {
        self.executable
    }
}

/// SDK-free observed key and privileges for one ordered account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionAccountPrivilegeV1 {
    /// Opaque address bytes used only for alias validation.
    pub key: [u8; 32],
    /// Whether the runtime presents the account as a signer.
    pub is_signer: bool,
    /// Whether the runtime presents the account as writable.
    pub is_writable: bool,
    /// Whether the runtime presents the account as executable.
    pub is_executable: bool,
}

/// The exact categorical-resolution frame family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionFrameKindV1 {
    /// Price resolution that performs provider CPIs.
    Price,
    /// Permissionless failure resolution without provider CPIs.
    Failure,
}

/// Refusal returned by exact frame validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolutionFrameErrorV1 {
    /// The supplied account list omitted a required role or carried an extra role.
    InvalidAccountCount,
    /// An account omitted a required signer or writable privilege.
    InsufficientPrivilege,
    /// An account carried a signer or writable privilege its role does not admit.
    UnexpectedPrivilege,
    /// An account's executable privilege did not exactly match its role.
    InvalidExecutablePrivilege,
    /// Two roles that must remain distinct were supplied with the same key.
    UnsafeAlias,
}

/// Result alias for categorical-resolution frame validation.
pub type ResolutionFrameResultV1<T> = core::result::Result<T, ResolutionFrameErrorV1>;

/// Borrowed exact frame description for one resolution path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionFrameV1 {
    kind: ResolutionFrameKindV1,
    roles: &'static [ResolutionAccountRoleV1],
}

impl ResolutionFrameV1 {
    /// Return the resolution path named by this exact account frame.
    pub const fn kind(self) -> ResolutionFrameKindV1 {
        self.kind
    }

    /// Return all ordered role requirements.
    pub const fn roles(self) -> &'static [ResolutionAccountRoleV1] {
        self.roles
    }
}

const fn role(
    role: ResolutionRoleV1,
    class: ResolutionAccountClassV1,
    signer: bool,
    writable: bool,
    executable: bool,
) -> ResolutionAccountRoleV1 {
    ResolutionAccountRoleV1::new(role, class, signer, writable, executable)
}

/// Exact ordered price-resolution frame.
///
/// This is the canonical 15-role order: resolver, update, Market, funding
/// state, resolution material, capability manifest, sponsor, receiver,
/// receiver ProgramData, receiver config, encoded VAA, router, router
/// ProgramData, treasury, System Program.
pub const PRICE_RESOLUTION_FRAME_V1: [ResolutionAccountRoleV1; 15] = [
    role(
        ResolutionRoleV1::Resolver,
        ResolutionAccountClassV1::ResolverAuthority,
        true,
        true,
        false,
    ),
    role(
        ResolutionRoleV1::Update,
        ResolutionAccountClassV1::ProviderMutableAccount,
        true,
        true,
        false,
    ),
    role(
        ResolutionRoleV1::Market,
        ResolutionAccountClassV1::MarketState,
        false,
        true,
        false,
    ),
    role(
        ResolutionRoleV1::FundingState,
        ResolutionAccountClassV1::FundingState,
        false,
        true,
        false,
    ),
    role(
        ResolutionRoleV1::ResolutionMaterial,
        ResolutionAccountClassV1::ImmutableProtocolRecord,
        false,
        false,
        false,
    ),
    role(
        ResolutionRoleV1::CapabilityManifest,
        ResolutionAccountClassV1::ImmutableProtocolRecord,
        false,
        false,
        false,
    ),
    role(
        ResolutionRoleV1::Sponsor,
        ResolutionAccountClassV1::PayoutAccount,
        false,
        true,
        false,
    ),
    role(
        ResolutionRoleV1::Receiver,
        ResolutionAccountClassV1::ProviderProgram,
        false,
        false,
        true,
    ),
    role(
        ResolutionRoleV1::ReceiverProgramData,
        ResolutionAccountClassV1::ProviderReadonlyState,
        false,
        false,
        false,
    ),
    role(
        ResolutionRoleV1::ReceiverConfig,
        ResolutionAccountClassV1::ProviderReadonlyState,
        false,
        false,
        false,
    ),
    role(
        ResolutionRoleV1::EncodedVaa,
        ResolutionAccountClassV1::ProviderMessage,
        false,
        false,
        false,
    ),
    role(
        ResolutionRoleV1::Router,
        ResolutionAccountClassV1::ProviderProgram,
        false,
        false,
        true,
    ),
    role(
        ResolutionRoleV1::RouterProgramData,
        ResolutionAccountClassV1::ProviderReadonlyState,
        false,
        false,
        false,
    ),
    role(
        ResolutionRoleV1::Treasury,
        ResolutionAccountClassV1::ProviderTreasury,
        false,
        true,
        false,
    ),
    role(
        ResolutionRoleV1::SystemProgram,
        ResolutionAccountClassV1::SystemProgram,
        false,
        false,
        true,
    ),
];

/// Exact ordered permissionless failure-resolution frame.
pub const FAILURE_RESOLUTION_FRAME_V1: [ResolutionAccountRoleV1; 6] = [
    role(
        ResolutionRoleV1::BountyRecipient,
        ResolutionAccountClassV1::PayoutAccount,
        false,
        true,
        false,
    ),
    role(
        ResolutionRoleV1::Market,
        ResolutionAccountClassV1::MarketState,
        false,
        true,
        false,
    ),
    role(
        ResolutionRoleV1::FundingState,
        ResolutionAccountClassV1::FundingState,
        false,
        true,
        false,
    ),
    role(
        ResolutionRoleV1::ResolutionMaterial,
        ResolutionAccountClassV1::ImmutableProtocolRecord,
        false,
        false,
        false,
    ),
    role(
        ResolutionRoleV1::CapabilityManifest,
        ResolutionAccountClassV1::ImmutableProtocolRecord,
        false,
        false,
        false,
    ),
    role(
        ResolutionRoleV1::Sponsor,
        ResolutionAccountClassV1::PayoutAccount,
        false,
        true,
        false,
    ),
];

/// Return the exact ordered frame for one categorical-resolution path.
pub const fn resolution_frame_v1(kind: ResolutionFrameKindV1) -> ResolutionFrameV1 {
    let roles: &'static [ResolutionAccountRoleV1] = match kind {
        ResolutionFrameKindV1::Price => &PRICE_RESOLUTION_FRAME_V1,
        ResolutionFrameKindV1::Failure => &FAILURE_RESOLUTION_FRAME_V1,
    };
    ResolutionFrameV1 { kind, roles }
}

/// Validate one exact price-resolution account list.
///
/// The resolver and sponsor are the only allowed key alias. Payout accounts
/// may independently be transaction signers or fee payers without becoming
/// protocol authorities; all writable and executable privileges remain exact.
pub fn validate_price_resolution_frame_v1(
    accounts: &[ResolutionAccountPrivilegeV1],
) -> ResolutionFrameResultV1<()> {
    validate_frame(ResolutionFrameKindV1::Price, accounts)
}

/// Validate one exact permissionless failure-resolution account list.
///
/// The bounty recipient and sponsor are the only allowed key alias.  All
/// Market, FundingState, immutable record, provider, and System roles remain
/// distinct from each other and from either payout role.
pub fn validate_failure_resolution_frame_v1(
    accounts: &[ResolutionAccountPrivilegeV1],
) -> ResolutionFrameResultV1<()> {
    validate_frame(ResolutionFrameKindV1::Failure, accounts)
}

fn validate_frame(
    kind: ResolutionFrameKindV1,
    accounts: &[ResolutionAccountPrivilegeV1],
) -> ResolutionFrameResultV1<()> {
    let frame = resolution_frame_v1(kind);
    if accounts.len() != frame.roles().len() {
        return Err(ResolutionFrameErrorV1::InvalidAccountCount);
    }
    for (actual, required) in accounts.iter().zip(frame.roles()) {
        if (required.is_signer() && !actual.is_signer)
            || (required.is_writable() && !actual.is_writable)
        {
            return Err(ResolutionFrameErrorV1::InsufficientPrivilege);
        }
        if actual.is_writable != required.is_writable() {
            return Err(ResolutionFrameErrorV1::UnexpectedPrivilege);
        }
        if actual.is_executable != required.is_executable() {
            return Err(ResolutionFrameErrorV1::InvalidExecutablePrivilege);
        }
    }
    validate_signer_privileges(accounts, frame.roles())?;
    validate_aliases(kind, accounts)
}

fn validate_signer_privileges(
    accounts: &[ResolutionAccountPrivilegeV1],
    roles: &[ResolutionAccountRoleV1],
) -> ResolutionFrameResultV1<()> {
    for (actual, required) in accounts.iter().zip(roles) {
        if actual.is_signer
            && !required.is_signer()
            && required.class() != ResolutionAccountClassV1::PayoutAccount
        {
            return Err(ResolutionFrameErrorV1::UnexpectedPrivilege);
        }
    }
    Ok(())
}

fn validate_aliases(
    kind: ResolutionFrameKindV1,
    accounts: &[ResolutionAccountPrivilegeV1],
) -> ResolutionFrameResultV1<()> {
    let mut left = 0usize;
    while left < accounts.len() {
        let mut right = left
            .checked_add(1)
            .ok_or(ResolutionFrameErrorV1::UnsafeAlias)?;
        while right < accounts.len() {
            let left_account = accounts
                .get(left)
                .ok_or(ResolutionFrameErrorV1::InvalidAccountCount)?;
            let right_account = accounts
                .get(right)
                .ok_or(ResolutionFrameErrorV1::InvalidAccountCount)?;
            if left_account.key == right_account.key && !safe_payout_alias(kind, left, right) {
                return Err(ResolutionFrameErrorV1::UnsafeAlias);
            }
            right = right
                .checked_add(1)
                .ok_or(ResolutionFrameErrorV1::UnsafeAlias)?;
        }
        left = left
            .checked_add(1)
            .ok_or(ResolutionFrameErrorV1::UnsafeAlias)?;
    }
    Ok(())
}

const fn safe_payout_alias(kind: ResolutionFrameKindV1, left: usize, right: usize) -> bool {
    match kind {
        ResolutionFrameKindV1::Price => left == 0 && right == 6,
        ResolutionFrameKindV1::Failure => left == 0 && right == 5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn accounts(kind: ResolutionFrameKindV1) -> [ResolutionAccountPrivilegeV1; 15] {
        let mut result = [ResolutionAccountPrivilegeV1 {
            key: [0; 32],
            is_signer: false,
            is_writable: false,
            is_executable: false,
        }; 15];
        for (index, required) in resolution_frame_v1(kind).roles().iter().enumerate() {
            let key = u8::try_from(index)
                .expect("bounded test frame index")
                .checked_add(1)
                .expect("bounded test key");
            let account = result
                .get_mut(index)
                .expect("exact frame fits test backing array");
            *account = ResolutionAccountPrivilegeV1 {
                key: [key; 32],
                is_signer: required.is_signer(),
                is_writable: required.is_writable(),
                is_executable: required.is_executable(),
            };
        }
        result
    }

    #[test]
    fn exact_orders_classes_and_privileges_validate() {
        let price = accounts(ResolutionFrameKindV1::Price);
        let failure = accounts(ResolutionFrameKindV1::Failure);
        assert_eq!(PRICE_RESOLUTION_FRAME_V1.len(), 15);
        assert_eq!(FAILURE_RESOLUTION_FRAME_V1.len(), 6);
        assert_eq!(
            PRICE_RESOLUTION_FRAME_V1.get(4).map(|role| role.role()),
            Some(ResolutionRoleV1::ResolutionMaterial)
        );
        assert_eq!(
            PRICE_RESOLUTION_FRAME_V1.get(5).map(|role| role.class()),
            Some(ResolutionAccountClassV1::ImmutableProtocolRecord)
        );
        assert_eq!(
            PRICE_RESOLUTION_FRAME_V1.get(6).map(|role| role.role()),
            Some(ResolutionRoleV1::Sponsor)
        );
        assert_eq!(
            FAILURE_RESOLUTION_FRAME_V1.get(5).map(|role| role.role()),
            Some(ResolutionRoleV1::Sponsor)
        );
        assert_eq!(validate_price_resolution_frame_v1(&price), Ok(()));
        assert_eq!(validate_failure_resolution_frame_v1(&failure[..6]), Ok(()));
    }

    #[test]
    fn missing_extra_and_required_privilege_refuse() {
        let price = accounts(ResolutionFrameKindV1::Price);
        assert_eq!(
            validate_price_resolution_frame_v1(&price[..14]),
            Err(ResolutionFrameErrorV1::InvalidAccountCount)
        );
        let extra = price;
        let extra_slice = extra
            .get(..)
            .expect("fixed test frame supports full long-form input");
        assert_eq!(
            validate_failure_resolution_frame_v1(extra_slice),
            Err(ResolutionFrameErrorV1::InvalidAccountCount)
        );

        let mut missing_signer = price;
        let resolver = missing_signer
            .get_mut(0)
            .expect("resolver exists in exact price frame");
        resolver.is_signer = false;
        assert_eq!(
            validate_price_resolution_frame_v1(&missing_signer),
            Err(ResolutionFrameErrorV1::InsufficientPrivilege)
        );

        let mut unexpected_writable = price;
        let material = unexpected_writable
            .get_mut(4)
            .expect("material exists in exact price frame");
        material.is_writable = true;
        assert_eq!(
            validate_price_resolution_frame_v1(&unexpected_writable),
            Err(ResolutionFrameErrorV1::UnexpectedPrivilege),
            "immutable protocol records are exactly readonly"
        );
        let mut missing_writable = price;
        let market = missing_writable
            .get_mut(2)
            .expect("Market exists in exact price frame");
        market.is_writable = false;
        assert_eq!(
            validate_price_resolution_frame_v1(&missing_writable),
            Err(ResolutionFrameErrorV1::InsufficientPrivilege)
        );

        for index in [2, 4, 7] {
            let mut unexpected_signer = price;
            let account = unexpected_signer
                .get_mut(index)
                .expect("state, immutable, and provider indices exist");
            account.is_signer = true;
            assert_eq!(
                validate_price_resolution_frame_v1(&unexpected_signer),
                Err(ResolutionFrameErrorV1::UnexpectedPrivilege)
            );
        }
    }

    #[test]
    fn executable_privileges_are_exact() {
        let price = accounts(ResolutionFrameKindV1::Price);
        for index in 0..PRICE_RESOLUTION_FRAME_V1.len() {
            let mut changed = price;
            let account = changed
                .get_mut(index)
                .expect("exact price index is in backing array");
            account.is_executable = !account.is_executable;
            assert_eq!(
                validate_price_resolution_frame_v1(&changed),
                Err(ResolutionFrameErrorV1::InvalidExecutablePrivilege)
            );
        }
    }

    #[test]
    fn payout_aliases_are_admitted_but_owned_or_provider_aliases_refuse() {
        let mut price = accounts(ResolutionFrameKindV1::Price);
        let resolver_key = price.first().expect("resolver exists").key;
        let sponsor = price.get_mut(6).expect("sponsor exists");
        sponsor.key = resolver_key;
        sponsor.is_signer = true;
        assert_eq!(validate_price_resolution_frame_v1(&price), Ok(()));

        let mut market_alias = accounts(ResolutionFrameKindV1::Price);
        let market_key = market_alias.get(2).expect("Market exists").key;
        let sponsor = market_alias.get_mut(6).expect("sponsor exists");
        sponsor.key = market_key;
        assert_eq!(
            validate_price_resolution_frame_v1(&market_alias),
            Err(ResolutionFrameErrorV1::UnsafeAlias)
        );

        let mut provider_alias = accounts(ResolutionFrameKindV1::Price);
        let update_key = provider_alias.get(1).expect("update exists").key;
        let resolver = provider_alias.get_mut(0).expect("resolver exists");
        resolver.key = update_key;
        assert_eq!(
            validate_price_resolution_frame_v1(&provider_alias),
            Err(ResolutionFrameErrorV1::UnsafeAlias)
        );

        let mut independent_price_payout = accounts(ResolutionFrameKindV1::Price);
        let sponsor = independent_price_payout.get_mut(6).expect("sponsor exists");
        sponsor.is_signer = true;
        assert_eq!(
            validate_price_resolution_frame_v1(&independent_price_payout),
            Ok(()),
            "a separately signed payout recipient gains no resolver authority"
        );

        let mut failure = accounts(ResolutionFrameKindV1::Failure);
        let bounty_key = failure.first().expect("bounty recipient exists").key;
        let sponsor = failure.get_mut(5).expect("sponsor exists");
        sponsor.key = bounty_key;
        assert_eq!(validate_failure_resolution_frame_v1(&failure[..6]), Ok(()));
        let fund_key = failure.get(2).expect("FundingState exists").key;
        let bounty = failure.get_mut(0).expect("bounty recipient exists");
        bounty.key = fund_key;
        assert_eq!(
            validate_failure_resolution_frame_v1(&failure[..6]),
            Err(ResolutionFrameErrorV1::UnsafeAlias)
        );

        let mut independent_failure_payouts = accounts(ResolutionFrameKindV1::Failure);
        let bounty = independent_failure_payouts
            .get_mut(0)
            .expect("bounty recipient exists");
        bounty.is_signer = true;
        let sponsor = independent_failure_payouts
            .get_mut(5)
            .expect("sponsor exists");
        sponsor.is_signer = true;
        assert_eq!(
            validate_failure_resolution_frame_v1(&independent_failure_payouts[..6]),
            Ok(()),
            "permissionless failure remains valid when either payout role pays fees"
        );
    }
}
