"""Collects English and Ukrainian UI strings from the macOS localization tables
(*.loctable) into en.txt and uk.txt, as a text corpus for tools/bigrams.py.

Usage (macOS only): python3 tools/extract_macos_corpus.py OUTPUT_DIR
"""

import json
import pathlib
import subprocess
import sys

out = {"en": [], "uk": []}
files = subprocess.run(
    ["find", "/System/Library", "/Applications", "-name", "*.loctable"],
    capture_output=True,
    text=True,
).stdout.split()
for path in files:
    converted = subprocess.run(["plutil", "-convert", "json", "-o", "-", path], capture_output=True)
    if converted.returncode:
        continue
    try:
        data = json.loads(converted.stdout)
    except ValueError:
        continue
    for lang in out:
        table = data.get(lang) or (data.get("en_US") if lang == "en" else None)
        if isinstance(table, dict):
            out[lang].extend(v for v in table.values() if isinstance(v, str))

for lang, texts in out.items():
    pathlib.Path(sys.argv[1], f"{lang}.txt").write_text("\n".join(texts), encoding="utf-8")
    print(f"{lang}: {len(texts)} strings")
