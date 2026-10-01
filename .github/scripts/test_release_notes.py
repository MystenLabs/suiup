# Copyright (c) Mysten Labs, Inc.
# SPDX-License-Identifier: Apache-2.0

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from release_notes import extract_release_notes


CHANGELOG = """## Unreleased

- Work for a future release.

## 1.2.3 - 2026-07-08

### Fixed

- Current fix.
- Another fix.

## 1.2.2 - 2026-07-01

- Previous fix.
"""


class ReleaseNotesTests(unittest.TestCase):
    def test_extracts_only_the_tagged_release(self):
        self.assertEqual(
            extract_release_notes(CHANGELOG, "v1.2.3"),
            "## 1.2.3 - 2026-07-08\n\n### Fixed\n\n- Current fix.\n- Another fix.\n",
        )

    def test_can_extract_an_older_tag_at_end_of_file(self):
        self.assertEqual(
            extract_release_notes(CHANGELOG.rstrip(), "v1.2.2"),
            "## 1.2.2 - 2026-07-01\n\n- Previous fix.\n",
        )

    def test_normalizes_windows_line_endings(self):
        self.assertEqual(
            extract_release_notes(CHANGELOG.replace("\n", "\r\n"), "v1.2.3"),
            extract_release_notes(CHANGELOG, "v1.2.3"),
        )

    def test_missing_tag_does_not_fall_back_to_unreleased_or_another_version(self):
        for tag in ["v1.2.4", "v1.2", "v1.2.30"]:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                extract_release_notes(CHANGELOG, tag)

    def test_rejects_empty_and_duplicate_sections(self):
        for changelog in [
            "## 1.2.3 - 2026-07-08\n\n## 1.2.2\n\n- Older fix.\n",
            "## 1.2.3\n  \n",
            CHANGELOG + "\n## 1.2.3\n\n- Duplicate.\n",
        ]:
            with self.subTest(changelog=changelog), self.assertRaises(ValueError):
                extract_release_notes(changelog, "v1.2.3")

    def test_rejects_non_release_refs(self):
        for tag in ["main", "1.2.3", "v", "vUnreleased"]:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                extract_release_notes(CHANGELOG, tag)

    def test_cli_writes_notes_and_fails_without_output_for_missing_version(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            changelog = root / "Changes.md"
            changelog.write_text(CHANGELOG, encoding="utf-8")
            output = root / "notes.md"
            script = Path(__file__).with_name("release_notes.py")
            result = subprocess.run(
                [sys.executable, "-B", str(script), "v1.2.3", str(changelog), str(output)],
                capture_output=True,
                text=True,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                output.read_text(encoding="utf-8"),
                extract_release_notes(CHANGELOG, "v1.2.3"),
            )
            output.unlink()
            result = subprocess.run(
                [sys.executable, "-B", str(script), "v9.9.9", str(changelog), str(output)],
                capture_output=True,
                text=True,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
