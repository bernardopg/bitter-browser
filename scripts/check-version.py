#!/usr/bin/env python3
"""Check that every file declaring the release version agrees.

Usage: check-version.py [EXPECTED]. EXPECTED may be a tag such as v1.2.3.
"""
import json
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def read(path):
    return (ROOT / path).read_text(encoding="utf-8")


def one(pattern, text, source):
    found = re.findall(pattern, text, re.M)
    if len(found) != 1:
        sys.exit(f"{source}: expected one version match, found {found}")
    return found[0]


def versions():
    yield "Cargo.toml", tomllib.loads(read("Cargo.toml"))["workspace"]["package"]["version"]
    lock = tomllib.loads(read("Cargo.lock"))
    for package in lock["package"]:
        if package["name"].startswith("bitter-") and "source" not in package:
            yield f"Cargo.lock ({package['name']})", package["version"]
    yield "meson.build", one(r"^\s*version:\s*'([^']+)'", read("meson.build"), "meson.build")
    yield "packaging/arch/PKGBUILD", one(r"^pkgver=(\S+)$", read("packaging/arch/PKGBUILD"),
                                         "packaging/arch/PKGBUILD")
    yield "packaging/arch-bin/PKGBUILD", one(r"^pkgver=(\S+)$", read("packaging/arch-bin/PKGBUILD"),
                                             "packaging/arch-bin/PKGBUILD")
    yield "packaging/rpm/bitter-browser.spec", one(r"^Version:\s+(\S+)$",
                                                   read("packaging/rpm/bitter-browser.spec"),
                                                   "packaging/rpm/bitter-browser.spec")
    metainfo = read("crates/bitter-ui/data/br.com.bebitterbebetter.browser.Bitter.metainfo.xml")
    releases = re.findall(r'<release version="([^"]+)"', metainfo)
    if not releases:
        sys.exit("metainfo: no <release> entries")
    yield "metainfo (newest release)", releases[0]
    changelog = re.findall(r"^## \[(\d+\.\d+\.\d+)\]", read("CHANGELOG.md"), re.M)
    if not changelog:
        sys.exit("CHANGELOG.md: no released versions")
    yield "CHANGELOG.md (newest release)", changelog[0]
    readme = read("README.md")
    for bundle in re.findall(r"bitter-browser-(\d+\.\d+\.\d+)-x86_64\.flatpak", readme):
        yield "README.md (bundle name)", bundle


def main():
    found = list(versions())
    expected = sys.argv[1].removeprefix("v") if len(sys.argv) > 1 else found[0][1]
    if not re.fullmatch(r"\d+\.\d+\.\d+", expected):
        sys.exit(f"Not a release version: {expected}")
    wrong = [(source, version) for source, version in found if version != expected]
    for source, version in found:
        print(f"{'ok ' if version == expected else 'BAD'} {source}: {version}")
    if wrong:
        sys.exit(f"{len(wrong)} file(s) do not declare version {expected}")
    print(json.dumps({"version": expected}))


if __name__ == "__main__":
    main()
