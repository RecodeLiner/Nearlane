#!/usr/bin/env python3
"""Render the real Qt screens with isolated, explicitly synthetic UI states."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


CASES = {
    "send-light": dict(width=1040, height=760, dark=False, page=0, scene="ready"),
    "send-dark": dict(width=1040, height=760, dark=True, page=0, scene="ready"),
    "send-selected": dict(width=1040, height=760, dark=True, page=0, scene="ready", fileName="nearlane-ui-test.txt", devices=["Тестовое устройство", "Ещё одно тестовое устройство"]),
    "navigation-focus": dict(width=1040, height=760, dark=False, page=0, scene="ready", devices=["Тестовое устройство"], exerciseNavigation=True),
    "send-compact": dict(width=540, height=760, dark=False, page=0, scene="ready"),
    "send-large-text": dict(width=540, height=900, dark=True, page=0, scene="ready", scale=2, devices=["Очень длинное название тестового устройства для проверки переноса и доступности интерфейса"]),
    "send-large-text-bottom": dict(width=540, height=900, dark=True, page=0, scene="ready", scale=2, devices=["Очень длинное название тестового устройства для проверки переноса и доступности интерфейса"], scrollBottom=True),
    "receive-light": dict(width=1040, height=760, dark=False, page=1, scene="ready"),
    "receive-incoming": dict(width=1040, height=760, dark=True, page=1, scene="incoming", offer="Тестовое устройство\nnearlane-ui-test.txt\n94 bytes"),
    "settings-light": dict(width=1040, height=900, dark=False, page=2, scene="ready"),
    "settings-compact": dict(width=540, height=760, dark=True, page=2, scene="ready"),
    "settings-bottom": dict(width=540, height=760, dark=False, page=2, scene="ready", scrollBottom=True),
    "empty": dict(width=540, height=760, dark=False, page=0, scene="empty"),
    "loading": dict(width=1040, height=760, dark=True, page=0, scene="loading"),
    "signed-out": dict(width=1040, height=760, dark=False, page=2, scene="signed-out", scrollBottom=True),
    "error": dict(width=540, height=760, dark=False, page=2, scene="error"),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--only", nargs="+", choices=CASES)
    parser.add_argument("--native", action="store_true")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="nearlane-design-") as directory:
        config = Path(directory) / "synthetic.json"
        config.touch(mode=0o600)
        config.write_text(json.dumps({"client_id": "synthetic-design-preview"}))
        for name in args.only or CASES:
            path = output / f"{name}.png"
            path.unlink(missing_ok=True)
            settings = dict(CASES[name], output=str(path))
            env = os.environ.copy()
            env.update(
                NEARLANE_SMOKE_TEST="1",
                NEARLANE_DESIGN_PREVIEW=json.dumps(settings),
                NEARLANE_OAUTH_CONFIG=str(config),
                NEARLANE_PREFERENCES_SMOKE_DIR=directory,
                QT_FORCE_STDERR_LOGGING="1",
                QT_LOGGING_RULES="*.warning=true",
                QT_QUICK_BACKEND="software",
                QT_SCALE_FACTOR="1",
            )
            env.pop("NEARLANE_TRAY_SMOKE_TEST", None)
            env.pop("NEARLANE_PREFERENCES_SMOKE_TEST", None)
            if not args.native:
                env["QT_QPA_PLATFORM"] = "offscreen"
            try:
                result = subprocess.run([str(binary)], env=env, capture_output=True, text=True, timeout=20)
            except subprocess.TimeoutExpired as error:
                log = (error.stdout or b"") + (error.stderr or b"")
                (output / f"{name}.log").write_bytes(log)
                raise RuntimeError(f"{name} preview timed out; see {output / f'{name}.log'}") from error
            log = result.stdout + result.stderr
            (output / f"{name}.log").write_text(log)
            errors = ("ReferenceError", "TypeError", "Binding loop", "recursive rearrange", "Unable to assign", "failed to load", "Cannot assign", "is not a type", "no signal of the target matches")
            if result.returncode or not path.is_file() or "preview: saved synthetic UI" not in log or any(error in log for error in errors):
                raise RuntimeError(f"{name} preview failed:\n{log}")
            if settings.get("exerciseNavigation") and "preview: navigation, resize continuity and Tab focus checked" not in log:
                raise RuntimeError("Navigation and focus checks did not complete")
            print(f"PASS: {name} ({settings['width']} × {settings['height']})")


if __name__ == "__main__":
    main()
