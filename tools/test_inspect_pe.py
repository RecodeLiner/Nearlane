"""Boundary tests for the offline parser, using original synthetic bytes only."""
import struct
import tempfile
import unittest
from pathlib import Path

from inspect_pe import inspect


class PeTest(unittest.TestCase):
    def inspect_bytes(self, data):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "synthetic.exe"
            path.write_bytes(data)
            return inspect(path)

    @staticmethod
    def fixture():
        data = bytearray(1024)
        data[:2] = b"MZ"
        struct.pack_into("<I", data, 0x3C, 64)
        data[64:68] = b"PE\0\0"
        struct.pack_into("<HHI", data, 68, 0x8664, 1, 0)
        struct.pack_into("<H", data, 84, 240)
        struct.pack_into("<H", data, 88, 0x20B)
        struct.pack_into("<Q", data, 112, 0x140000000)
        data[328:336] = b".rdata\0\0"
        struct.pack_into("<IIII", data, 336, 512, 0x1000, 512, 512)
        needle = b"https://oauth2.googleapis.com/token"
        data[600:600+len(needle)] = needle
        return data

    def test_maps_offsets_to_virtual_addresses(self):
        result = self.inspect_bytes(self.fixture())
        location = result["indicators"]["token_endpoint"][0]
        self.assertEqual(location, {"file_offset": "0x258", "virtual_address": "0x140001058"})

    def test_does_not_dump_arbitrary_strings(self):
        data = self.fixture()
        data[800:816] = b"PRIVATE_PASSWORD"
        self.assertNotIn("PRIVATE_PASSWORD", str(self.inspect_bytes(data)))

    def test_rejects_truncated_and_invalid_headers(self):
        for data in [b"MZ", b"not a PE", self.fixture()[:80], self.fixture()[:500]]:
            with self.subTest(length=len(data)):
                with self.assertRaises(ValueError):
                    self.inspect_bytes(data)

    def test_rejects_sections_outside_file(self):
        data = self.fixture()
        struct.pack_into("<I", data, 348, 99999)
        with self.assertRaises(ValueError):
            self.inspect_bytes(data)


if __name__ == "__main__":
    unittest.main()
