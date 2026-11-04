"""Executable publication must preserve old running images and failed updates."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import run_textflow_large_macos as launcher


class PublicationTests(unittest.TestCase):
    def test_open_old_image_survives_publication(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, destination = root / "source", root / "app"
            source.write_bytes(b"new executable")
            source.chmod(0o755)
            destination.write_bytes(b"old executable")
            old_inode = destination.stat().st_ino
            with destination.open("rb") as old_image:
                launcher.publish_executable(source, destination)
                self.assertEqual(old_image.read(), b"old executable")
            self.assertEqual(destination.read_bytes(), b"new executable")
            self.assertNotEqual(destination.stat().st_ino, old_inode)
            self.assertEqual(destination.stat().st_mode & 0o777, 0o755)
            self.assertEqual({p.name for p in root.iterdir()}, {"source", "app"})

    def test_failed_copy_preserves_published_image(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, destination = root / "source", root / "app"
            source.write_bytes(b"new executable")
            destination.write_bytes(b"old executable")
            old_inode = destination.stat().st_ino
            with patch.object(launcher.shutil, "copy2", side_effect=OSError("copy failed")):
                with self.assertRaisesRegex(OSError, "copy failed"):
                    launcher.publish_executable(source, destination)
            self.assertEqual(destination.read_bytes(), b"old executable")
            self.assertEqual(destination.stat().st_ino, old_inode)
            self.assertEqual({p.name for p in root.iterdir()}, {"source", "app"})


if __name__ == "__main__":
    unittest.main()
