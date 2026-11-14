"""Synthetic-only tests: no proprietary binary or real client configuration."""

from pathlib import Path
import tempfile
import unittest

from import_google_oauth import extract, save_private


class PrivateImportTests(unittest.TestCase):
    def test_unknown_artifact_is_not_interpreted_as_configuration(self):
        with self.assertRaises(ValueError):
            extract(b"MZ synthetic artifact")

    def test_private_modes_and_no_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            path = save_private(Path(directory), {"client_secret": "synthetic-first"})
            if hasattr(path.stat(), "st_mode"):
                self.assertEqual(path.stat().st_mode & 0o777, 0o600)
                self.assertEqual(path.parent.stat().st_mode & 0o777, 0o700)
            with self.assertRaises(FileExistsError):
                save_private(Path(directory), {"client_secret": "synthetic-second"})
            self.assertIn("synthetic-first", path.read_text())
            self.assertNotIn("synthetic-second", path.read_text())

    def test_symlinked_work_directory_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "other"
            target.mkdir()
            (root / "work").symlink_to(target, target_is_directory=True)
            with self.assertRaises(ValueError):
                save_private(root, {"client_secret": "synthetic"})
            self.assertFalse((target / "private").exists())


if __name__ == "__main__":
    unittest.main()
