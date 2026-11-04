"""Check links outside the Sphinx source tree against one built language edition."""

import re
from argparse import ArgumentParser
from functools import lru_cache
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit


BOOK = "https://jij-inc-ommx.readthedocs-hosted.com/"
ROOT = Path(__file__).resolve().parents[1]


class Anchors(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids = set()

    def handle_starttag(self, tag, attrs):
        if element_id := dict(attrs).get("id"):
            self.ids.add(element_id)


@lru_cache
def anchors(page):
    parser = Anchors()
    parser.feed(page.read_text(encoding="utf-8"))
    return parser.ids


def entry_links():
    sources = [
        ROOT / "README.md",
        ROOT / "DEVELOPMENT.md",
        ROOT / "python/ommx/pyproject.toml",
    ]
    for adapter in sorted((ROOT / "python").glob("ommx-*-adapter")):
        sources.extend([adapter / "README.md", adapter / "pyproject.toml"])
    for source in sources:
        for url in re.findall(
            re.escape(BOOK) + r"""[^\s()"'<>]+""", source.read_text()
        ):
            yield source, url


def main():
    parser = ArgumentParser(description=__doc__)
    parser.add_argument("language", choices=["en", "ja"])
    parser.add_argument("html", type=Path, help="Sphinx HTML output directory")
    args = parser.parse_args()
    errors = []
    count = 0
    prefix = f"/{args.language}/latest/"
    for source, url in entry_links():
        parts = urlsplit(url)
        path, fragment = parts.path, unquote(parts.fragment)
        if not path.startswith(("/en/latest/", "/ja/latest/")):
            errors.append(f"{source.relative_to(ROOT)}: unexpected Book URL {url}")
            continue
        if not path.startswith(prefix):
            continue
        page = args.html / unquote(path.removeprefix(prefix))
        if path.endswith("/"):
            page /= "index.html"
        if not page.is_file() or (fragment and fragment not in anchors(page)):
            errors.append(f"{source.relative_to(ROOT)}: missing destination {url}")
        count += 1
    if errors:
        raise SystemExit("\n".join(errors))
    print(f"{args.language}: checked {count} links to the built Book and API Reference")


if __name__ == "__main__":
    main()
