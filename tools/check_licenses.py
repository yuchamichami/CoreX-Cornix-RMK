#!/usr/bin/env python3
"""Verify registry license coverage against every distributed Cargo lockfile."""

import argparse
import hashlib
import json
from pathlib import Path
import tarfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]
LOCKFILES = (
    "source/corex-rmk-pair/right/Cargo.lock",
    "source/corex-rmk-pair/left/Cargo.lock",
    "tools/pointing-host-tests/Cargo.lock",
    "source/corex-rmk-upstream/rmk/Cargo.lock",
)
LOCAL_PACKAGES = {
    "corex-pair-left", "corex-pair-right", "corex-pointing-host-tests",
    "rmk", "rmk-config", "rmk-macro", "rmk-types",
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def notice_name(name):
    return (
        any(word in Path(name).name.lower()
            for word in ("license", "licence", "copying", "copyright", "notice"))
        and not name.endswith((".rs", ".c", ".h", ".html"))
    )


def local_file(name):
    path = (ROOT / name).resolve()
    require(path.is_relative_to(ROOT), f"Notice path leaves repository: {name}")
    require(path.is_file(), f"Missing notice: {name}")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--crate-cache", type=Path,
        help="Optional Cargo registry cache directory containing .crate archives",
    )
    args = parser.parse_args()

    packages = {}
    memberships = {}
    locals_found = set()
    for name in LOCKFILES:
        for package in tomllib.loads((ROOT / name).read_text())["package"]:
            key = (package["name"], package["version"])
            if "source" not in package:
                locals_found.add(package["name"])
                continue
            require(package["source"].startswith("registry+"),
                    f"Unreviewed non-registry source: {key}")
            if key in packages:
                previous = packages[key]
                require(previous["source"] == package["source"]
                        and previous["checksum"] == package["checksum"],
                        f"Conflicting lockfile sources: {key}")
            packages[key] = package
            memberships.setdefault(key, []).append(name)
    require(locals_found == LOCAL_PACKAGES,
            f"Local source inventory changed: {locals_found ^ LOCAL_PACKAGES}")

    records = json.loads((ROOT / "licenses/dependencies.json").read_text())
    inventory = {(entry["name"], entry["version"]): entry for entry in records}
    require(len(inventory) == len(records), "Duplicate dependency inventory entry")
    require(inventory.keys() == packages.keys(),
            f"Dependency inventory mismatch; missing={packages.keys() - inventory.keys()}, "
            f"extra={inventory.keys() - packages.keys()}")

    notices_checked = 0
    archive_notices = 0
    for key, entry in inventory.items():
        package = packages[key]
        for field in ("source", "checksum"):
            require(entry[field] == package[field], f"Wrong {field}: {key}")
        require(entry["lockfiles"] == memberships[key], f"Wrong lockfile membership: {key}")
        files = entry["license_files"]
        require(files and len(files) == len(set(files)), f"Empty/duplicate notices: {key}")
        require(set(entry["license_file_sha256"]) == set(files), f"Missing notice hash: {key}")
        for name in files:
            require(digest(local_file(name).read_bytes()) == entry["license_file_sha256"][name],
                    f"Notice hash mismatch: {name}")
            notices_checked += 1
        if "provenance_file" in entry:
            require(digest(local_file(entry["provenance_file"]).read_bytes())
                    == entry["provenance_sha256"], f"Provenance hash mismatch: {key}")

        if args.crate_cache is None:
            continue
        stem = "-".join(key)
        archive = args.crate_cache.expanduser() / (stem + ".crate")
        require(archive.is_file(), f"Missing crate archive: {stem}")
        require(digest(archive.read_bytes()) == package["checksum"],
                f"Crate archive checksum mismatch: {stem}")
        with tarfile.open(archive) as source:
            metadata = tomllib.loads(source.extractfile(stem + "/Cargo.toml").read().decode())["package"]
            if "license" in metadata:
                require(entry["license"] == metadata["license"], f"Changed license declaration: {stem}")
            else:
                require("license-file" in metadata and entry["license"].startswith("LicenseRef-"),
                        f"Missing package license declaration: {stem}")
            require(entry.get("repository") == metadata.get("repository"),
                    f"Changed repository metadata: {stem}")
            # Also verify original README files carrying embedded license text,
            # not only files whose names explicitly mention a license.
            for name in files:
                prefix = "licenses/dependencies/"
                if not name.startswith(prefix):
                    continue
                member_name = name[len(prefix):]
                try:
                    original = source.getmember(member_name)
                except KeyError:
                    continue  # Separately documented upstream/extracted notice.
                require(original.isfile()
                        and local_file(name).read_bytes() == source.extractfile(original).read(),
                        f"Archived source differs: {name}")
            for member in source.getmembers():
                if not member.isfile() or not notice_name(member.name):
                    continue
                name = "licenses/dependencies/" + member.name
                require(name in files, f"Unlisted archive notice: {name}")
                require(local_file(name).read_bytes() == source.extractfile(member).read(),
                        f"Archive notice differs: {name}")
                archive_notices += 1

    print(f"License inventory: {len(inventory)} registry packages, "
          f"{notices_checked} notice references, {len(locals_found)} local packages accounted for")
    if args.crate_cache is not None:
        print(f"Verified all {len(inventory)} crate archive checksums and "
              f"{archive_notices} original archive notices")


if __name__ == "__main__":
    main()
