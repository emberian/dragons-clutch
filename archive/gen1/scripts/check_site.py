#!/usr/bin/env python3
"""Check the hand-authored Pages tree without network access or dependencies."""

from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1] / "site"


class Page(HTMLParser):
    def __init__(self, path: Path) -> None:
        super().__init__()
        self.path = path
        self.ids: set[str] = set()
        self.duplicate_ids: set[str] = set()
        self.h1_count = 0
        self.references: list[tuple[str, str]] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        fields = dict(attrs)
        if fields.get("id"):
            value = fields["id"] or ""
            if value in self.ids:
                self.duplicate_ids.add(value)
            self.ids.add(value)
        if tag == "h1":
            self.h1_count += 1
        for name in ("href", "src"):
            if fields.get(name):
                self.references.append((name, fields[name] or ""))


def main() -> None:
    pages: dict[Path, Page] = {}
    for path in sorted(ROOT.glob("*.html")):
        page = Page(path)
        page.feed(path.read_text(encoding="utf-8"))
        pages[path.resolve()] = page

    failures: list[str] = []
    for path, page in pages.items():
        if page.h1_count != 1:
            failures.append(f"{path.name}: expected one h1, found {page.h1_count}")
        for duplicate in sorted(page.duplicate_ids):
            failures.append(f"{path.name}: duplicate id: {duplicate}")
        for attribute, reference in page.references:
            parsed = urlsplit(reference)
            if parsed.scheme or parsed.netloc:
                failures.append(f"{path.name}: external {attribute} is forbidden: {reference}")
                continue
            target_text = unquote(parsed.path)
            target = (path.parent / target_text).resolve() if target_text else path
            try:
                target.relative_to(ROOT.resolve())
            except ValueError:
                failures.append(f"{path.name}: {attribute} escapes site/: {reference}")
                continue
            if not target.exists():
                failures.append(f"{path.name}: missing {attribute} target: {reference}")
                continue
            if parsed.fragment and target.suffix == ".html":
                target_page = pages.get(target)
                if target_page is None or parsed.fragment not in target_page.ids:
                    failures.append(f"{path.name}: missing fragment: {reference}")

    if failures:
        raise SystemExit("\n".join(failures))
    print(f"site check passed: {len(pages)} HTML pages; local references only")


if __name__ == "__main__":
    main()
