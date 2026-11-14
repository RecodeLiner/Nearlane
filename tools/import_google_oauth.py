#!/usr/bin/env python3
"""Import desktop OAuth configuration from a known official binary, locally only.

Does not run the binary, contact Google, read account tokens, or print credentials.
Only the exact previously examined Windows artifact is accepted. Output is confined
to ignored work/private/, with a private directory and an exclusive mode-0600 file.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re


ARTIFACT_SHA256 = "4364301891d44e7b1ee84685874f559cda405ccdfcb0637df5f7ec223cb806c4"
SCOPE = "https://www.googleapis.com/auth/nearbysharing-pa"


def extract(data):
    """Return only the application configuration of the examined artifact."""
    if len(data) != 47_606_936 or hashlib.sha256(data).hexdigest() != ARTIFACT_SHA256:
        raise ValueError("unsupported artifact; configuration was not imported")
    client_id = data[0x2568880:0x2568880 + 71].decode("ascii")
    client_secret = data[0x25688C8:0x25688C8 + 35].decode("ascii")
    if not re.fullmatch(r"[0-9]+-[a-z0-9]+\.apps\.googleusercontent\.com", client_id):
        raise ValueError("unexpected client ID format")
    if not re.fullmatch(r"GOCSPX-[A-Za-z0-9_-]+", client_secret):
        raise ValueError("unexpected desktop client configuration")
    return dict(client_id=client_id, client_secret=client_secret, scope=SCOPE,
                artifact_sha256=ARTIFACT_SHA256)


def save_private(root, config):
    """Write exclusively to the local work directory; never overwrite a file."""
    private = root / "work" / "private"
    if private.is_symlink() or private.parent.is_symlink():
        raise ValueError("private directory must not be a symlink")
    private.mkdir(mode=0o700, parents=True, exist_ok=True)
    private.chmod(0o700)
    output = private / "oauth-client.json"
    fd = os.open(output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w") as stream:
        json.dump(config, stream)
        stream.write("\n")
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=Path)
    args = parser.parse_args()
    # Check before reading: an arbitrary or huge file is not an accepted input.
    try:
        if args.artifact.stat().st_size != 47_606_936:
            parser.exit(1, "unsupported artifact size; nothing imported\n")
        config = extract(args.artifact.read_bytes())
        root = Path(__file__).resolve().parent.parent
        save_private(root, config)
    except (OSError, ValueError, UnicodeError):
        parser.exit(1, "import failed; check the artifact and whether local configuration already exists\n")
    print("Imported desktop application configuration to work/private/oauth-client.json (0600).")
    print("No account credentials were read. No network requests were made.")


if __name__ == "__main__":
    main()
