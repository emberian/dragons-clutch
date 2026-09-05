//! Immutable exact-transfer profiles over authenticated token-program state.

use crate::token_svm::{
    Address, COption, Error, LEGACY_TOKEN_PROGRAM_ID, Mint, Result, TOKEN_2022_PROGRAM_ID,
    TokenAccount, TokenProgram, state::AccountState,
};

/// Immutable token-program semantics admitted by an exact-transfer Realm.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactTransferProfileV1 {
    /// Legacy SPL Token with exact 82-byte Mints and 165-byte Accounts.
    LegacyExactTransferV1,
    /// Token-2022 with exact base widths and therefore no extension storage.
    Token2022ZeroExtensionExactTransferV1,
    /// Token-2022 admitting the ATA program's `ImmutableOwner` on participants.
    ///
    /// TRANSFER PARTICIPANTS ONLY. The Associated Token Account program writes
    /// `ImmutableOwner` into every account it creates under Token-2022, so the
    /// destination a wallet actually owns is 170 bytes and the zero-extension
    /// profile refuses it -- measured on cohort-13, on chain, where the
    /// founder's ATA had to be replaced with a 165-byte account created by
    /// hand. This profile admits exactly that one extension at exactly that
    /// one width, and [`Self::check_custody_account`] keeps the base width
    /// under every profile because a protocol vault is created by this
    /// protocol and never by the ATA program.
    Token2022ImmutableOwnerTransferV1,
}

impl ExactTransferProfileV1 {
    /// Return the sole program address admitted by this profile.
    pub const fn program_id(self) -> Address {
        match self {
            Self::LegacyExactTransferV1 => LEGACY_TOKEN_PROGRAM_ID,
            Self::Token2022ZeroExtensionExactTransferV1
            | Self::Token2022ImmutableOwnerTransferV1 => TOKEN_2022_PROGRAM_ID,
        }
    }

    /// Parse one transfer participant at every width this profile admits.
    ///
    /// The whole difference between the second and third adapter releases is
    /// which parser this returns, so it is one function and not a condition
    /// spelled at each call site.
    fn parse_participant(self, account_data: &[u8]) -> Result<TokenAccount> {
        match self {
            Self::LegacyExactTransferV1 | Self::Token2022ZeroExtensionExactTransferV1 => {
                TokenAccount::parse(account_data)
            }
            Self::Token2022ImmutableOwnerTransferV1 => {
                TokenAccount::parse_base_or_immutable_owner(account_data)
            }
        }
    }

    /// Check both general recognition and exact profile identity.
    pub fn check_program(self, program_id: Address) -> Result<TokenProgram> {
        let program = TokenProgram::parse(program_id)?;
        if program_id != self.program_id() {
            return Err(Error::ProfileProgramMismatch);
        }
        Ok(program)
    }

    /// Parse and require one initialized exact-width Mint.
    ///
    /// Mint and freeze authorities remain returned facts. They are not silently
    /// forbidden here because the immutable Realm has separate explicit issuer
    /// authority policies.
    pub fn check_mint(self, program_id: Address, mint_data: &[u8]) -> Result<Mint> {
        self.check_program(program_id)?;
        let mint = Mint::parse(mint_data)?;
        if !mint.is_initialized {
            return Err(Error::MintUninitialized);
        }
        Ok(mint)
    }

    /// Parse one account participating in exact token-only transfer semantics.
    ///
    /// This refuses uninitialized, frozen, and wrapped-native accounts.
    /// Delegate and close-authority facts are retained because they do not
    /// change a `TransferChecked` amount. Use [`Self::check_custody_account`]
    /// for a protocol vault that must exclude those authorities.
    pub fn check_transfer_account(
        self,
        program_id: Address,
        account_data: &[u8],
    ) -> Result<TokenAccount> {
        self.check_program(program_id)?;
        check_transferable(self.parse_participant(account_data)?)
    }

    /// Require the stronger account policy for program custody.
    ///
    /// A custody account must use the expected Mint and owner and retain no
    /// native reserve, delegate allowance, or separate close authority.
    ///
    /// AND IT IS EXACTLY THE BASE WIDTH UNDER EVERY PROFILE, including the one
    /// that admits `ImmutableOwner` on transfer participants. A protocol vault
    /// is allocated by this protocol at [`crate::token_svm::ACCOUNT_BYTES`] and
    /// initialized with `InitializeAccount3`; it is never produced by the
    /// Associated Token Account program, so nothing that needed the third
    /// adapter release reaches here. Widening this too would have bought
    /// nothing and would have admitted an extension suffix on an account whose
    /// rent, poststate projection and closure this protocol owns.
    pub fn check_custody_account(
        self,
        program_id: Address,
        account_data: &[u8],
        expected_mint: Address,
        expected_owner: Address,
    ) -> Result<TokenAccount> {
        self.check_program(program_id)?;
        let account = check_transferable(TokenAccount::parse(account_data)?)?;
        if account.mint != expected_mint {
            return Err(Error::MintMismatch);
        }
        if account.owner != expected_owner {
            return Err(Error::AuthorityMismatch);
        }
        if !account.delegate.is_none() || account.delegated_amount != 0 {
            return Err(Error::DelegatePresent);
        }
        if !account.close_authority.is_none() {
            return Err(Error::CloseAuthorityPresent);
        }
        Ok(account)
    }

    /// Validate all byte-state preconditions for one exact checked transfer.
    ///
    /// The destination may have its own delegate or close authority: those
    /// facts do not affect the amount credited by this transfer. Custody roles
    /// are checked separately with [`Self::check_custody_account`].
    pub fn check_transfer<'a>(self, input: ExactTransferInput<'a>) -> Result<ExactTransferFacts> {
        let mint = self.check_mint(input.program_id, input.mint_data)?;
        let source = self.check_transfer_account(input.program_id, input.source_data)?;
        let destination = self.check_transfer_account(input.program_id, input.destination_data)?;
        if source.mint != input.mint_address || destination.mint != input.mint_address {
            return Err(Error::MintMismatch);
        }
        if mint.decimals != input.decimals {
            return Err(Error::DecimalsMismatch);
        }
        if source.amount < input.amount {
            return Err(Error::InsufficientFunds);
        }
        destination
            .amount
            .checked_add(input.amount)
            .ok_or(Error::ArithmeticOverflow)?;

        let authority_role = if source.owner == input.authority {
            AuthorityRole::Owner
        } else {
            match source.delegate {
                COption::Some(delegate) if delegate == input.authority => {
                    if source.delegated_amount < input.amount {
                        return Err(Error::InsufficientDelegateAllowance);
                    }
                    AuthorityRole::Delegate
                }
                COption::None | COption::Some(_) => return Err(Error::AuthorityMismatch),
            }
        };
        Ok(ExactTransferFacts {
            mint,
            source,
            destination,
            authority_role,
        })
    }
}

/// Borrowed state and instruction facts required for an exact transfer check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactTransferInput<'a> {
    /// Token program address observed by the adapter.
    pub program_id: Address,
    /// Mint account address named by both token Accounts and the instruction.
    pub mint_address: Address,
    /// Exact Mint account data.
    pub mint_data: &'a [u8],
    /// Exact source Account data.
    pub source_data: &'a [u8],
    /// Exact destination Account data.
    pub destination_data: &'a [u8],
    /// Owner or delegate authority named by the instruction.
    pub authority: Address,
    /// Exact raw token amount named by the instruction.
    pub amount: u64,
    /// Mint decimals named by the checked instruction.
    pub decimals: u8,
}

/// Authority path authenticated for a checked transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthorityRole {
    /// The source Account owner authorized the transfer.
    Owner,
    /// The source Account delegate authorized the transfer within allowance.
    Delegate,
}

/// Decoded facts after all exact-transfer profile checks succeed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactTransferFacts {
    mint: Mint,
    source: TokenAccount,
    destination: TokenAccount,
    authority_role: AuthorityRole,
}

impl ExactTransferFacts {
    /// Return the authenticated Mint base state.
    pub const fn mint(&self) -> Mint {
        self.mint
    }

    /// Return the authenticated source base state.
    pub const fn source(&self) -> TokenAccount {
        self.source
    }

    /// Return the authenticated destination base state.
    pub const fn destination(&self) -> TokenAccount {
        self.destination
    }

    /// Return whether owner or delegate authority was selected.
    pub const fn authority_role(&self) -> AuthorityRole {
        self.authority_role
    }
}

fn check_transferable(account: TokenAccount) -> Result<TokenAccount> {
    match account.state {
        AccountState::Uninitialized => return Err(Error::AccountUninitialized),
        AccountState::Initialized => {}
        AccountState::Frozen => return Err(Error::AccountFrozen),
    }
    if !account.native_reserve.is_none() {
        return Err(Error::NativeAccount);
    }
    Ok(account)
}

#[cfg(test)]
mod tests {
    use crate::token_svm::{
        ACCOUNT_BYTES, MINT_BYTES,
        state::fixtures::{DESTINATION_OWNER_KEY, MINT_KEY, OWNER_KEY, account, mint, put},
    };

    use super::*;

    const DELEGATE_OFFSET: usize = 72;
    const STATE_OFFSET: usize = 108;
    const NATIVE_OFFSET: usize = 109;
    const DELEGATED_AMOUNT_OFFSET: usize = 121;
    const CLOSE_AUTHORITY_OFFSET: usize = 129;

    fn transfer_input<'a>(
        program_id: Address,
        mint_data: &'a [u8],
        source_data: &'a [u8],
        destination_data: &'a [u8],
        authority: Address,
        amount: u64,
    ) -> ExactTransferInput<'a> {
        ExactTransferInput {
            program_id,
            mint_address: MINT_KEY,
            mint_data,
            source_data,
            destination_data,
            authority,
            amount,
            decimals: 6,
        }
    }

    #[test]
    fn both_program_profiles_accept_only_their_exact_program() {
        let mint_data = mint();
        let source = account(OWNER_KEY, 20);
        let destination = account(DESTINATION_OWNER_KEY, 3);
        for (profile, program_id) in [
            (
                ExactTransferProfileV1::LegacyExactTransferV1,
                LEGACY_TOKEN_PROGRAM_ID,
            ),
            (
                ExactTransferProfileV1::Token2022ZeroExtensionExactTransferV1,
                TOKEN_2022_PROGRAM_ID,
            ),
        ] {
            let checked = profile.check_transfer(transfer_input(
                program_id,
                &mint_data,
                &source,
                &destination,
                OWNER_KEY,
                7,
            ));
            assert!(checked.is_ok());
        }
        assert_eq!(
            ExactTransferProfileV1::LegacyExactTransferV1.check_program(TOKEN_2022_PROGRAM_ID),
            Err(Error::ProfileProgramMismatch)
        );
        assert_eq!(
            ExactTransferProfileV1::Token2022ZeroExtensionExactTransferV1
                .check_program(LEGACY_TOKEN_PROGRAM_ID),
            Err(Error::ProfileProgramMismatch)
        );
        assert_eq!(
            ExactTransferProfileV1::LegacyExactTransferV1.check_program([44; 32]),
            Err(Error::UnsupportedProgram)
        );
    }

    /// THE THIRD RELEASE'S WHOLE BEHAVIOURAL DIFFERENCE, at the profile layer.
    ///
    /// Same bytes, same call, two profiles: the released one refuses the
    /// account every wallet actually has, and the one cohort-14 founds under
    /// admits it. Both refusals and both admissions are asserted here so the
    /// difference is a measurement rather than a description.
    #[test]
    fn only_the_immutable_owner_profile_admits_the_ata_shape() {
        let mut ata = account(OWNER_KEY, 20).to_vec();
        ata.extend_from_slice(&crate::token_svm::IMMUTABLE_OWNER_ACCOUNT_SUFFIX);
        assert_eq!(ata.len(), crate::token_svm::IMMUTABLE_OWNER_ACCOUNT_BYTES);

        assert_eq!(
            ExactTransferProfileV1::Token2022ZeroExtensionExactTransferV1
                .check_transfer_account(TOKEN_2022_PROGRAM_ID, &ata),
            Err(Error::InvalidLength),
            "the released profile refuses it, which is why cohort-13 could not be paid into one",
        );
        assert_eq!(
            ExactTransferProfileV1::Token2022ImmutableOwnerTransferV1
                .check_transfer_account(TOKEN_2022_PROGRAM_ID, &ata)
                .expect("the ATA shape is admitted")
                .amount,
            20,
        );
        // The base account is admitted by BOTH, so founding cohort-14 under the
        // third release takes nothing away from an ordinary 165-byte account.
        let base = account(OWNER_KEY, 20);
        for profile in [
            ExactTransferProfileV1::Token2022ZeroExtensionExactTransferV1,
            ExactTransferProfileV1::Token2022ImmutableOwnerTransferV1,
        ] {
            assert!(
                profile
                    .check_transfer_account(TOKEN_2022_PROGRAM_ID, &base)
                    .is_ok()
            );
        }
        // And it is still Token-2022 only.
        assert_eq!(
            ExactTransferProfileV1::Token2022ImmutableOwnerTransferV1
                .check_program(LEGACY_TOKEN_PROGRAM_ID),
            Err(Error::ProfileProgramMismatch)
        );
    }

    /// ONE EXTENSION, NOT A CLASS OF THEM.
    ///
    /// Six other extension types staged at the identical 170-byte width, where
    /// only the type discriminant can tell them apart. Each changes what a
    /// transfer MEANS; `ImmutableOwner` is the only one that strengthens the
    /// owner check the payout already makes, so it is the only one admitted.
    #[test]
    fn the_immutable_owner_profile_refuses_six_other_extensions_at_the_same_width() {
        let profile = ExactTransferProfileV1::Token2022ImmutableOwnerTransferV1;
        for extension_type in [
            1_u16, // TransferFeeConfig
            5,     // NonTransferable
            6,     // ImmutableOwner is 7; 6 is its neighbour and is not it
            8,     // MemoTransfer
            11,    // CpiGuard
            14,    // TransferHookAccount
        ] {
            let mut hostile = account(OWNER_KEY, 20).to_vec();
            hostile.push(2);
            hostile.extend_from_slice(&extension_type.to_le_bytes());
            hostile.extend_from_slice(&0_u16.to_le_bytes());
            assert_eq!(hostile.len(), crate::token_svm::IMMUTABLE_OWNER_ACCOUNT_BYTES);
            assert_eq!(
                profile.check_transfer_account(TOKEN_2022_PROGRAM_ID, &hostile),
                Err(Error::InvalidExtensionLayout),
                "extension {extension_type} must not be admitted by the third release",
            );
        }
    }

    /// PROTOCOL CUSTODY IS BASE WIDTH UNDER EVERY PROFILE.
    ///
    /// The third release widens transfer PARTICIPANTS. A Hoard vault, a Direct
    /// token PDA and every other account this protocol allocates is created at
    /// `ACCOUNT_BYTES` by this protocol, so admitting a suffix there would buy
    /// nothing and would let an account whose rent and poststate this protocol
    /// projects carry storage it did not write.
    #[test]
    fn protocol_custody_refuses_the_suffix_under_every_profile() {
        let mut ata = account(OWNER_KEY, 20).to_vec();
        ata.extend_from_slice(&crate::token_svm::IMMUTABLE_OWNER_ACCOUNT_SUFFIX);
        for profile in [
            ExactTransferProfileV1::Token2022ZeroExtensionExactTransferV1,
            ExactTransferProfileV1::Token2022ImmutableOwnerTransferV1,
        ] {
            assert_eq!(
                profile.check_custody_account(TOKEN_2022_PROGRAM_ID, &ata, MINT_KEY, OWNER_KEY),
                Err(Error::InvalidLength),
                "a protocol vault is never an associated token account",
            );
            // The control: the same account at the base width commits.
            assert!(
                profile
                    .check_custody_account(
                        TOKEN_2022_PROGRAM_ID,
                        &account(OWNER_KEY, 20),
                        MINT_KEY,
                        OWNER_KEY,
                    )
                    .is_ok()
            );
        }
    }

    /// THE PAYOUT ITSELF: a Hoard vault paying an ATA-shaped destination.
    ///
    /// This is the shape `build_wallet_terminal_payout_v3` builds -- a
    /// 165-byte protocol source and a 170-byte wallet destination -- checked
    /// through the one function Custody calls. Under the released profile it
    /// refuses; under the third release it produces facts.
    #[test]
    fn the_terminal_payout_shape_checks_only_under_the_third_release() {
        let mint_data = mint();
        let source = account(OWNER_KEY, 500_000_000);
        let mut destination = account(DESTINATION_OWNER_KEY, 0).to_vec();
        destination.extend_from_slice(&crate::token_svm::IMMUTABLE_OWNER_ACCOUNT_SUFFIX);
        let input = |profile: ExactTransferProfileV1| {
            profile.check_transfer(transfer_input(
                TOKEN_2022_PROGRAM_ID,
                &mint_data,
                &source,
                &destination,
                OWNER_KEY,
                500_000_000,
            ))
        };
        assert_eq!(
            input(ExactTransferProfileV1::Token2022ZeroExtensionExactTransferV1),
            Err(Error::InvalidLength),
        );
        let facts = input(ExactTransferProfileV1::Token2022ImmutableOwnerTransferV1)
            .expect("the conventional destination checks under the third release");
        assert_eq!(facts.authority_role(), AuthorityRole::Owner);
        assert_eq!(facts.destination().owner, DESTINATION_OWNER_KEY);
        assert_eq!(facts.source().amount, 500_000_000);
    }

    #[test]
    fn token_2022_profile_rejects_every_extension_representation_by_length() {
        let profile = ExactTransferProfileV1::Token2022ZeroExtensionExactTransferV1;
        let mut extended_mint = [0; MINT_BYTES + 84];
        let base_mint = mint();
        put(&mut extended_mint, 0, &base_mint);
        let mut extended_account = [0; ACCOUNT_BYTES + 1];
        let base_account = account(OWNER_KEY, 20);
        put(&mut extended_account, 0, &base_account);
        assert_eq!(
            profile.check_mint(TOKEN_2022_PROGRAM_ID, &extended_mint),
            Err(Error::InvalidLength)
        );
        assert_eq!(
            profile.check_transfer_account(TOKEN_2022_PROGRAM_ID, &extended_account),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn state_and_native_accounts_are_hostile_to_exact_transfer() {
        let profile = ExactTransferProfileV1::LegacyExactTransferV1;
        for (state, error) in [(0, Error::AccountUninitialized), (2, Error::AccountFrozen)] {
            let mut hostile = account(OWNER_KEY, 20);
            put(&mut hostile, STATE_OFFSET, &[state]);
            assert_eq!(
                profile.check_transfer_account(LEGACY_TOKEN_PROGRAM_ID, &hostile),
                Err(error)
            );
        }
        let mut native = account(OWNER_KEY, 20);
        put(&mut native, NATIVE_OFFSET, &[1, 0, 0, 0]);
        put(&mut native, NATIVE_OFFSET + 4, &10u64.to_le_bytes());
        assert_eq!(
            profile.check_transfer_account(LEGACY_TOKEN_PROGRAM_ID, &native),
            Err(Error::NativeAccount)
        );
    }

    #[test]
    fn delegate_is_supported_for_transfer_but_refused_for_custody() {
        let profile = ExactTransferProfileV1::LegacyExactTransferV1;
        let mint_data = mint();
        let mut delegated = account(OWNER_KEY, 20);
        put(&mut delegated, DELEGATE_OFFSET, &[1, 0, 0, 0]);
        put(&mut delegated, DELEGATE_OFFSET + 4, &[5; 32]);
        put(&mut delegated, DELEGATED_AMOUNT_OFFSET, &8u64.to_le_bytes());
        let destination = account(DESTINATION_OWNER_KEY, 3);
        let checked = profile.check_transfer(transfer_input(
            LEGACY_TOKEN_PROGRAM_ID,
            &mint_data,
            &delegated,
            &destination,
            [5; 32],
            7,
        ));
        assert!(checked.is_ok());
        if let Ok(facts) = checked {
            assert_eq!(facts.authority_role(), AuthorityRole::Delegate);
        }
        assert_eq!(
            profile.check_transfer(transfer_input(
                LEGACY_TOKEN_PROGRAM_ID,
                &mint_data,
                &delegated,
                &destination,
                [5; 32],
                9,
            )),
            Err(Error::InsufficientDelegateAllowance)
        );
        assert_eq!(
            profile
                .check_custody_account(LEGACY_TOKEN_PROGRAM_ID, &delegated, MINT_KEY, OWNER_KEY,),
            Err(Error::DelegatePresent)
        );
    }

    #[test]
    fn custody_rejects_stale_allowance_and_close_authority() {
        let profile = ExactTransferProfileV1::LegacyExactTransferV1;
        let mut stale_allowance = account(OWNER_KEY, 20);
        put(
            &mut stale_allowance,
            DELEGATED_AMOUNT_OFFSET,
            &1u64.to_le_bytes(),
        );
        assert_eq!(
            profile.check_custody_account(
                LEGACY_TOKEN_PROGRAM_ID,
                &stale_allowance,
                MINT_KEY,
                OWNER_KEY,
            ),
            Err(Error::DelegatePresent)
        );

        let mut closable = account(OWNER_KEY, 20);
        put(&mut closable, CLOSE_AUTHORITY_OFFSET, &[1, 0, 0, 0]);
        put(&mut closable, CLOSE_AUTHORITY_OFFSET + 4, &[6; 32]);
        assert_eq!(
            profile.check_custody_account(LEGACY_TOKEN_PROGRAM_ID, &closable, MINT_KEY, OWNER_KEY,),
            Err(Error::CloseAuthorityPresent)
        );
    }

    #[test]
    fn cross_state_transfer_checks_refuse_every_semantic_mismatch() {
        let profile = ExactTransferProfileV1::LegacyExactTransferV1;
        let mint_data = mint();
        let source = account(OWNER_KEY, 20);
        let destination = account(DESTINATION_OWNER_KEY, 3);

        let mut wrong_mint = destination;
        put(&mut wrong_mint, 0, &[12; 32]);
        assert_eq!(
            profile.check_transfer(transfer_input(
                LEGACY_TOKEN_PROGRAM_ID,
                &mint_data,
                &source,
                &wrong_mint,
                OWNER_KEY,
                7,
            )),
            Err(Error::MintMismatch)
        );

        let mut wrong_decimals = transfer_input(
            LEGACY_TOKEN_PROGRAM_ID,
            &mint_data,
            &source,
            &destination,
            OWNER_KEY,
            7,
        );
        wrong_decimals.decimals = 5;
        assert_eq!(
            profile.check_transfer(wrong_decimals),
            Err(Error::DecimalsMismatch)
        );
        assert_eq!(
            profile.check_transfer(transfer_input(
                LEGACY_TOKEN_PROGRAM_ID,
                &mint_data,
                &source,
                &destination,
                [99; 32],
                7,
            )),
            Err(Error::AuthorityMismatch)
        );
        assert_eq!(
            profile.check_transfer(transfer_input(
                LEGACY_TOKEN_PROGRAM_ID,
                &mint_data,
                &source,
                &destination,
                OWNER_KEY,
                21,
            )),
            Err(Error::InsufficientFunds)
        );

        let overflowing = account(DESTINATION_OWNER_KEY, u64::MAX);
        assert_eq!(
            profile.check_transfer(transfer_input(
                LEGACY_TOKEN_PROGRAM_ID,
                &mint_data,
                &source,
                &overflowing,
                OWNER_KEY,
                1,
            )),
            Err(Error::ArithmeticOverflow)
        );
    }
}
