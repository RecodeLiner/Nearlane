#!/usr/bin/env python3
"""Import desktop OAuth configuration from a known official binary, locally only.

Does not run the binary, contact Google, read account tokens, or print credentials.
Only the exact previously examined Windows artifact is accepted. Output defaults
to the runtime application configuration directory, never the package/Nix store.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import sys
import tempfile


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


def default_output(environ=None, platform=None):
    """Resolve the same absolute runtime location as nearlane-platform."""
    env = os.environ if environ is None else environ
    platform = sys.platform if platform is None else platform
    def absolute(value):
        return Path(value) if value and Path(value).is_absolute() else None
    if "NEARLANE_OAUTH_CONFIG" in env:
        path = absolute(env["NEARLANE_OAUTH_CONFIG"])
        if path is None:
            raise ValueError("NEARLANE_OAUTH_CONFIG must be an absolute file path")
        return path
    home = absolute(env.get("HOME"))
    if platform == "darwin":
        base = home / "Library" / "Application Support" / "Nearlane" if home else None
    elif platform == "win32":
        appdata = absolute(env.get("APPDATA"))
        base = appdata / "Nearlane" if appdata else None
    else:
        config = absolute(env.get("XDG_CONFIG_HOME")) or (home / ".config" if home else None)
        base = config / "nearlane" if config else None
    if base is None:
        raise ValueError("runtime configuration directory is unavailable")
    return base / "oauth-client.json"


def save_private(output, config):
    """Atomically publish a private runtime file; never overwrite an existing one."""
    output = Path(output)
    if not output.is_absolute() or output.is_relative_to("/nix/store"):
        raise ValueError("configuration must stay outside the Nix store")
    private = output.parent
    if private.is_symlink() or (private.exists() and not private.is_dir()):
        raise ValueError("configuration directory must not be a symlink")
    new_directory = not private.exists()
    private.mkdir(mode=0o700, parents=True, exist_ok=True)
    if private.resolve().is_relative_to("/nix/store"):
        raise ValueError("configuration directory resolves into the Nix store")
    if new_directory or private.name in ("nearlane", "Nearlane"):
        private.chmod(0o700)
    if os.name == "posix" and private.stat().st_mode & 0o022:
        raise ValueError("configuration directory is writable by another account")
    fd, name = tempfile.mkstemp(prefix=".oauth-client-", dir=private)
    temporary = Path(name)
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as stream:
            json.dump(config, stream)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
        # Same-directory hard-link publication is exclusive even under a race.
        os.link(temporary, output)
    finally:
        temporary.unlink(missing_ok=True)
    return output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("artifact", type=Path)
    parser.add_argument("--output", type=Path, help="absolute runtime file path (default: application config directory)")
    args = parser.parse_args()
    # Check before reading: an arbitrary or huge file is not an accepted input.
    try:
        if args.artifact.stat().st_size != 47_606_936:
            parser.exit(1, "unsupported artifact size; nothing imported\n")
        config = extract(args.artifact.read_bytes())
        output = save_private(args.output if args.output is not None else default_output(), config)
    except (OSError, ValueError, UnicodeError):
        parser.exit(1, "import failed; check the artifact and whether local configuration already exists\n")
    print(f"Imported desktop application configuration to {output} (private file).")
    print("No account credentials were read. No network requests were made.")


if __name__ == "__main__":
    main()
