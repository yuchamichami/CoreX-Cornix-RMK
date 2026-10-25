#!/usr/bin/env python3
"""Create a firmware download with the matching instructions and license notices."""
import json
import re
from pathlib import Path
from urllib.parse import quote, unquote, urlsplit
from zipfile import ZipFile, ZIP_DEFLATED

from verify_release import verify

ROOT = Path(__file__).resolve().parents[1]
manifest = json.loads((ROOT / 'firmware/manifest.json').read_text())
files = []
for image in manifest['images']:
    path = ROOT / 'firmware' / image['filename']
    verify(image, path)
    files.append(path)
files += [ROOT / name for name in ['firmware/manifest.json', 'firmware/SHA256SUMS',
          'README.md', 'docs/usage.md', 'docs/flashing.md', 'docs/validation.md', 'keymaps/README.md',
          'keymaps/CoreX-default.vil', 'LICENSE-MIT', 'LICENSE-APACHE', 'THIRD_PARTY_NOTICES.md']]
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
        address = f'https://github.com/yuchamichami/CoreX-RMK/{kind}/{manifest["release"]}/{quote(relative)}'
        return f'[{label}]({address}' + (f'#{url.fragment}' if url.fragment else '') + ')'
    return re.sub(r'\[([^\]]+)\]\(([^)]+)\)', resolve, path.read_text())
out = ROOT / 'build/downloads' / f"CoreX-firmware-{manifest['release']}.zip"
out.parent.mkdir(parents=True, exist_ok=True)
with ZipFile(out, 'w', ZIP_DEFLATED) as archive:
    for path in sorted(files):
        if path.suffix == '.md' and not path.is_relative_to(ROOT / 'licenses'):
            archive.writestr(path.relative_to(ROOT).as_posix(), documentation(path))
        else:
            archive.write(path, path.relative_to(ROOT))
    archive.writestr('START-HERE.txt', '''CoreX firmware / Cornix TB

書き込み済み製品を使う場合は docs/usage.md から始めてください。
更新する場合はfirmwareフォルダに書き込み用のUF2があります。
基板に合うファイルを選ぶ手順は docs/flashing.md を参照してください。
CoreX-Right: CoreX右 / CoreX-Left: CoreX左 / Cornix-Stock-Left: Cornix純正左
CoreX右・左は、ブートローダのSoftDeviceに合うs140v6またはs140v7を選びます。
この配布版の実機での最終確認は未実施です。docs/validation.md を参照してください。

ファームのソースコード、ライセンス中の参照ファイル:
https://github.com/yuchamichami/CoreX-RMK
''')
print(out)
