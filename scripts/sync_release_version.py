#!/usr/bin/env python3

from __future__ import annotations

import hashlib
import re
import sys
import urllib.request
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib


ROOT = Path(__file__).resolve().parent.parent
CARGO_PATH = ROOT / "Cargo.toml"
FORMULA_PATH = ROOT / "Formula" / "uq.rb"
INSTALL_PATH = ROOT / "install.sh"
ARCHES = ("arm64", "x86_64")
RELEASE_URL = "https://github.com/yxxbc/uq/releases/download/v{version}/uq-v{version}-macos-{arch}.tar.gz"


def read_version() -> str:
    cargo_text = CARGO_PATH.read_text(encoding="utf-8")
    data = tomllib.loads(cargo_text)
    version = data["package"]["version"]
    if not version:
        raise ValueError("Cargo.toml package.version is empty")
    return version


def sync_formula(version: str) -> None:
    if not FORMULA_PATH.exists():
        raise FileNotFoundError(f"Formula not found: {FORMULA_PATH}")

    text = FORMULA_PATH.read_text(encoding="utf-8")
    original = text

    text = re.sub(
        r'(?m)^  version ".*?"$',
        f'  version "{version}"',
        text,
        count=1,
    )
    text = re.sub(
        r'(?m)^\s*url "https://github\.com/yxxbc/uq/releases/download/v.*?/uq-v.*?-macos-arm64\.tar\.gz"$',
        f'      url "https://github.com/yxxbc/uq/releases/download/v{version}/uq-v{version}-macos-arm64.tar.gz"',
        text,
        count=1,
    )
    text = re.sub(
        r'(?m)^\s*url "https://github\.com/yxxbc/uq/releases/download/v.*?/uq-v.*?-macos-x86_64\.tar\.gz"$',
        f'      url "https://github.com/yxxbc/uq/releases/download/v{version}/uq-v{version}-macos-x86_64.tar.gz"',
        text,
        count=1,
    )
    text = re.sub(
        r'(?m)^    assert_match "uq .*?", shell_output\("(?:#\{bin\}|#<built-in function bin>)/uq --version"\)$',
        f'    assert_match "uq {version}", shell_output("#{{bin}}/uq --version")',
        text,
        count=1,
    )

    if text == original:
        replaced = False
        for pat in (
            r'(?m)^  version ".*?"$',
            r'(?m)^\s*url ".*?uq-v.*?-macos-arm64\.tar\.gz"$',
            r'(?m)^\s*url ".*?uq-v.*?-macos-x86_64\.tar\.gz"$',
            r'(?m)^    assert_match "uq .*?", shell_output\(".*?/uq --version"\)$',
        ):
            if re.search(pat, text):
                replaced = True
                break
        if not replaced:
            raise RuntimeError("No formula fields were updated. Check Formula/uq.rb format.")

    FORMULA_PATH.write_text(text, encoding="utf-8")
    print(f"Synced Formula/uq.rb to version {version}")


def sync_installer(version: str) -> None:
    if not INSTALL_PATH.exists():
        raise FileNotFoundError(f"Installer not found: {INSTALL_PATH}")

    text = INSTALL_PATH.read_text(encoding="utf-8")
    pattern = r'(?m)^VERSION="v.*?"$'
    if not re.search(pattern, text):
        raise RuntimeError(f"No VERSION field found in {INSTALL_PATH}")

    updated = re.sub(pattern, f'VERSION="v{version}"', text, count=1)
    if updated != text:
        INSTALL_PATH.write_text(updated, encoding="utf-8")
    print(f"Synced install.sh to version {version}")


def fetch_hashes(version: str) -> None:
    text = FORMULA_PATH.read_text(encoding="utf-8")
    for arch in ARCHES:
        url = RELEASE_URL.format(version=version, arch=arch)
        with urllib.request.urlopen(url, timeout=120) as response:  # noqa: S310
            digest = hashlib.sha256(response.read()).hexdigest()
        pattern = rf'(uq-v[0-9.]+-macos-{arch}\.tar\.gz"\s*\n\s*sha256 ")[^"]*(")'
        text, replaced = re.subn(pattern, rf"\g<1>{digest}\g<2>", text)
        if replaced != 1:
            raise RuntimeError(f"Could not update the {arch} sha256 in {FORMULA_PATH}")
        print(f"Synced {arch} sha256: {digest}")
    FORMULA_PATH.write_text(text, encoding="utf-8")


if __name__ == "__main__":
    version = read_version()
    sync_formula(version)
    sync_installer(version)
    if "--fetch-hashes" in sys.argv[1:]:
        fetch_hashes(version)
