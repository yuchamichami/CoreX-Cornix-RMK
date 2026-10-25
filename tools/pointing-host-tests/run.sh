#!/bin/sh
# Pure host tests; no device access. Each test receives a fresh mock clock/process.
set -eu
COREX_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
COREX_HOST=$(rustup run 1.95.0 rustc -vV | sed -n 's/^host: //p')
COREX_TEST_TARGET="$COREX_ROOT/build/tests/pointing"
mkdir -p "$COREX_TEST_TARGET"
COREX_FIRMWARE_VERSION=$(cat "$COREX_ROOT/source/corex-rmk-pair/VERSION")
export COREX_FIRMWARE_VERSION
if ! rustup run 1.95.0 cargo test \
  --manifest-path "$COREX_ROOT/tools/pointing-host-tests/Cargo.toml" \
  --locked \
  --target "$COREX_HOST" --target-dir "$COREX_TEST_TARGET" \
  --lib --no-run --message-format=json > "$COREX_TEST_TARGET/artifacts.jsonl"; then
  python3 - "$COREX_TEST_TARGET/artifacts.jsonl" <<'PY'
import json, sys
for line in open(sys.argv[1]):
    entry = json.loads(line)
    if entry.get('reason') == 'compiler-message':
        print(entry['message'].get('rendered', ''), file=sys.stderr)
PY
  exit 1
fi
python3 - "$COREX_TEST_TARGET/artifacts.jsonl" <<'PY'
import json, os, subprocess, sys
artifacts = [json.loads(line) for line in open(sys.argv[1]) if line.strip()]
executables = [a['executable'] for a in artifacts
               if a.get('reason') == 'compiler-artifact' and a.get('executable')
               and a.get('profile', {}).get('test') and a['target']['name'] == 'corex_pointing_host_tests']
assert len(executables) == 1, executables
exe = executables[0]
names = [line.removesuffix(': test') for line in
         subprocess.check_output([exe, '--list', '--format', 'terse'], text=True).splitlines()
         if line.endswith(': test')]
assert any(name.startswith('ports::') for name in names)
for name in names:
    subprocess.run([exe, '--exact', name], check=True,
                   env={**os.environ, 'RMK_TEST_PROCESS_ISOLATED': '1'})
print(f'PASS {len(names)} CoreX pointing host tests; each ran in a fresh process')
PY
