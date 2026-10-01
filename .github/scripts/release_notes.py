# Copyright (c) Mysten Labs, Inc.
# SPDX-License-Identifier: Apache-2.0

"""Extract the tagged release's section from Changes.md."""

import argparse
import re
from pathlib import Path


def extract_release_notes(changelog: str, tag: str) -> str:
    """Return one nonempty release section, including its heading."""
    if not tag.startswith("v") or not tag[1:] or tag[1:].lower() == "unreleased":
        raise ValueError(f"Expected a release tag such as v1.2.3, got {tag!r}")

    version = tag[1:]
    lines = changelog.splitlines()
    sections = [i for i, line in enumerate(lines) if re.match(r"^##(?:\s|$)", line)]
    release_heading = re.compile(
        rf"##[ \t]+{re.escape(version)}(?:[ \t]+-[ \t]+.*)?[ \t]*"
    )
    matches = [i for i in sections if release_heading.fullmatch(lines[i])]
    if len(matches) != 1:
        raise ValueError(
            f"Expected exactly one Changes.md section for {tag}, found {len(matches)}"
        )

    start = matches[0]
    end = next((i for i in sections if i > start), len(lines))
    if not "\n".join(lines[start + 1 : end]).strip():
        raise ValueError(f"Changes.md section for {tag} is empty")
    return "\n".join(lines[start:end]).strip() + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("tag")
    parser.add_argument("changelog", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    try:
        notes = extract_release_notes(args.changelog.read_text(encoding="utf-8"), args.tag)
        args.output.write_text(notes, encoding="utf-8")
    except (OSError, ValueError) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
