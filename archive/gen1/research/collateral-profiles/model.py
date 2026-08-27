# SPDX-License-Identifier: AGPL-3.0-or-later
"""Deterministic, host-only Dragon's Clutch collateral-profile model.

This is an independent policy experiment.  It does not parse Solana accounts,
perform CPI, access RPC, hold keys, sign, or submit transactions.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import IntEnum
from hashlib import sha256
from struct import pack, unpack_from
from typing import Iterable, Optional


PROFILE_MAGIC = b"DCCOLP1\0"
PROFILE_SCHEMA_VERSION = 1
PROFILE_DOMAIN = b"dragons-clutch/collateral-profile/v1\0"
PROFILE_RESERVED_BYTES = 16
PUBKEY_BYTES = 32
U64_MAX = (1 << 64) - 1

LEGACY_TOKEN_PROGRAM_TEXT = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"
TOKEN_2022_PROGRAM_TEXT = "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"
DREGG_MINT_TEXT = "XkeTXo1125vz5H9svJpGiw4JvLbN8VmMu9cmMvspump"


class ModelError(ValueError):
    """A malformed profile or snapshot."""


class CurrencyKind(IntEnum):
    NATIVE_SOL = 0
    SPL_TOKEN = 1


class ExtensionType(IntEnum):
    """Token-2022 extension discriminants at the pinned source revision."""

    UNINITIALIZED = 0
    TRANSFER_FEE_CONFIG = 1
    TRANSFER_FEE_AMOUNT = 2
    MINT_CLOSE_AUTHORITY = 3
    CONFIDENTIAL_TRANSFER_MINT = 4
    CONFIDENTIAL_TRANSFER_ACCOUNT = 5
    DEFAULT_ACCOUNT_STATE = 6
    IMMUTABLE_OWNER = 7
    MEMO_TRANSFER = 8
    NON_TRANSFERABLE = 9
    INTEREST_BEARING_CONFIG = 10
    CPI_GUARD = 11
    PERMANENT_DELEGATE = 12
    NON_TRANSFERABLE_ACCOUNT = 13
    TRANSFER_HOOK = 14
    TRANSFER_HOOK_ACCOUNT = 15
    CONFIDENTIAL_TRANSFER_FEE_CONFIG = 16
    CONFIDENTIAL_TRANSFER_FEE_AMOUNT = 17
    METADATA_POINTER = 18
    TOKEN_METADATA = 19
    GROUP_POINTER = 20
    TOKEN_GROUP = 21
    GROUP_MEMBER_POINTER = 22
    TOKEN_GROUP_MEMBER = 23
    CONFIDENTIAL_MINT_BURN = 24
    SCALED_UI_AMOUNT = 25
    PAUSABLE = 26
    PAUSABLE_ACCOUNT = 27
    PERMISSIONED_BURN = 28


MINT_EXTENSIONS = frozenset(
    {
        ExtensionType.TRANSFER_FEE_CONFIG,
        ExtensionType.MINT_CLOSE_AUTHORITY,
        ExtensionType.CONFIDENTIAL_TRANSFER_MINT,
        ExtensionType.DEFAULT_ACCOUNT_STATE,
        ExtensionType.NON_TRANSFERABLE,
        ExtensionType.INTEREST_BEARING_CONFIG,
        ExtensionType.PERMANENT_DELEGATE,
        ExtensionType.TRANSFER_HOOK,
        ExtensionType.CONFIDENTIAL_TRANSFER_FEE_CONFIG,
        ExtensionType.METADATA_POINTER,
        ExtensionType.TOKEN_METADATA,
        ExtensionType.GROUP_POINTER,
        ExtensionType.TOKEN_GROUP,
        ExtensionType.GROUP_MEMBER_POINTER,
        ExtensionType.TOKEN_GROUP_MEMBER,
        ExtensionType.CONFIDENTIAL_MINT_BURN,
        ExtensionType.SCALED_UI_AMOUNT,
        ExtensionType.PAUSABLE,
        ExtensionType.PERMISSIONED_BURN,
    }
)

ACCOUNT_EXTENSIONS = frozenset(
    {
        ExtensionType.TRANSFER_FEE_AMOUNT,
        ExtensionType.CONFIDENTIAL_TRANSFER_ACCOUNT,
        ExtensionType.IMMUTABLE_OWNER,
        ExtensionType.MEMO_TRANSFER,
        ExtensionType.CPI_GUARD,
        ExtensionType.NON_TRANSFERABLE_ACCOUNT,
        ExtensionType.TRANSFER_HOOK_ACCOUNT,
        ExtensionType.CONFIDENTIAL_TRANSFER_FEE_AMOUNT,
        ExtensionType.PAUSABLE_ACCOUNT,
    }
)

# Conservative V1 support ceiling.  Base Token-2022 mints are supported.  The
# only extension admitted anywhere is ImmutableOwner on token accounts; it does
# not add transfer arithmetic, a mutable external program, opaque balances, or
# an authority able to seize/freeze the Hoard.  Realm profiles can narrow this
# ceiling but cannot expand it.
PROTOCOL_MINT_EXTENSION_CEILING = frozenset()
PROTOCOL_ACCOUNT_EXTENSION_CEILING = frozenset({ExtensionType.IMMUTABLE_OWNER})


EXTENSION_REFUSAL_REASON = {
    ExtensionType.UNINITIALIZED: "padding/unknown semantic state is never admitted",
    ExtensionType.TRANSFER_FEE_CONFIG: "gross transfer does not equal Hoard credit",
    ExtensionType.TRANSFER_FEE_AMOUNT: "withheld balances create a second balance state",
    ExtensionType.MINT_CLOSE_AUTHORITY: "mint can be closed and reinitialized",
    ExtensionType.CONFIDENTIAL_TRANSFER_MINT: "opaque balances are outside transparent V1",
    ExtensionType.CONFIDENTIAL_TRANSFER_ACCOUNT: "opaque balances are outside transparent V1",
    ExtensionType.DEFAULT_ACCOUNT_STATE: "new accounts can default frozen and the policy is mutable",
    ExtensionType.IMMUTABLE_OWNER: "allowed on token accounts only",
    ExtensionType.MEMO_TRANSFER: "adds per-account transfer preconditions not modeled by V1",
    ExtensionType.NON_TRANSFERABLE: "collateral cannot move into and out of a Hoard",
    ExtensionType.INTEREST_BEARING_CONFIG: "display-unit transformations are outside atom-only V1",
    ExtensionType.CPI_GUARD: "can refuse the adapter CPI path",
    ExtensionType.PERMANENT_DELEGATE: "third party can transfer or burn Hoard collateral",
    ExtensionType.NON_TRANSFERABLE_ACCOUNT: "account belongs to non-transferable collateral",
    ExtensionType.TRANSFER_HOOK: "invokes mutable external program and extra-account policy",
    ExtensionType.TRANSFER_HOOK_ACCOUNT: "account belongs to transfer-hook collateral",
    ExtensionType.CONFIDENTIAL_TRANSFER_FEE_CONFIG: "opaque fee state is outside transparent V1",
    ExtensionType.CONFIDENTIAL_TRANSFER_FEE_AMOUNT: "opaque withheld balance is outside transparent V1",
    ExtensionType.METADATA_POINTER: "mutable pointer semantics are unnecessary in minimal V1",
    ExtensionType.TOKEN_METADATA: "variable-length mutable metadata is unnecessary in minimal V1",
    ExtensionType.GROUP_POINTER: "mutable pointer semantics are unnecessary in minimal V1",
    ExtensionType.TOKEN_GROUP: "group semantics are unnecessary in minimal V1",
    ExtensionType.GROUP_MEMBER_POINTER: "mutable pointer semantics are unnecessary in minimal V1",
    ExtensionType.TOKEN_GROUP_MEMBER: "group-member semantics are unnecessary in minimal V1",
    ExtensionType.CONFIDENTIAL_MINT_BURN: "opaque supply changes are outside transparent V1",
    ExtensionType.SCALED_UI_AMOUNT: "mutable UI scaling is outside atom-only V1",
    ExtensionType.PAUSABLE: "authority can pause transfers, minting, and burning",
    ExtensionType.PAUSABLE_ACCOUNT: "account belongs to pausable collateral",
    ExtensionType.PERMISSIONED_BURN: "burn authority changes ordinary fungible-token semantics",
}


def decode_base58_pubkey(text: str) -> bytes:
    """Decode a canonical base58 public key without third-party dependencies."""

    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    if not text:
        raise ModelError("empty base58 public key")
    number = 0
    for char in text:
        try:
            digit = alphabet.index(char)
        except ValueError as exc:
            raise ModelError("invalid base58 public key") from exc
        number = number * 58 + digit
    payload = number.to_bytes((number.bit_length() + 7) // 8, "big") if number else b""
    leading_zeroes = len(text) - len(text.lstrip("1"))
    decoded = b"\0" * leading_zeroes + payload
    if len(decoded) != PUBKEY_BYTES:
        raise ModelError("public key must decode to 32 bytes")
    return decoded


LEGACY_TOKEN_PROGRAM = decode_base58_pubkey(LEGACY_TOKEN_PROGRAM_TEXT)
TOKEN_2022_PROGRAM = decode_base58_pubkey(TOKEN_2022_PROGRAM_TEXT)
DREGG_MINT = decode_base58_pubkey(DREGG_MINT_TEXT)


def _require_pubkey(value: bytes, name: str, *, nonzero: bool = True) -> None:
    if not isinstance(value, bytes) or len(value) != PUBKEY_BYTES:
        raise ModelError(f"{name} must be exactly 32 bytes")
    if nonzero and value == bytes(PUBKEY_BYTES):
        raise ModelError(f"{name} cannot be zero")


@dataclass(frozen=True)
class CurrencyRef:
    """An atom-denominated currency identity in canonical profile bytes."""

    kind: CurrencyKind
    token_program: bytes
    mint: bytes
    decimals: int

    def __post_init__(self) -> None:
        if not 0 <= self.decimals <= 255:
            raise ModelError("currency decimals must fit u8")
        if self.kind == CurrencyKind.NATIVE_SOL:
            if self.token_program != bytes(PUBKEY_BYTES) or self.mint != bytes(
                PUBKEY_BYTES
            ):
                raise ModelError("native SOL must use zero program and mint identities")
            if self.decimals != 9:
                raise ModelError("native SOL decimals must be nine")
        elif self.kind == CurrencyKind.SPL_TOKEN:
            _require_pubkey(self.token_program, "currency token program")
            _require_pubkey(self.mint, "currency mint")
            if self.token_program not in {LEGACY_TOKEN_PROGRAM, TOKEN_2022_PROGRAM}:
                raise ModelError("unsupported token program")
        else:
            raise ModelError("unknown currency kind")

    @classmethod
    def native_sol(cls) -> "CurrencyRef":
        return cls(CurrencyKind.NATIVE_SOL, bytes(PUBKEY_BYTES), bytes(PUBKEY_BYTES), 9)

    @classmethod
    def spl(cls, token_program: bytes, mint: bytes, decimals: int) -> "CurrencyRef":
        return cls(CurrencyKind.SPL_TOKEN, token_program, mint, decimals)

    def encode(self) -> bytes:
        return (
            bytes([int(self.kind)])
            + self.token_program
            + self.mint
            + bytes([self.decimals])
        )

    @classmethod
    def decode(cls, raw: bytes) -> "CurrencyRef":
        if len(raw) != 66:
            raise ModelError("currency reference must be 66 bytes")
        try:
            kind = CurrencyKind(raw[0])
        except ValueError as exc:
            raise ModelError("unknown currency kind") from exc
        return cls(kind, raw[1:33], raw[33:65], raw[65])


FLAG_REQUIRE_MINT_AUTHORITY_NONE = 1 << 0
FLAG_REQUIRE_FREEZE_AUTHORITY_NONE = 1 << 1
FLAG_REQUIRE_NONZERO_SUPPLY = 1 << 2
FLAG_REQUIRE_ACCOUNT_DELEGATE_NONE = 1 << 3
FLAG_REQUIRE_ACCOUNT_CLOSE_AUTHORITY_NONE = 1 << 4
KNOWN_FLAGS = (
    FLAG_REQUIRE_MINT_AUTHORITY_NONE
    | FLAG_REQUIRE_FREEZE_AUTHORITY_NONE
    | FLAG_REQUIRE_NONZERO_SUPPLY
    | FLAG_REQUIRE_ACCOUNT_DELEGATE_NONE
    | FLAG_REQUIRE_ACCOUNT_CLOSE_AUTHORITY_NONE
)
STRICT_FLAGS = KNOWN_FLAGS


def extension_mask(extensions: Iterable[ExtensionType]) -> int:
    mask = 0
    for extension in extensions:
        mask |= 1 << int(extension)
    return mask


def extensions_from_mask(mask: int) -> frozenset[ExtensionType]:
    if not 0 <= mask <= U64_MAX:
        raise ModelError("extension mask must fit u64")
    known_mask = extension_mask(ExtensionType)
    if mask & ~known_mask:
        raise ModelError("extension mask contains unknown bits")
    return frozenset(
        extension for extension in ExtensionType if mask & (1 << int(extension))
    )


@dataclass(frozen=True)
class RealmCollateralProfile:
    """Immutable, collateral-generic Realm policy."""

    collateral: CurrencyRef
    fee_currency: CurrencyRef
    liveness_currency: CurrencyRef
    max_supply_atoms: int
    flags: int = STRICT_FLAGS
    allowed_mint_extensions: int = 0
    required_mint_extensions: int = 0
    allowed_account_extensions: int = extension_mask(PROTOCOL_ACCOUNT_EXTENSION_CEILING)
    required_account_extensions: int = 0
    schema_version: int = PROFILE_SCHEMA_VERSION

    def __post_init__(self) -> None:
        if self.schema_version != PROFILE_SCHEMA_VERSION:
            raise ModelError("unsupported collateral-profile schema")
        if self.collateral.kind != CurrencyKind.SPL_TOKEN:
            raise ModelError("V1 collateral must be an SPL token")
        if not 1 <= self.max_supply_atoms <= U64_MAX:
            raise ModelError("maximum supply must be a positive u64")
        if self.flags & ~KNOWN_FLAGS:
            raise ModelError("profile contains unknown flags")
        if self.flags != STRICT_FLAGS:
            raise ModelError("Realm cannot weaken the V1 authority/state policy")
        native_sol = CurrencyRef.native_sol()
        if self.fee_currency not in {self.collateral, native_sol}:
            raise ModelError("V1 fee currency must be collateral or native SOL")
        if self.liveness_currency != native_sol:
            raise ModelError("V1 liveness currency must be native SOL")

        allowed_mint = extensions_from_mask(self.allowed_mint_extensions)
        required_mint = extensions_from_mask(self.required_mint_extensions)
        allowed_account = extensions_from_mask(self.allowed_account_extensions)
        required_account = extensions_from_mask(self.required_account_extensions)
        if not required_mint <= allowed_mint or not required_account <= allowed_account:
            raise ModelError("required extensions must also be allowed")
        if not allowed_mint <= PROTOCOL_MINT_EXTENSION_CEILING:
            raise ModelError("Realm cannot expand the V1 mint-extension ceiling")
        if not allowed_account <= PROTOCOL_ACCOUNT_EXTENSION_CEILING:
            raise ModelError("Realm cannot expand the V1 account-extension ceiling")
        if self.collateral.token_program == LEGACY_TOKEN_PROGRAM and any(
            (allowed_mint, required_mint, allowed_account, required_account)
        ):
            raise ModelError(
                "legacy SPL Token profile cannot declare Token-2022 extensions"
            )

    def canonical_bytes(self) -> bytes:
        return b"".join(
            (
                PROFILE_MAGIC,
                pack("<H", self.schema_version),
                pack("<H", self.flags),
                self.collateral.encode(),
                self.fee_currency.encode(),
                self.liveness_currency.encode(),
                pack("<Q", self.max_supply_atoms),
                pack("<Q", self.allowed_mint_extensions),
                pack("<Q", self.required_mint_extensions),
                pack("<Q", self.allowed_account_extensions),
                pack("<Q", self.required_account_extensions),
                bytes(PROFILE_RESERVED_BYTES),
            )
        )

    @classmethod
    def from_canonical_bytes(cls, raw: bytes) -> "RealmCollateralProfile":
        if len(raw) != 266:
            raise ModelError("collateral profile must be exactly 266 bytes")
        if raw[:8] != PROFILE_MAGIC:
            raise ModelError("invalid collateral-profile magic")
        if raw[-PROFILE_RESERVED_BYTES:] != bytes(PROFILE_RESERVED_BYTES):
            raise ModelError("collateral-profile reserved bytes must be zero")
        schema_version, flags = unpack_from("<HH", raw, 8)
        offset = 12
        currencies = []
        for _ in range(3):
            currencies.append(CurrencyRef.decode(raw[offset : offset + 66]))
            offset += 66
        values = unpack_from("<QQQQQ", raw, offset)
        profile = cls(
            collateral=currencies[0],
            fee_currency=currencies[1],
            liveness_currency=currencies[2],
            max_supply_atoms=values[0],
            allowed_mint_extensions=values[1],
            required_mint_extensions=values[2],
            allowed_account_extensions=values[3],
            required_account_extensions=values[4],
            flags=flags,
            schema_version=schema_version,
        )
        if profile.canonical_bytes() != raw:
            raise ModelError("non-canonical collateral-profile encoding")
        return profile

    def digest(self) -> bytes:
        return sha256(PROFILE_DOMAIN + self.canonical_bytes()).digest()

    def digest_hex(self) -> str:
        return self.digest().hex()


class RefusalCode(IntEnum):
    ACCEPT = 0
    WRONG_PROGRAM = 1
    WRONG_MINT = 2
    UNINITIALIZED = 3
    WRONG_DECIMALS = 4
    ZERO_SUPPLY = 5
    SUPPLY_EXCEEDS_PROFILE = 6
    MINT_AUTHORITY_PRESENT = 7
    FREEZE_AUTHORITY_PRESENT = 8
    UNKNOWN_EXTENSION = 9
    WRONG_EXTENSION_LOCATION = 10
    EXTENSION_NOT_ALLOWED = 11
    REQUIRED_EXTENSION_MISSING = 12
    FROZEN_ACCOUNT = 13
    WRONG_ACCOUNT_OWNER = 14
    DELEGATE_PRESENT = 15
    CLOSE_AUTHORITY_PRESENT = 16
    MALFORMED_EXTENSION_SET = 17


@dataclass(frozen=True)
class ValidationResult:
    code: RefusalCode
    detail: str

    @property
    def accepted(self) -> bool:
        return self.code == RefusalCode.ACCEPT


@dataclass(frozen=True)
class MintSnapshot:
    token_program: bytes
    mint: bytes
    initialized: bool
    decimals: int
    supply_atoms: int
    mint_authority: Optional[bytes]
    freeze_authority: Optional[bytes]
    extensions: tuple[int, ...] = ()


@dataclass(frozen=True)
class TokenAccountSnapshot:
    token_program: bytes
    mint: bytes
    owner_authority: bytes
    initialized: bool
    frozen: bool
    amount_atoms: int
    delegate: Optional[bytes]
    close_authority: Optional[bytes]
    extensions: tuple[int, ...] = ()


def _decode_snapshot_extensions(
    raw: tuple[int, ...], expected_location: frozenset[ExtensionType]
) -> tuple[Optional[frozenset[ExtensionType]], Optional[ValidationResult]]:
    if len(raw) != len(set(raw)):
        return None, ValidationResult(
            RefusalCode.MALFORMED_EXTENSION_SET, "duplicate extension discriminant"
        )
    decoded: set[ExtensionType] = set()
    for value in sorted(raw):
        try:
            extension = ExtensionType(value)
        except ValueError:
            return None, ValidationResult(
                RefusalCode.UNKNOWN_EXTENSION, f"unknown extension discriminant {value}"
            )
        if extension not in expected_location:
            return None, ValidationResult(
                RefusalCode.WRONG_EXTENSION_LOCATION,
                f"{extension.name} is not valid at this location",
            )
        decoded.add(extension)
    return frozenset(decoded), None


def validate_mint(
    profile: RealmCollateralProfile, snapshot: MintSnapshot
) -> ValidationResult:
    collateral = profile.collateral
    if snapshot.token_program != collateral.token_program:
        return ValidationResult(
            RefusalCode.WRONG_PROGRAM, "mint owner program mismatch"
        )
    if snapshot.mint != collateral.mint:
        return ValidationResult(RefusalCode.WRONG_MINT, "mint identity mismatch")
    if not snapshot.initialized:
        return ValidationResult(RefusalCode.UNINITIALIZED, "mint is not initialized")
    if snapshot.decimals != collateral.decimals:
        return ValidationResult(RefusalCode.WRONG_DECIMALS, "mint decimals mismatch")
    if not 0 <= snapshot.supply_atoms <= U64_MAX:
        raise ModelError("snapshot supply must fit u64")
    if profile.flags & FLAG_REQUIRE_NONZERO_SUPPLY and snapshot.supply_atoms == 0:
        return ValidationResult(RefusalCode.ZERO_SUPPLY, "mint supply is zero")
    if snapshot.supply_atoms > profile.max_supply_atoms:
        return ValidationResult(
            RefusalCode.SUPPLY_EXCEEDS_PROFILE, "mint supply exceeds profile ceiling"
        )
    if (
        profile.flags & FLAG_REQUIRE_MINT_AUTHORITY_NONE
        and snapshot.mint_authority is not None
    ):
        return ValidationResult(
            RefusalCode.MINT_AUTHORITY_PRESENT, "mint authority remains"
        )
    if (
        profile.flags & FLAG_REQUIRE_FREEZE_AUTHORITY_NONE
        and snapshot.freeze_authority is not None
    ):
        return ValidationResult(
            RefusalCode.FREEZE_AUTHORITY_PRESENT, "freeze authority remains"
        )
    extensions, error = _decode_snapshot_extensions(
        snapshot.extensions, MINT_EXTENSIONS
    )
    if error is not None:
        return error
    assert extensions is not None
    if snapshot.token_program == LEGACY_TOKEN_PROGRAM and extensions:
        return ValidationResult(
            RefusalCode.EXTENSION_NOT_ALLOWED,
            "legacy SPL Token mint has extension claims",
        )
    allowed = extensions_from_mask(profile.allowed_mint_extensions)
    required = extensions_from_mask(profile.required_mint_extensions)
    denied = extensions - allowed
    if denied:
        extension = min(denied, key=int)
        return ValidationResult(
            RefusalCode.EXTENSION_NOT_ALLOWED,
            EXTENSION_REFUSAL_REASON[extension],
        )
    missing = required - extensions
    if missing:
        return ValidationResult(
            RefusalCode.REQUIRED_EXTENSION_MISSING,
            f"missing {min(missing, key=int).name}",
        )
    return ValidationResult(
        RefusalCode.ACCEPT, "mint satisfies immutable Realm profile"
    )


def validate_hoard_account(
    profile: RealmCollateralProfile,
    snapshot: TokenAccountSnapshot,
    expected_owner_authority: bytes,
) -> ValidationResult:
    _require_pubkey(expected_owner_authority, "expected Hoard owner authority")
    collateral = profile.collateral
    if snapshot.token_program != collateral.token_program:
        return ValidationResult(
            RefusalCode.WRONG_PROGRAM, "token-account owner program mismatch"
        )
    if snapshot.mint != collateral.mint:
        return ValidationResult(RefusalCode.WRONG_MINT, "token-account mint mismatch")
    if not snapshot.initialized:
        return ValidationResult(
            RefusalCode.UNINITIALIZED, "token account is not initialized"
        )
    if snapshot.frozen:
        return ValidationResult(
            RefusalCode.FROZEN_ACCOUNT, "Hoard token account is frozen"
        )
    if snapshot.owner_authority != expected_owner_authority:
        return ValidationResult(
            RefusalCode.WRONG_ACCOUNT_OWNER, "Hoard owner authority mismatch"
        )
    if not 0 <= snapshot.amount_atoms <= U64_MAX:
        raise ModelError("snapshot amount must fit u64")
    if (
        profile.flags & FLAG_REQUIRE_ACCOUNT_DELEGATE_NONE
        and snapshot.delegate is not None
    ):
        return ValidationResult(
            RefusalCode.DELEGATE_PRESENT, "Hoard account has a delegate"
        )
    if (
        profile.flags & FLAG_REQUIRE_ACCOUNT_CLOSE_AUTHORITY_NONE
        and snapshot.close_authority is not None
    ):
        return ValidationResult(
            RefusalCode.CLOSE_AUTHORITY_PRESENT, "Hoard account has a close authority"
        )
    extensions, error = _decode_snapshot_extensions(
        snapshot.extensions, ACCOUNT_EXTENSIONS
    )
    if error is not None:
        return error
    assert extensions is not None
    if snapshot.token_program == LEGACY_TOKEN_PROGRAM and extensions:
        return ValidationResult(
            RefusalCode.EXTENSION_NOT_ALLOWED,
            "legacy SPL Token account has extension claims",
        )
    allowed = extensions_from_mask(profile.allowed_account_extensions)
    required = extensions_from_mask(profile.required_account_extensions)
    denied = extensions - allowed
    if denied:
        extension = min(denied, key=int)
        return ValidationResult(
            RefusalCode.EXTENSION_NOT_ALLOWED,
            EXTENSION_REFUSAL_REASON[extension],
        )
    missing = required - extensions
    if missing:
        return ValidationResult(
            RefusalCode.REQUIRED_EXTENSION_MISSING,
            f"missing {min(missing, key=int).name}",
        )
    return ValidationResult(RefusalCode.ACCEPT, "Hoard account satisfies Realm profile")


def dregg_dogfood_profile(
    *, decimals: int, max_supply_atoms: int
) -> RealmCollateralProfile:
    """Build the DREGG-specific instance without creating a protocol branch.

    The caller must obtain ``decimals`` and the desired ceiling from an
    authenticated mint snapshot before promotion.  This function performs no
    network lookup and makes no claim about DREGG's current chain state.
    """

    dregg = CurrencyRef.spl(LEGACY_TOKEN_PROGRAM, DREGG_MINT, decimals)
    return RealmCollateralProfile(
        collateral=dregg,
        fee_currency=dregg,
        liveness_currency=CurrencyRef.native_sol(),
        max_supply_atoms=max_supply_atoms,
        allowed_account_extensions=0,
    )


# ---------------------------------------------------------------------------
# Parent Realm Profile identity (P1-G join)
# ---------------------------------------------------------------------------
#
# Status: MODEL/PROPOSED (2026-08-18).  This section decides the parent/child
# relation named in `docs/implementation/ADVERSARIAL_REVIEW_V0.md` P1-G, where
# two unjoined digest algorithms existed:
#
#   child  (this lab):  SHA-256("dragons-clutch/collateral-profile/v1\0" || 266 bytes)
#   parent (Rust layout): SHA-256("dragons-clutch/profile/v1" || profile_bytes)
#
# The decision is that the collateral-policy digest is NOT the Realm's Profile
# ID.  It is one domain-separated subfield inside a broader parent Profile whose
# canonical bytes are hashed by the already-frozen Rust rule.  The parent hash
# function therefore does not change at all; what is frozen here is *which
# bytes* it consumes.  Nothing below authenticates a mint, an account, or a
# deployment: it is an offline identity composition only.

PARENT_PROFILE_MAGIC = b"DCPROF1\0"
PARENT_PROFILE_DOMAIN = b"dragons-clutch/profile/v1"
PARENT_PROFILE_SCHEMA_VERSION = 1
PARENT_PROFILE_FLAGS = 0
PARENT_PROFILE_BYTES = 64
PARENT_PROFILE_RESERVED_BYTES = 16
DIGEST_BYTES = 32

SUBFIELD_COLLATERAL_POLICY = 1
KNOWN_SUBFIELD_TAGS = frozenset({SUBFIELD_COLLATERAL_POLICY})


@dataclass(frozen=True)
class ProfileIdentity:
    """The parent Realm Profile identity that embeds one collateral subfield.

    Exactly 64 canonical bytes:

    ===== ===== =================================================
    Off.  Bytes Field
    ===== ===== =================================================
    0     8     ASCII ``DCPROF1`` followed by one zero byte
    8     2     parent schema version, little-endian ``u16``
    10    2     parent flags, little-endian ``u16`` (zero in V1)
    12    2     subfield tag, little-endian ``u16``
    14    2     subfield schema version, little-endian ``u16``
    16    32    collateral-policy digest (the child digest)
    48    16    zero reserved bytes
    ===== ===== =================================================

    The identity is ``SHA-256(PARENT_PROFILE_DOMAIN || canonical_bytes())``.
    """

    collateral_policy_digest: bytes
    subfield_tag: int = SUBFIELD_COLLATERAL_POLICY
    subfield_schema_version: int = PROFILE_SCHEMA_VERSION
    schema_version: int = PARENT_PROFILE_SCHEMA_VERSION
    flags: int = PARENT_PROFILE_FLAGS

    def __post_init__(self) -> None:
        if (
            not isinstance(self.collateral_policy_digest, bytes)
            or len(self.collateral_policy_digest) != DIGEST_BYTES
        ):
            raise ModelError("collateral-policy digest must be exactly 32 bytes")
        if self.collateral_policy_digest == bytes(DIGEST_BYTES):
            raise ModelError("collateral-policy digest cannot be zero")
        if self.schema_version != PARENT_PROFILE_SCHEMA_VERSION:
            raise ModelError("unsupported parent-profile schema")
        if self.flags != PARENT_PROFILE_FLAGS:
            raise ModelError("parent profile carries unknown flags")
        if self.subfield_tag not in KNOWN_SUBFIELD_TAGS:
            raise ModelError("unknown parent-profile subfield tag")
        if self.subfield_schema_version != PROFILE_SCHEMA_VERSION:
            raise ModelError("unsupported collateral-profile subfield schema")

    @classmethod
    def from_profile(cls, profile: RealmCollateralProfile) -> "ProfileIdentity":
        """Compose the parent identity over one collateral profile."""

        return cls(
            collateral_policy_digest=profile.digest(),
            subfield_schema_version=profile.schema_version,
        )

    def canonical_bytes(self) -> bytes:
        return b"".join(
            (
                PARENT_PROFILE_MAGIC,
                pack("<H", self.schema_version),
                pack("<H", self.flags),
                pack("<H", self.subfield_tag),
                pack("<H", self.subfield_schema_version),
                self.collateral_policy_digest,
                bytes(PARENT_PROFILE_RESERVED_BYTES),
            )
        )

    @classmethod
    def from_canonical_bytes(cls, raw: bytes) -> "ProfileIdentity":
        if len(raw) != PARENT_PROFILE_BYTES:
            raise ModelError("parent profile must be exactly 64 bytes")
        if raw[:8] != PARENT_PROFILE_MAGIC:
            raise ModelError("invalid parent-profile magic")
        if raw[-PARENT_PROFILE_RESERVED_BYTES:] != bytes(PARENT_PROFILE_RESERVED_BYTES):
            raise ModelError("parent-profile reserved bytes must be zero")
        schema_version, flags, subfield_tag, subfield_schema_version = unpack_from(
            "<HHHH", raw, 8
        )
        identity = cls(
            collateral_policy_digest=raw[16:48],
            subfield_tag=subfield_tag,
            subfield_schema_version=subfield_schema_version,
            schema_version=schema_version,
            flags=flags,
        )
        if identity.canonical_bytes() != raw:
            raise ModelError("non-canonical parent-profile encoding")
        return identity

    def digest(self) -> bytes:
        """The parent Profile ID, using the already-frozen Rust hash rule."""

        return sha256(PARENT_PROFILE_DOMAIN + self.canonical_bytes()).digest()

    def digest_hex(self) -> str:
        return self.digest().hex()

    def binds(self, profile: RealmCollateralProfile) -> bool:
        """Whether this identity commits to exactly ``profile``."""

        return (
            self.subfield_tag == SUBFIELD_COLLATERAL_POLICY
            and self.subfield_schema_version == profile.schema_version
            and self.collateral_policy_digest == profile.digest()
        )


def compose_profile_identity(profile: RealmCollateralProfile) -> ProfileIdentity:
    """Convenience alias for :meth:`ProfileIdentity.from_profile`."""

    return ProfileIdentity.from_profile(profile)


def verify_profile_identity(
    profile: RealmCollateralProfile, parent_bytes: bytes
) -> ProfileIdentity:
    """Decode parent bytes and refuse unless they bind ``profile`` exactly.

    This is the check an eventual adapter owes: decoding a well-formed parent
    profile is not enough, because a well-formed parent can commit to somebody
    else's collateral policy.
    """

    identity = ProfileIdentity.from_canonical_bytes(parent_bytes)
    if not identity.binds(profile):
        raise ModelError("parent profile does not bind this collateral policy")
    return identity
