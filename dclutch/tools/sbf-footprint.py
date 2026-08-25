#!/usr/bin/env python3
"""Report exact ELF sections and equivalent Loader V3 capitalization.

This utility deliberately uses only Python's standard library.  It reads the
ELF64 section table itself, so a measurement does not depend on whichever
``size`` implementation happens to be first on PATH.

The rent calculation is an explicitly named local-default profile, not a live
cluster observation.  Its defaults match ``Rent::default()`` in the pinned
dClutch evidence: 6,960 lamports per allocated byte including the 128-byte
account-storage overhead, with an 890,880-lamport floor.  Loader V3 is modeled
as one 36-byte Program account plus one ProgramData account containing 45 bytes
of metadata followed by the complete ELF.
"""

from __future__ import annotations

import argparse
import hashlib
import struct
import sys
from dataclasses import dataclass
from pathlib import Path


ELF64_LITTLE_ENDIAN = struct.Struct("<16sHHIQQQIHHHHHH")
SECTION64_LITTLE_ENDIAN = struct.Struct("<IIQQQQIIQQ")


@dataclass(frozen=True)
class RentProfile:
    """Explicit inputs to an equivalent Loader V3 capitalization estimate."""

    lamports_per_allocated_byte: int = 6_960
    account_storage_overhead: int = 128
    minimum_balance_floor: int = 890_880
    program_account_bytes: int = 36
    programdata_metadata_bytes: int = 45

    def minimum_balance(self, data_bytes: int) -> int:
        """Return the profile's rent-exempt minimum for one data allocation."""

        if data_bytes < 0:
            raise ValueError("data width must be nonnegative")
        calculated = (data_bytes + self.account_storage_overhead) * (
            self.lamports_per_allocated_byte
        )
        return max(self.minimum_balance_floor, calculated)

    def loader_v3_capitalization(self, elf_bytes: int) -> int:
        """Return Program plus exact ProgramData capitalization in lamports."""

        return self.minimum_balance(
            self.program_account_bytes
        ) + self.minimum_balance(self.programdata_metadata_bytes + elf_bytes)


@dataclass(frozen=True)
class ElfReport:
    """Exact file identity and named section widths for one SBF ELF."""

    path: Path
    sha256: str
    file_bytes: int
    sections: dict[str, int]

    @property
    def allocated_section_bytes(self) -> int:
        """Return the sum of all nonempty named sections."""

        return sum(self.sections.values())


def _cstring(table: bytes, offset: int) -> str:
    if offset < 0 or offset >= len(table):
        raise ValueError(f"section-name offset {offset} exceeds string table")
    end = table.find(b"\0", offset)
    if end < 0:
        raise ValueError("unterminated section name")
    return table[offset:end].decode("ascii")


def inspect_elf(path: Path) -> ElfReport:
    """Hostile-parse the portions of an ELF64 little-endian section table used here."""

    body = path.read_bytes()
    if len(body) < ELF64_LITTLE_ENDIAN.size:
        raise ValueError("file is shorter than an ELF64 header")
    header = ELF64_LITTLE_ENDIAN.unpack_from(body)
    identity = header[0]
    if identity[:4] != b"\x7fELF":
        raise ValueError("file does not have ELF magic")
    if identity[4] != 2:
        raise ValueError("only ELF64 is supported")
    if identity[5] != 1:
        raise ValueError("only little-endian ELF is supported")

    section_offset = header[6]
    section_entry_bytes = header[11]
    section_count = header[12]
    names_index = header[13]
    if section_entry_bytes != SECTION64_LITTLE_ENDIAN.size:
        raise ValueError(
            f"unexpected ELF64 section entry width {section_entry_bytes}"
        )
    if section_count == 0 or names_index >= section_count:
        raise ValueError("missing or invalid section-name string table")
    section_table_end = section_offset + section_count * section_entry_bytes
    if section_table_end > len(body):
        raise ValueError("section table exceeds file")

    raw_sections = [
        SECTION64_LITTLE_ENDIAN.unpack_from(
            body, section_offset + index * section_entry_bytes
        )
        for index in range(section_count)
    ]
    names_header = raw_sections[names_index]
    names_offset = names_header[4]
    names_bytes = names_header[5]
    names_end = names_offset + names_bytes
    if names_end > len(body):
        raise ValueError("section-name string table exceeds file")
    names = body[names_offset:names_end]

    sections: dict[str, int] = {}
    for raw in raw_sections:
        name = _cstring(names, raw[0])
        width = raw[5]
        if name:
            if name in sections:
                raise ValueError(f"duplicate named section {name!r}")
            sections[name] = width
    return ElfReport(
        path=path,
        sha256=hashlib.sha256(body).hexdigest(),
        file_bytes=len(body),
        sections=sections,
    )


def arguments(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("elf", nargs="+", type=Path, help="SBF ELF to inspect")
    parser.add_argument(
        "--lamports-per-allocated-byte",
        type=int,
        default=6_960,
        help="two-year rent multiplier used by the named profile",
    )
    parser.add_argument(
        "--account-storage-overhead", type=int, default=128
    )
    parser.add_argument("--minimum-balance-floor", type=int, default=890_880)
    parser.add_argument("--program-account-bytes", type=int, default=36)
    parser.add_argument("--programdata-metadata-bytes", type=int, default=45)
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = arguments(argv)
    profile = RentProfile(
        lamports_per_allocated_byte=args.lamports_per_allocated_byte,
        account_storage_overhead=args.account_storage_overhead,
        minimum_balance_floor=args.minimum_balance_floor,
        program_account_bytes=args.program_account_bytes,
        programdata_metadata_bytes=args.programdata_metadata_bytes,
    )
    if min(
        profile.lamports_per_allocated_byte,
        profile.account_storage_overhead,
        profile.minimum_balance_floor,
        profile.program_account_bytes,
        profile.programdata_metadata_bytes,
    ) < 0:
        raise ValueError("rent-profile inputs must be nonnegative")

    reports = [inspect_elf(path) for path in args.elf]
    columns = [".text", ".rodata", ".data.rel.ro", ".rel.dyn"]
    print(
        "artifact\tsha256\telf_bytes\t"
        + "\t".join(name.removeprefix(".") + "_bytes" for name in columns)
        + "\tnamed_section_bytes\tloader_v3_lamports"
    )
    total_files = 0
    total_capitalization = 0
    for report in reports:
        capitalization = profile.loader_v3_capitalization(report.file_bytes)
        total_files += report.file_bytes
        total_capitalization += capitalization
        widths = [str(report.sections.get(name, 0)) for name in columns]
        print(
            f"{report.path}\t{report.sha256}\t{report.file_bytes}\t"
            + "\t".join(widths)
            + f"\t{report.allocated_section_bytes}\t{capitalization}"
        )
    if len(reports) > 1:
        print(
            f"TOTAL\t-\t{total_files}\t-\t-\t-\t-\t-\t{total_capitalization}"
        )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, ValueError) as error:
        print(f"sbf-footprint: {error}", file=sys.stderr)
        raise SystemExit(2) from error
