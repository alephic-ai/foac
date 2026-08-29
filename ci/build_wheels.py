#!/usr/bin/env python3
"""Repackage the release binaries as PyPI wheels, so `uvx foac` runs foac.

A wheel is a zip, and installers copy `<name>-<version>.data/scripts/*` onto
PATH, so a wheel carrying nothing but the binary publishes foac to PyPI with no
Python in it. Linux ships the static musl binary under both the manylinux and
the musllinux tag: it links no libc, so it honours either promise, and one
build covers glibc and musl distros alike.

Run from the repo root, over the archives of a GitHub release:

    python3 ci/build_wheels.py --version 2.21.2 --artifacts artifacts --out dist
"""

import argparse
import base64
import hashlib
import re
import tarfile
import zipfile
from pathlib import Path

REPO = "https://github.com/alephic-ai/foac"

# Release target -> the platform tags its binary satisfies. The macOS versions
# are the deployment targets rustc defaults to for those targets.
TARGETS = {
    "aarch64-apple-darwin": ["macosx_11_0_arm64"],
    "x86_64-apple-darwin": ["macosx_10_12_x86_64"],
    "aarch64-unknown-linux-musl": ["manylinux_2_17_aarch64", "musllinux_1_2_aarch64"],
    "x86_64-unknown-linux-musl": ["manylinux_2_17_x86_64", "musllinux_1_2_x86_64"],
    "aarch64-pc-windows-msvc": ["win_arm64"],
    "x86_64-pc-windows-msvc": ["win_amd64"],
}


def binary(artifacts: Path, target: str) -> tuple[str, bytes]:
    """The foac executable inside that target's release archive."""
    if "windows" in target:
        with zipfile.ZipFile(artifacts / f"foac-{target}.zip") as archive:
            return "foac.exe", archive.read("foac.exe")
    with tarfile.open(artifacts / f"foac-{target}.tar.gz") as archive:
        return "foac", archive.extractfile("foac").read()


def metadata(version: str, summary: str) -> str:
    return f"""Metadata-Version: 2.1
Name: foac
Version: {version}
Summary: {summary}
License: GPL-3.0-or-later
Classifier: License :: OSI Approved :: GNU General Public License v3 or later (GPLv3+)
Project-URL: Homepage, {REPO}
Project-URL: Source, {REPO}
Description-Content-Type: text/markdown

# foac

One CLI for every SaaS provider your coding agents touch. This package ships
the release binary, nothing else: `uvx foac --help`, or `uv tool install foac`.

Docs, provider list and source: <{REPO}>
"""


def add(wheel: zipfile.ZipFile, name: str, data: bytes, mode: int) -> str:
    """Add one file, and return its RECORD line."""
    entry = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
    # The mode has to be spelled out: scripts land on PATH and must be
    # executable, and a ZipInfo built by hand carries no mode at all.
    entry.external_attr = (0o100000 | mode) << 16
    entry.create_system = 3  # Unix, so the mode above is honoured
    wheel.writestr(entry, data, zipfile.ZIP_DEFLATED)
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=")
    return f"{name},sha256={digest.decode()},{len(data)}\n"


def build(out: Path, version: str, summary: str, tags: list[str], executable: tuple[str, bytes]) -> Path:
    name, blob = executable
    dist_info = f"foac-{version}.dist-info"
    wheel_tags = "".join(f"Tag: py3-none-{tag}\n" for tag in tags)
    path = out / f"foac-{version}-py3-none-{'.'.join(tags)}.whl"
    with zipfile.ZipFile(path, "w") as wheel:
        record = "".join(
            [
                add(wheel, f"foac-{version}.data/scripts/{name}", blob, 0o755),
                add(wheel, f"{dist_info}/METADATA", metadata(version, summary).encode(), 0o644),
                add(
                    wheel,
                    f"{dist_info}/WHEEL",
                    f"Wheel-Version: 1.0\nGenerator: foac ci/build_wheels.py\n"
                    f"Root-Is-Purelib: false\n{wheel_tags}".encode(),
                    0o644,
                ),
            ]
        )
        add(wheel, f"{dist_info}/RECORD", f"{record}{dist_info}/RECORD,,\n".encode(), 0o644)
    return path


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True, help="release version, without the v")
    parser.add_argument("--artifacts", type=Path, required=True, help="directory of release archives")
    parser.add_argument("--out", type=Path, required=True, help="directory to write the wheels to")
    args = parser.parse_args()

    # The crate description is the summary PyPI shows; keep the two in step.
    cargo = Path("Cargo.toml").read_text()
    summary = re.search(r'^description = "(.*)"$', cargo, re.MULTILINE).group(1)

    args.out.mkdir(parents=True, exist_ok=True)
    for target, tags in TARGETS.items():
        path = build(args.out, args.version, summary, tags, binary(args.artifacts, target))
        print(path)


if __name__ == "__main__":
    main()
