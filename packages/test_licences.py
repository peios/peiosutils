"""Missing or undeclared licence inputs must stop package assembly."""

import hashlib
import json
import os
import tempfile
import unittest
from pathlib import Path

from licences import collect


class LicenceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.previous = Path.cwd()
        os.chdir(self.temp.name)
        self.addCleanup(os.chdir, self.previous)
        Path("Cargo.lock").write_text(
            '[[package]]\nname = "fixture"\nversion = "1.0.0"\n'
            'source = "registry+https://example.invalid/index"\n',
            encoding="utf-8",
        )
        root = Path("external/fixture")
        root.mkdir(parents=True)
        self.manifest = root / "Cargo.toml"
        self.manifest.write_text(
            '[package]\nname = "fixture"\nversion = "1.0.0"\nlicense = "MIT OR Apache-2.0"\n',
            encoding="utf-8",
        )
        self.notice = root / "LICENSE"
        self.notice.write_text("fixture licence", encoding="utf-8")

    def test_notices_and_normalised_expression(self) -> None:
        expression = collect([self.manifest, self.notice], Path("out"))
        self.assertEqual(expression, "(Apache-2.0 OR MIT) AND MIT")
        self.assertEqual(
            Path("out/fixture-1.0.0/LICENSE").read_text(encoding="utf-8"), "fixture licence"
        )
        self.assertIn("fixture\t1.0.0", Path("out/crate-licenses.tsv").read_text(encoding="utf-8"))

    def test_workspace_metadata_is_inherited(self) -> None:
        Path("Cargo.toml").write_text(
            '[workspace.package]\nversion = "1.0.0"\nlicense = "MIT"\n', encoding="utf-8"
        )
        Path("Cargo.lock").write_text(
            '[[package]]\nname = "fixture"\nversion = "1.0.0"\n', encoding="utf-8"
        )
        member = Path("fixture")
        member.mkdir()
        manifest = member / "Cargo.toml"
        manifest.write_text(
            '[package]\nname = "fixture"\nversion.workspace = true\nlicense.workspace = true\n',
            encoding="utf-8",
        )
        self.assertEqual(collect([manifest], Path("out")), "MIT")

    def test_undeclared_notice_is_not_read(self) -> None:
        with self.assertRaisesRegex(ValueError, "missing declared licence"):
            collect([self.manifest], Path("out"))

    def test_supplementary_notice_requires_exact_identity_and_hash(self) -> None:
        root = Path("third-party")
        root.mkdir()
        notice = root / "LICENSE"
        notice.write_text("recorded source notice", encoding="utf-8")
        record = {
            "name": "fixture",
            "version": "1.0.0",
            "source": "registry+https://example.invalid/index",
            "notices": [
                {"path": str(notice), "sha256": hashlib.sha256(notice.read_bytes()).hexdigest()}
            ],
        }
        manifest = root / "notices.json"
        manifest.write_text(json.dumps([record]), encoding="utf-8")
        inputs = [self.manifest, manifest, notice]
        collect(inputs, Path("good"))
        self.assertEqual((Path("good/fixture-1.0.0/LICENSE")).read_bytes(), notice.read_bytes())
        with self.assertRaisesRegex(ValueError, "undeclared supplementary"):
            collect(inputs[:-1], Path("undeclared"))
        notice.write_text("changed notice", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            collect(inputs, Path("changed"))
        record["version"] = "2.0.0"
        manifest.write_text(json.dumps([record]), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "missing declared licence"):
            collect(inputs, Path("wrong-version"))

    def test_unlocked_crate_is_rejected(self) -> None:
        Path("Cargo.lock").write_text("package = []\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "absent from Cargo.lock"):
            collect([self.manifest, self.notice], Path("out"))

    def test_missing_expression_is_rejected(self) -> None:
        self.manifest.write_text(
            '[package]\nname = "fixture"\nversion = "1.0.0"\n', encoding="utf-8"
        )
        with self.assertRaisesRegex(ValueError, "missing licence expression"):
            collect([self.manifest, self.notice], Path("out"))


if __name__ == "__main__":
    unittest.main()
