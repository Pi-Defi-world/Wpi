#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_DIR="${ROOT_DIR}/target/wasm32-unknown-unknown/release"
OUTPUT_FILE="${RELEASE_DIR}/wasm-provenance.json"
PYTHON="${PYTHON:-python3}"

cd "$ROOT_DIR"
mkdir -p "$RELEASE_DIR"

"$PYTHON" - "$OUTPUT_FILE" <<'PY'
import hashlib
import json
import subprocess
import sys
import tomllib
from pathlib import Path

output_file = Path(sys.argv[1])
root = Path.cwd()
manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
patches = manifest["patch"]["crates-io"]
package_names = (
    "ark-ec",
    "ark-ff",
    "ark-ff-asm",
    "ark-ff-macros",
    "ark-poly",
    "ark-serialize",
    "ark-serialize-derive",
    "wasmi_core",
)

metadata = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1"],
        text=True,
    )
)
packages = {package["name"]: package for package in metadata["packages"]}

dependencies = []
for name in package_names:
    patch = patches[name]
    expected_source = f"git+{patch['git']}?rev={patch['rev']}#{patch['rev']}"
    package = packages.get(name)
    actual_source = None if package is None else package.get("source")
    if actual_source != expected_source:
        raise SystemExit(
            f"{name}: expected locked source {expected_source}, got {actual_source}"
        )
    dependencies.append(
        {
            "name": name,
            "version": package["version"],
            "source": actual_source,
        }
    )

artifacts = []
for name in ("wpi_token.wasm", "mock_amm.wasm"):
    path = output_file.parent / name
    if not path.is_file():
        raise SystemExit(f"missing release artifact: {path}")
    data = path.read_bytes()
    artifacts.append(
        {
            "name": name,
            "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
        }
    )

lockfile = root / "Cargo.lock"
provenance = {
    "schema_version": 1,
    "cargo_lock_sha256": hashlib.sha256(lockfile.read_bytes()).hexdigest(),
    "dependencies": dependencies,
    "artifacts": artifacts,
}
output_file.write_text(
    json.dumps(provenance, indent=2, sort_keys=True) + "\n",
    encoding="utf-8",
)

for dependency in dependencies:
    print(f"{dependency['name']} {dependency['source']}")
for artifact in artifacts:
    print(f"{artifact['name']} sha256:{artifact['sha256']}")
print(f"Wrote {output_file}")
PY
