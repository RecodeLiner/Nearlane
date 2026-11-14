#!/usr/bin/env python3
"""Check QML loading and private-mode gating, with Qt diagnostics on stderr."""

import argparse
import os
from pathlib import Path
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--kde", action="store_true", help="also check the KDE desktop style")
    parser.add_argument("--native", action="store_true", help="briefly show a window on the desktop")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    for style in [None, "org.kde.desktop"] if args.kde else [None]:
        env = os.environ.copy()
        env.update(
            NEARLANE_SMOKE_TEST="1",
            QT_FORCE_STDERR_LOGGING="1",
            QT_LOGGING_RULES="*.warning=true",
            QT_QUICK_BACKEND="software",
        )
        if not args.native:
            env["QT_QPA_PLATFORM"] = "offscreen"
        if style:
            env.update(QT_QUICK_CONTROLS_STYLE=style, QT_QPA_PLATFORMTHEME="kde")
        try:
            result = subprocess.run(
                [str(binary)], env=env, capture_output=True, text=True, timeout=15
            )
        except subprocess.TimeoutExpired:
            print(f"FAIL ({style or 'session style'}): startup timed out", file=sys.stderr)
            return 1
        output = result.stdout + result.stderr
        errors = ("qmlRegisterType requires absolute URLs", "failed to load component", "unavailable")
        if (
            result.returncode != 0
            or "smoke: QML loaded" not in output
            or "smoke: policy checked; device remains hidden" not in output
            or any(error in output for error in errors)
        ):
            print(f"FAIL ({style or 'session style'}): exit {result.returncode}", file=sys.stderr)
            print(output, file=sys.stderr)
            return 1
        print(f"PASS ({style or 'session style'}): QML loaded, private-mode check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
