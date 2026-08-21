import unittest

from scripts.build_current import get_platform


class PlatformTests(unittest.TestCase):
    def test_linux_x86_64(self):
        self.assertEqual(get_platform("linux", "x86_64"), "linux-x64")

    def test_windows_x86_64(self):
        self.assertEqual(get_platform("win32", "AMD64"), "win32-x64")

    def test_macos_arm64(self):
        self.assertEqual(get_platform("darwin", "arm64"), "darwin-arm64")


if __name__ == "__main__":
    unittest.main()
