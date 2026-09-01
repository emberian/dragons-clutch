#!/usr/bin/env python3
"""Emit one bounded canonical member listing for a Node distribution archive."""

from __future__ import annotations

import argparse
from pathlib import Path, PurePosixPath
import stat
import sys
import tarfile


MAX_ARCHIVE_BYTES = 128 * 1024 * 1024
MAX_MEMBER_COUNT = 100_000
MAX_MEMBER_NAME_BYTES = 4_096
MAX_LISTING_BYTES = 16 * 1024 * 1024


class Refusal(RuntimeError):
    pass


def canonical_name(name: str, index: int) -> str:
    try:
        encoded = name.encode("utf-8")
    except UnicodeEncodeError as error:
        raise Refusal(f"archive member {index} is not canonical UTF-8: {error}") from error
    if (
        not encoded
        or len(encoded) > MAX_MEMBER_NAME_BYTES
        or b"\x00" in encoded
        or b"\n" in encoded
        or b"\r" in encoded
    ):
        raise Refusal(f"archive member {index} has an empty, NUL, or overlong name")
    if name.startswith("/") or "\\" in name:
        raise Refusal(f"archive member {index} has an absolute or non-POSIX name")
    path = PurePosixPath(name)
    if any(part in {"", ".", ".."} for part in name.split("/")) or str(path) != name:
        raise Refusal(f"archive member {index} has a noncanonical path")
    return name


def listing(archive: Path, required: tuple[str, ...]) -> bytes:
    try:
        mode = archive.lstat().st_mode
    except FileNotFoundError as error:
        raise Refusal(f"archive is missing: {archive}") from error
    if not stat.S_ISREG(mode) or archive.stat().st_size <= 0:
        raise Refusal("archive must be one nonempty regular non-symlink file")
    if archive.stat().st_size > MAX_ARCHIVE_BYTES:
        raise Refusal(f"archive exceeds {MAX_ARCHIVE_BYTES} bytes")
    names: list[str] = []
    seen: set[str] = set()
    regular: set[str] = set()
    listing_bytes = 0
    try:
        with tarfile.open(archive, "r:xz") as source:
            for index, member in enumerate(source, start=1):
                if index > MAX_MEMBER_COUNT:
                    raise Refusal(f"archive exceeds {MAX_MEMBER_COUNT} members")
                name = canonical_name(member.name, index)
                if name in seen:
                    raise Refusal(f"archive repeats member {name!r}")
                seen.add(name)
                if member.isfile():
                    regular.add(name)
                names.append(name)
                listing_bytes += len(name.encode("utf-8")) + 1
                if listing_bytes > MAX_LISTING_BYTES:
                    raise Refusal(f"archive member listing exceeds {MAX_LISTING_BYTES} bytes")
    except (tarfile.TarError, EOFError, OSError) as error:
        raise Refusal(f"archive is not one readable xz tar: {error}") from error
    if not names:
        raise Refusal("archive member listing is empty")
    for name in required:
        canonical_name(name, 0)
        if name not in seen:
            raise Refusal(f"archive omitted required member {name!r}")
        if name not in regular:
            raise Refusal(f"archive required member {name!r} is not a regular file")
    return ("\n".join(names) + "\n").encode()


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--archive", required=True)
    result.add_argument("--required", action="append", default=[])
    return result


def main() -> int:
    arguments = parser().parse_args()
    try:
        sys.stdout.buffer.write(
            listing(Path(arguments.archive), tuple(arguments.required))
        )
        return 0
    except Refusal as error:
        print(f"NODE ARCHIVE REFUSED: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
