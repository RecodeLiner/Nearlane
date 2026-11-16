"""Synthetic-only tests: no proprietary binary or real client configuration."""

import os
from pathlib import Path
import tempfile
import unittest

from import_google_oauth import default_output, extract, save_private


class PrivateImportTests(unittest.TestCase):
    def test_unknown_artifact_is_not_interpreted_as_configuration(self):
        with self.assertRaises(ValueError):
            extract(b"MZ synthetic artifact")

    def test_private_modes_no_overwrite_and_atomic_publication(self):
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "nearlane" / "oauth-client.json"
            path = save_private(destination, {"client_secret": "synthetic-first"})
            if os.name == "posix":
                self.assertEqual(path.stat().st_mode & 0o777, 0o600)
                self.assertEqual(path.parent.stat().st_mode & 0o777, 0o700)
            with self.assertRaises(FileExistsError):
                save_private(destination, {"client_secret": "synthetic-second"})
            self.assertIn("synthetic-first", path.read_text())
            self.assertNotIn("synthetic-second", path.read_text())
            self.assertEqual(list(path.parent.iterdir()), [path])

    @unittest.skipUnless(os.name == "posix", "symlink fixture needs native permissions")
    def test_symlinked_directory_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "other"
            target.mkdir()
            (root / "nearlane").symlink_to(target, target_is_directory=True)
            with self.assertRaises(ValueError):
                save_private(root / "nearlane" / "oauth-client.json", {"client_secret": "synthetic"})
            self.assertEqual(list(target.iterdir()), [])

    def test_runtime_paths_and_explicit_absolute_override(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            env = {"HOME": str(root), "APPDATA": str(root / "roaming")}
            self.assertEqual(default_output(env, "linux"), root / ".config/nearlane/oauth-client.json")
            self.assertEqual(default_output(env, "darwin"), root / "Library/Application Support/Nearlane/oauth-client.json")
            self.assertEqual(default_output(env, "win32"), root / "roaming/Nearlane/oauth-client.json")
            env["XDG_CONFIG_HOME"] = str(root / "custom")
            self.assertEqual(default_output(env, "linux"), root / "custom/nearlane/oauth-client.json")
            env["XDG_CONFIG_HOME"] = "relative"
            self.assertEqual(default_output(env, "linux"), root / ".config/nearlane/oauth-client.json")
            env["NEARLANE_OAUTH_CONFIG"] = str(root / "explicit.json")
            self.assertEqual(default_output(env, "linux"), root / "explicit.json")
            for bad in ("", "relative.json"):
                env["NEARLANE_OAUTH_CONFIG"] = bad
                with self.assertRaises(ValueError):
                    default_output(env, "linux")

    def test_relative_output_and_store_are_rejected_before_writing(self):
        for path in (Path("relative/oauth-client.json"), Path("/nix/store/nearlane-test/oauth-client.json")):
            if path.is_absolute() or os.name != "nt":
                with self.assertRaises(ValueError):
                    save_private(path, {"client_secret": "synthetic"})


if __name__ == "__main__":
    unittest.main()
