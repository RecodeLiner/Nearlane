#!/usr/bin/env python3
"""Check QML loading and private-mode gating, with Qt diagnostics on stderr."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

from import_google_oauth import default_output


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--kde", action="store_true", help="also check the KDE desktop style")
    parser.add_argument("--native", action="store_true", help="briefly show a window on the desktop")
    parser.add_argument("--tray", action="store_true", help="check native tray close/show and request a test notification")
    parser.add_argument("--config-import", action="store_true", help="check local JSON import and a second launch after deleting the source")
    args = parser.parse_args()
    if args.tray and not args.native:
        parser.error("--tray requires --native")
    binary = args.binary.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="nearlane-smoke-") as directory:
        config = Path(directory) / "synthetic-client.json"
        config.touch(mode=0o600)
        config.write_text(json.dumps({"client_id": "synthetic-smoke-client"}))
        return check(binary, args, config, Path(directory) / "settings")


def check(binary, args, config, settings):
    styles = [None, "org.kde.desktop"] if args.kde else ([None, None] if args.config_import else [None])
    for index, style in enumerate(styles):
        env = os.environ.copy()
        env.update(
            NEARLANE_SMOKE_TEST="1",
            NEARLANE_OAUTH_CONFIG=str(config),
            QT_FORCE_STDERR_LOGGING="1",
            QT_LOGGING_RULES="*.warning=true",
            QT_QUICK_BACKEND="software",
            NEARLANE_PREFERENCES_SMOKE_DIR=str(settings),
            NEARLANE_PREFERENCES_SMOKE_TEST="write" if index == 0 else "read",
        )
        if args.config_import:
            env.pop("NEARLANE_OAUTH_CONFIG", None)
            env.update(
                XDG_CONFIG_HOME=str(config.parent / "runtime-config"),
                HOME=str(config.parent / "home"),
                APPDATA=str(config.parent / "appdata"),
                NEARLANE_CONFIG_SMOKE_PHASE="write" if index == 0 else "read",
                NEARLANE_CONFIG_SMOKE_SOURCE=str(config),
            )
            Path(env["HOME"]).mkdir(exist_ok=True)
            if index == 1:
                config.unlink()
            cwd = config.parent / f"launch-{index}"
            cwd.mkdir(exist_ok=True)
        else:
            env.pop("NEARLANE_CONFIG_SMOKE_PHASE", None)
            env.pop("NEARLANE_CONFIG_SMOKE_SOURCE", None)
            cwd = None
        if not args.native:
            env["QT_QPA_PLATFORM"] = "offscreen"
        if args.tray:
            env["NEARLANE_TRAY_SMOKE_TEST"] = "1"
        else:
            env.pop("NEARLANE_TRAY_SMOKE_TEST", None)
        if style:
            env.update(QT_QUICK_CONTROLS_STYLE=style, QT_QPA_PLATFORMTHEME="kde")
        try:
            result = subprocess.run(
                [str(binary)], env=env, cwd=cwd, capture_output=True, text=True, timeout=15
            )
        except subprocess.TimeoutExpired:
            print(f"FAIL ({style or 'session style'}): startup timed out", file=sys.stderr)
            return 1
        output = result.stdout + result.stderr
        errors = ("qmlRegisterType requires absolute URLs", "failed to load component", "unavailable")
        if (
            result.returncode != 0
            or "smoke: QML loaded" not in output
            or "smoke: application identity and window icon checked" not in output
            or "smoke: live translations checked" not in output
            or "smoke: language preference persisted; synthetic settings only" not in output
            or "smoke: policy checked; device remains hidden" not in output
            or "smoke: local sign-in cancelled; device remains hidden" not in output
            or "smoke: signed out; device remains hidden" not in output
            or "smoke: receiving preferences persisted; synthetic settings only" not in output
            or (args.config_import and "smoke: persistent configuration loaded; synthetic settings only" not in output)
            or (args.tray and "smoke: tray close/show and notification checked" not in output)
            or any(error in output for error in errors)
        ):
            print(f"FAIL ({style or 'session style'}): exit {result.returncode}", file=sys.stderr)
            print(output, file=sys.stderr)
            return 1
        print(f"PASS ({style or 'session style'}): QML loaded, local login cancelled, device hidden")
        print("PASS: private receiving defaults and saved settings (isolated from personal preferences)")
        if args.config_import:
            destination = default_output(env)
            saved = json.loads(destination.read_text())
            if saved.get("client_id") != "synthetic-smoke-client":
                raise RuntimeError("Synthetic configuration was not persisted")
            if os.name == "posix" and destination.stat().st_mode & 0o777 != 0o600:
                raise RuntimeError("Configuration permissions are not private")
            print("PASS: configuration import/restart, independent working directories and removed source")
        if args.tray:
            print("PASS: native tray available, close hides, show restores, notification requested")
    return 0


if __name__ == "__main__":
    sys.exit(main())
