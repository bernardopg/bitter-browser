#!/usr/bin/env python3
"""Check that flatpak/cargo-sources.json vendors exactly the crates in Cargo.lock.

Regenerate the file with scripts/update-flatpak-sources.sh when this fails.
"""
import json
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"

lock = tomllib.loads((ROOT / "Cargo.lock").read_text())
expected = {
    f"https://static.crates.io/crates/{p['name']}/{p['name']}-{p['version']}.crate": p["checksum"]
    for p in lock["package"]
    if p.get("source") == REGISTRY
}
other = [p["name"] for p in lock["package"] if "source" in p and p["source"] != REGISTRY]
if other:
    sys.exit(f"Unsupported dependency sources: {other}")

sources = json.loads((ROOT / "flatpak/cargo-sources.json").read_text())
vendored = {s["url"]: s["sha256"] for s in sources if s.get("type") == "archive"}

missing = sorted(set(expected) - set(vendored))
extra = sorted(set(vendored) - set(expected))
mismatched = sorted(url for url in set(expected) & set(vendored) if expected[url] != vendored[url])
for label, urls in (("missing", missing), ("not in Cargo.lock", extra), ("checksum differs", mismatched)):
    for url in urls:
        print(f"{label}: {url}")
if missing or extra or mismatched:
    sys.exit("flatpak/cargo-sources.json is out of date: run scripts/update-flatpak-sources.sh")
print(f"flatpak/cargo-sources.json matches Cargo.lock ({len(expected)} crates)")
