#!/usr/bin/env python3
"""Report the exact static stack frame LLVM gave each SBF function.

WHY THIS EXISTS. `cargo build-sbf` says one thing about frames and it says it
only at the wall: "A function call in method X overwrites values in the frame."
That message is a BOOLEAN dressed as a count -- it is emitted once per call site
inside an already-over-bound function, so the number drifts (75, then 78, then
82, on the same defect, under unrelated edits) and it never says how close to
the bound anything is. A gate built on it is green at 4,095 bytes and red at
4,097, with nothing in between, which is exactly how
`hot_v3::execute_child_routes_v3` sat at 3,712 of 4,096 in the shipped Trading
link -- reporting zero, 384 bytes from the wall -- while the same function was
at 5,184 in the dealer accelerator's link and had been for a wave.

So measure the frame instead of counting its complaints. LLVM will write one
`.stack_sizes` section per function when asked, and the number in it is the
static frame size the backend actually allocated.

HOW TO ASK. The flag is `-Zemit-stack-sizes`, which is unstable, and this
repository pins a stable toolchain -- so it needs `RUSTC_BOOTSTRAP=1` and it is
a MEASUREMENT build, never a shipped one:

    RUSTC_BOOTSTRAP=1 RUSTFLAGS="-Zemit-stack-sizes --emit=obj,link" \\
        CARGO_TARGET_DIR=<scratch> cargo build-sbf --manifest-path <program>
    tools/sbf-frame-sizes.py <scratch>/sbpf-solana-solana/release/deps/<crate>.o

The flag adds a section; it does not change codegen. Checked both ways on the
2026-08-27 defect: the measured 5,184 agrees with the plain build's 82
diagnostics and with an independent disassembly's 5,176, and after the fix the
measured 2,752 agrees with the plain build's zero.

This reads the ELF itself with the standard library only, like
`tools/sbf-footprint.py` and for the same reason: a measurement should not
depend on whichever `llvm-readobj` happens to be on PATH, and the platform-tools
copy lives at a version-stamped path that moves under us.

WHAT IT REFUSES. A frame at or over the bound (SBPF v0: 4,096 bytes). It also
refuses an object with NO `.stack_sizes` sections at all, because a measurement
tool that silently measures nothing is worse than no tool -- that is the same
failure shape as a warm target directory reporting zero diagnostics for a crate
it never recompiled.

usage: sbf-frame-sizes.py OBJECT [--bound N] [--top N] [--quiet]
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

# SBPF v0 gives every call frame exactly this many bytes. A function whose
# locals plus outgoing call arguments exceed it does not get a bigger frame --
# it gets a diagnostic and a call that writes over its own locals.
SBPF_V0_FRAME_BYTES = 4096

ELF_SECTION_HEADER = struct.Struct("<IIQQQQIIQQ")
ELF_SYMBOL = struct.Struct("<IBBHQQ")
ELF_REL = struct.Struct("<QQ")
SHT_REL = 9
STT_FUNC = 2
STT_SECTION = 3


class ElfError(Exception):
    """The file is not an ELF this tool can read."""


class Elf:
    """Just enough ELF64 little-endian to find named sections and symbols."""

    def __init__(self, path: Path) -> None:
        self.raw = path.read_bytes()
        if self.raw[:4] != b"\x7fELF":
            raise ElfError(f"{path} is not an ELF file")
        if self.raw[4] != 2 or self.raw[5] != 1:
            raise ElfError(f"{path} is not ELF64 little-endian")
        self.shoff = struct.unpack_from("<Q", self.raw, 0x28)[0]
        self.shentsize, self.shnum, self.shstrndx = struct.unpack_from(
            "<HHH", self.raw, 0x3A
        )
        self.sections = [self._section(i) for i in range(self.shnum)]
        names = self.sections[self.shstrndx]
        for section in self.sections:
            section["name"] = self._string(names["offset"], section["name_offset"])

    def _section(self, index: int) -> dict:
        offset = self.shoff + index * self.shentsize
        fields = ELF_SECTION_HEADER.unpack_from(self.raw, offset)
        return {
            "index": index,
            "name_offset": fields[0],
            "type": fields[1],
            "offset": fields[4],
            "size": fields[5],
            "link": fields[6],
            "info": fields[7],
        }

    def _string(self, table_offset: int, offset: int) -> str:
        start = table_offset + offset
        end = self.raw.index(b"\0", start)
        return self.raw[start:end].decode("utf-8", "replace")

    def data(self, section: dict) -> bytes:
        return self.raw[section["offset"] : section["offset"] + section["size"]]

    def _symbol(self, symtab: dict, symbol_index: int) -> tuple[str, int, int]:
        strtab = self.sections[symtab["link"]]
        offset = symtab["offset"] + symbol_index * ELF_SYMBOL.size
        name_offset, info, _, shndx, _, _ = ELF_SYMBOL.unpack_from(self.raw, offset)
        return self._string(strtab["offset"], name_offset), info & 0xF, shndx

    def symbol_name(self, symtab_index: int, symbol_index: int) -> str:
        """Name the function a relocation points at.

        The relocation in `.rel.stack_sizes` does NOT point at the function
        symbol. It points at an anonymous local label (`.L0`) inside the
        function's own `.text.<mangled>` section, which `-ffunction-sections`
        gives every function. Reading the relocation's symbol name directly
        therefore reports every frame in the object as `.L0`, which is a table
        of correct numbers attached to nothing.

        So resolve through the SECTION the label lives in: prefer a real
        STT_FUNC defined there, and fall back to the section's own name with
        its `.text.` prefix removed, which is the mangled symbol verbatim.
        """

        symtab = self.sections[symtab_index]
        name, kind, section_index = self._symbol(symtab, symbol_index)
        if kind == STT_FUNC:
            return name
        if 0 < section_index < len(self.sections):
            for index in range(symtab["size"] // ELF_SYMBOL.size):
                candidate, candidate_kind, candidate_section = self._symbol(
                    symtab, index
                )
                if candidate_kind == STT_FUNC and candidate_section == section_index:
                    return candidate
            section_name = self.sections[section_index]["name"]
            if section_name.startswith(".text."):
                return section_name[len(".text.") :]
            return section_name
        return name


def _uleb128(data: bytes, position: int) -> int:
    """Return the ULEB128 value at ``position``; the frame size is one field."""

    value = 0
    shift = 0
    while True:
        if position >= len(data):
            raise ElfError("truncated ULEB128 in .stack_sizes")
        byte = data[position]
        position += 1
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value
        shift += 7


def frame_sizes(elf: Elf) -> list[tuple[int, str]]:
    """Return (frame_bytes, mangled_symbol) for every measured function.

    The compiler emits ONE `.stack_sizes` section per function, each holding a
    single (address, size) record, with a sibling `.rel.stack_sizes` whose only
    relocation names the function. The address field in a relocatable object is
    a placeholder, so the relocation is the only thing that says whose frame
    this is -- which is why this reads objects and not linked programs.
    """

    relocations = {
        section["info"]: section
        for section in elf.sections
        if section["type"] == SHT_REL and section["name"] == ".rel.stack_sizes"
    }
    measured: list[tuple[int, str]] = []
    for section in elf.sections:
        if section["name"] != ".stack_sizes":
            continue
        relocation = relocations.get(section["index"])
        if relocation is None:
            continue
        _, info = ELF_REL.unpack_from(elf.raw, relocation["offset"])
        symbol = elf.symbol_name(relocation["link"], info >> 32)
        measured.append((_uleb128(elf.data(section), 8), symbol))
    measured.sort(reverse=True)
    return measured


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("object", type=Path, help="a relocatable SBF object file")
    parser.add_argument("--bound", type=int, default=SBPF_V0_FRAME_BYTES)
    parser.add_argument("--top", type=int, default=8)
    parser.add_argument(
        "--quiet",
        action="store_true",
        help="print only the deepest frame and any refusal",
    )
    arguments = parser.parse_args()

    try:
        measured = frame_sizes(Elf(arguments.object))
    except (ElfError, OSError, ValueError, IndexError) as error:
        print(f"sbf-frame-sizes: cannot measure {arguments.object}: {error}", file=sys.stderr)
        return 2

    if not measured:
        print(
            f"sbf-frame-sizes: {arguments.object} carries no .stack_sizes sections. "
            "The build did not pass -Zemit-stack-sizes, or it did not recompile "
            "this crate at all. Either way this is silence, not a measurement, "
            "and it does not count as a clean frame report.",
            file=sys.stderr,
        )
        return 2

    over = [row for row in measured if row[0] >= arguments.bound]
    deepest, deepest_symbol = measured[0]
    if arguments.quiet:
        print(
            f"  deepest frame {deepest} of {arguments.bound} "
            f"({arguments.bound - deepest} spare)  {deepest_symbol}"
        )
    else:
        print(
            f"  {len(measured)} measured frames, bound {arguments.bound}; deepest:"
        )
        for size, symbol in measured[: arguments.top]:
            spare = arguments.bound - size
            marker = "  OVER" if size >= arguments.bound else f"{spare:6d} spare"
            print(f"    {size:6d}  {marker}  {symbol}")

    if over:
        print(
            f"sbf-frame-sizes: REFUSING -- {len(over)} function(s) at or over the "
            f"{arguments.bound}-byte frame bound. A call from one of these writes "
            "over its own locals and the toolchain says it may execute as "
            "undefined behavior:",
            file=sys.stderr,
        )
        for size, symbol in over:
            print(f"  {size} bytes  {symbol}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
