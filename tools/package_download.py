#!/usr/bin/env python3
"""Create a firmware download with the matching instructions and license notices."""
import json
import re
import subprocess
from pathlib import Path
from urllib.parse import quote, unquote, urlsplit
from zipfile import ZipFile, ZIP_DEFLATED

from verify_stock import verify

ROOT = Path(__file__).resolve().parents[1]
manifest = json.loads((ROOT / 'firmware/manifest.json').read_text())
verify()
source_commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
files = []
for image in manifest['images']:
    path = ROOT / 'firmware' / image['filename']
    files.append(path)
files += [ROOT / name for name in ['firmware/manifest.json', 'firmware/SHA256SUMS',
          'firmware/definition.xz', 'README.md', 'docs/cornix-stock.md', 'docs/cornix-stock-validation.md', 'keymaps/README.md',
          'provenance.json', 'LICENSE-MIT', 'LICENSE-APACHE', 'THIRD_PARTY_NOTICES.md']]
files += [p for p in (ROOT / 'licenses').rglob('*') if p.is_file()]
included = {path.resolve() for path in files}


def documentation(path):
    # A binary download has no source tree. Keep local guide links and direct
    # development/source references to the exact public release instead.
    def resolve(match):
        label, link = match.groups()
        url = urlsplit(link)
        if url.scheme or url.netloc or not url.path:
            return match.group(0)
        target = (path.parent / unquote(url.path)).resolve()
        relative = target.relative_to(ROOT).as_posix()
        if not target.exists():
            raise ValueError('Broken document link: ' + relative)
        if target in included or (target.is_dir() and any(p.is_relative_to(target) for p in included)):
            return match.group(0)
        kind = 'tree' if target.is_dir() else 'blob'
        address = f'https://github.com/yuchamichami/CoreX-Cornix-RMK/{kind}/{source_commit}/{quote(relative)}'
        return f'[{label}]({address}' + (f'#{url.fragment}' if url.fragment else '') + ')'
    return re.sub(r'\[([^\]]+)\]\(([^)]+)\)', resolve, path.read_text())
out = ROOT / 'build/downloads' / f"CoreX-Cornix-firmware-{manifest['version']}.zip"
out.parent.mkdir(parents=True, exist_ok=True)
with ZipFile(out, 'w', ZIP_DEFLATED) as archive:
    for path in sorted(files):
        if path.suffix == '.md' and not path.is_relative_to(ROOT / 'licenses'):
            archive.writestr(path.relative_to(ROOT).as_posix(), documentation(path))
        else:
            archive.write(path, path.relative_to(ROOT))
    archive.writestr('START-HERE.txt', 'CoreX for stock Cornix\n\n純正Cornixの左右セット用です。docs/cornix-stock.md から始めてください。\nfirmwareフォルダのRightを右、Leftを左へ使います。CoreX交換基板には使えません。\n実機未確認の試験版です。docs/cornix-stock-validation.md を確認してください。\n\nソースコードとライセンス:\nhttps://github.com/yuchamichami/CoreX-Cornix-RMK\n')
print(out)
