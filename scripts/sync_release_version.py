#!/usr/bin/env python3

from __future__ import annotations

import re
import sys
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib


ROOT = Path(__file__).resolve().parent.parent
CARGO_PATH = ROOT / "Cargo.toml"
FORMULA_PATH = ROOT / "Formula" / "uq.rb"


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
        r'(?m)^    url ".*?/download/v.*?/uq-v.*?-macos-arm64\.tar\.gz"$',
        f'    url "https://github.com/yxxbc/uq/releases/download/v{version}/uq-v{version}-macos-arm64.tar.gz"',
        text,
        count=1,
    )
    text = re.sub(
        r'(?m)^    assert_match "uq .*?", shell_output\("#\{bin\}/uq --version"\)$',
        f'    assert_match "uq {version}", shell_output("#{bin}/uq --version")',
        text,
        count=1,
    )

    if text == original:
        raise RuntimeError("No formula fields were updated. Check Formula/uq.rb format.")

    FORMULA_PATH.write_text(text, encoding="utf-8")
    print(f"Synced Formula/uq.rb to version {version}")


if __name__ == "__main__":
    version = read_version()
    sync_formula(version)
