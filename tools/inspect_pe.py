#!/usr/bin/env python3
"""Offline, bounded evidence extraction. Emits whitelisted indicators, not strings dumps.

Original implementation: MIT OR Apache-2.0. Does not execute the target or recover
client secrets. Input artifacts stay outside the source tree under ignored work/.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

INDICATORS = {
    "authorization_endpoint": b"https://accounts.google.com/o/oauth2/auth",
    "token_endpoint": b"https://oauth2.googleapis.com/token",
    "nearby_scope": b"https://www.googleapis.com/auth/nearbysharing-pa",
    "nearby_sdk_scope": b"https://www.googleapis.com/auth/nearbysharingsdk",
    "nearby_api": b"nearbysharing-pa.googleapis.com",
    "loopback_redirect": b"http://127.0.0.1:",
    "pkce_challenge": b"code_challenge_method",
    "pkce_verifier": b"code_verifier",
    "oauth_config_pkce_field": b"use_pkce",
    "authorization_code_grant": b"authorization_code",
    "refresh_grant": b"grant_type=refresh_token",
    "third_party_config": b"LoadUserAccount updates 1P Oauth config with 3P client id.",
}


def inspect(path):
    if path.stat().st_size > 256 * 1024 * 1024:
        raise ValueError("artifact exceeds the 256 MiB analysis limit")
    data = path.read_bytes()
    if len(data) < 64 or data[:2] != b"MZ":
        raise ValueError("not a PE file")
    pe = struct.unpack_from("<I", data, 0x3C)[0]
    if pe + 24 > len(data) or data[pe:pe+4] != b"PE\0\0":
        raise ValueError("invalid PE header")
    machine, count, timestamp = struct.unpack_from("<HHI", data, pe + 4)
    optional_size = struct.unpack_from("<H", data, pe + 20)[0]
    optional = pe + 24
    if optional + optional_size + count * 40 > len(data) or optional_size < 32:
        raise ValueError("truncated PE headers")
    magic = struct.unpack_from("<H", data, optional)[0]
    if magic == 0x20B:
        image_base = struct.unpack_from("<Q", data, optional + 24)[0]
    elif magic == 0x10B:
        image_base = struct.unpack_from("<I", data, optional + 28)[0]
    else:
        raise ValueError("unsupported PE optional header")
    sections = []
    for index in range(count):
        start = optional + optional_size + index * 40
        name = data[start:start+8].rstrip(b"\0").decode("ascii", "replace")
        vsize, rva, size, raw = struct.unpack_from("<IIII", data, start + 8)
        if raw + size > len(data):
            raise ValueError("section exceeds artifact size")
        sections.append({"name": name, "rva": rva, "virtual_size": vsize, "raw_offset": raw, "raw_size": size})

    def locate(offset):
        for section in sections:
            if section["raw_offset"] <= offset < section["raw_offset"] + section["raw_size"]:
                return hex(image_base + section["rva"] + offset - section["raw_offset"])
        return None

    indicators = {}
    for name, needle in INDICATORS.items():
        matches = []
        offset = 0
        while len(matches) < 32:
            offset = data.find(needle, offset)
            if offset < 0:
                break
            matches.append({"file_offset": hex(offset), "virtual_address": locate(offset)})
            offset += len(needle)
        indicators[name] = matches
    return {"artifact": path.name, "sha256": hashlib.sha256(data).hexdigest(), "size": len(data),
            "machine": hex(machine), "pe_timestamp": timestamp, "image_base": hex(image_base),
            "sections": sections, "indicators": indicators,
            "interpretation": "Static indicators only; execution, PKCE activation and third-party authorization are NOT verified."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        report = json.dumps(inspect(args.artifact), indent=2) + "\n"
    except (OSError, ValueError, struct.error) as error:
        parser.exit(1, f"analysis failed: {error}\n")
    if args.output:
        args.output.write_text(report)
    else:
        print(report, end="")


if __name__ == "__main__":
    main()
