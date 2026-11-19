#!/usr/bin/env python3
"""Update/check TS catalogs for Slint and deferred Rust presentation messages.

Slint uses I18n.tr(...). Rust uses Text::new/with_args or tr with
static English sources; no account name/path is ever interpreted as a source.
Run without flags to add new unfinished entries and remove unused ones.
"""
import argparse
import ast
from collections import Counter
from pathlib import Path
import re
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
STRING = r'"(?:[^"\\]|\\.)*"'


def decode(value):
    return ast.literal_eval(value)


def block(text, opening):
    """Balance braces while ignoring Rust strings and line comments."""
    depth = 0
    tokens = re.finditer(STRING + r'|//[^\n]*|[{}]', text[opening:])
    for token in tokens:
        if token[0] == '{': depth += 1
        elif token[0] == '}':
            depth -= 1
            if not depth: return text[opening:opening + token.end()]
    raise ValueError("Unbalanced message expression")


def sources():
    found = {"Ui": set(), "Backend": set()}
    for path in (ROOT / "apps/slint/ui").rglob("*.slint"):
        found["Ui"].update(decode(m[1]) for m in re.finditer(r'I18n\.tr\(\s*(' + STRING + ')', path.read_text()))
    for path in (ROOT / "crates").rglob("*.rs"):
        if path == ROOT / "crates/core/src/text.rs": continue
        text = path.read_text()
        found["Backend"].update(decode(m[1]) for m in re.finditer(r'\b(?:Text::(?:new|with_args)|tr)\(\s*(' + STRING + ')', text))
        for match in re.finditer(r'Text::new\((?:match|if)\b', text):
            expression = block(text, text.index('{', match.end()))
            # if/else has a second brace pair.
            if text[match.end()-2:match.end()] == 'if':
                end = text.index(expression, match.end()) + len(expression)
                tail = text[end:]
                if tail.lstrip().startswith('else'):
                    expression += block(text, text.index('{', end))
            found["Backend"].update(decode(m[0]) for m in re.finditer(STRING, expression))
        if path.parent == ROOT / "crates/service/src":
            for match in re.finditer('(' + STRING + r')\s*\.into\(\)', text):
                value = decode(match[1])
                if ' ' in value: found["Backend"].add(value)
    return found


def read(path):
    if not path.exists(): return {}
    return {(c.findtext("name"), m.findtext("source")): m.find("translation")
            for c in ET.parse(path).getroot().findall("context") for m in c.findall("message")}


def validate(path, expected):
    entries = read(path)
    keys = {(c, s) for c, strings in expected.items() for s in strings}
    errors = []
    for key in sorted(keys):
        translation = entries.get(key)
        if translation is None or translation.get("type") in ("unfinished", "obsolete", "vanished") or not translation.text:
            errors.append(f"Missing translation: {key}")
        elif Counter(re.findall(r'%[1-9][0-9]*', key[1])) != Counter(re.findall(r'%[1-9][0-9]*', translation.text)):
            errors.append(f"Changed placeholders: {key}")
    if set(entries) != keys: errors.append("Catalog keys differ from current sources; run tools/update_translations.py")
    if errors: raise ValueError(f"{path.relative_to(ROOT)}:\n" + '\n'.join(errors))
    print(f"PASS: {path.name}: {len(keys)} translated messages, placeholders intact")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    expected = sources()
    for language in ("en", "ru"):
        path = ROOT / f"crates/application/i18n/nearlane_{language}.ts"
        if args.check:
            validate(path, expected)
            continue
        old = read(path)
        root = ET.Element("TS", version="2.1", language=language, sourcelanguage="en")
        for context, strings in sorted(expected.items()):
            node = ET.SubElement(root, "context")
            ET.SubElement(node, "name").text = context
            for source in sorted(strings):
                message = ET.SubElement(node, "message")
                ET.SubElement(message, "source").text = source
                translation = old.get((context, source))
                if translation is not None: message.append(translation)
                else:
                    translation = ET.SubElement(message, "translation")
                    if language == "en": translation.text = source
                    else: translation.set("type", "unfinished")
        ET.indent(root, space="  ")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text('<?xml version="1.0" encoding="utf-8"?>\n<!DOCTYPE TS>\n' + ET.tostring(root, encoding="unicode") + '\n')
        print(f"Updated {path.relative_to(ROOT)}")


if __name__ == "__main__": main()
