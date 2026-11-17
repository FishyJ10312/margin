#!/usr/bin/env python3
"""Collect dependency notices from an offline, platform-filtered Cargo inventory.

Conservatively includes build dependencies too. No credentials or machine paths
are written into the output. Missing notices fail packaging instead of silently
producing an incomplete list. Upstream fallback texts are pinned in the repo.
"""
import argparse
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser()
parser.add_argument("metadata", type=Path)
args = parser.parse_args()
metadata = json.loads(args.metadata.read_text())
version = json.loads((root / 'package.json').read_text())['version']
if not any(p['name'] == 'liubai' and not p['source'] and p['version'] == version for p in metadata['packages']):
    raise SystemExit('Cargo inventory does not match the current app version')
fallbacks = json.loads((root / "licenses/upstream-sources.json").read_text())
prefixes = ("LICENSE", "LICENCE", "COPYING", "NOTICE", "COPYRIGHT")
sections = ["Margin third-party dependency notices\n\n"
            "Exact Cargo dependency inventory, including build dependencies.\n"
            "License expressions are upstream declarations, not a legal opinion.\n"]
inventory = []
missing = []
for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
    if not package["source"]:
        continue
    directory = Path(package["manifest_path"]).parent
    notices = sorted(p for p in directory.rglob("*")
                     if p.is_file() and p.name.upper().startswith(prefixes))
    if package.get("license_file"):
        declared = directory / package["license_file"]
        if declared.is_file() and declared not in notices:
            notices.append(declared)
    fallback = fallbacks.get(package["name"])
    if not notices and fallback and fallback["version"] == package["version"]:
        notice = root / "licenses" / fallback["notice"]
        if hashlib.sha256(notice.read_bytes()).hexdigest() != fallback["sha256"]:
            raise SystemExit(f"Changed fallback notice: {package['name']}")
        notices = [notice]
    if not notices:
        missing.append(f"{package['name']} {package['version']}")
        continue
    record = {k: package.get(k) for k in ("name", "version", "license", "repository")}
    record["notices"] = []
    sections.append(f"\n{'=' * 72}\n{package['name']} {package['version']}\n"
                    f"License: {package['license']}\nSource: {package.get('repository') or package['source']}\n")
    for notice in notices:
        data = notice.read_bytes()
        label = notice.relative_to(directory).as_posix() if notice.is_relative_to(directory) else notice.name
        record["notices"].append({"path": label, "sha256": hashlib.sha256(data).hexdigest()})
        sections.append(f"\n--- {label} ---\n{data.decode('utf-8', errors='replace')}\n")
    inventory.append(record)

for name in ("svelte", "clsx", "esm-env", "@lucide/svelte", "@tauri-apps/api", "@tauri-apps/plugin-dialog", "@tauri-apps/plugin-opener"):
    directory = root / "node_modules" / name
    package = json.loads((directory / "package.json").read_text())
    notices = sorted(p for p in directory.iterdir() if p.is_file() and p.name.upper().startswith(prefixes))
    # The two JS plugins use an SPDX declaration referring to the Tauri license
    # texts already collected from their matching Rust plugin crates.
    if not notices:
        missing.append(name)
    sections.append(f"\n{'=' * 72}\n{name} {package['version']}\nLicense: {package.get('license')}\n")
    for notice in notices:
        sections.append(f"\n--- {notice.name} ---\n{notice.read_text()}\n")
    inventory.append({"name": name, "version": package["version"], "license": package.get("license"), "notices": [p.name for p in notices]})
if missing:
    raise SystemExit("Missing notice texts: " + ", ".join(missing))
(root / "licenses/THIRD-PARTY-NOTICES.txt").write_text("".join(sections))
(root / "licenses/dependency-inventory.json").write_text(json.dumps(inventory, indent=2) + "\n")
print(json.dumps({"packages": len(inventory), "noticeBytes": (root / "licenses/THIRD-PARTY-NOTICES.txt").stat().st_size}))
