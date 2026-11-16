#!/usr/bin/env python3
"""Check the complete embedded asset set, compression, hashes and browser references."""
import gzip
import hashlib
import json
import re
from pathlib import Path, PurePosixPath

root = Path(__file__).resolve().parents[1]
assets = root / 'firmware/web-assets/official'
manifest = json.loads((assets / 'manifest.json').read_text())
decoded = {}
for path, record in manifest.items():
    name = 'index.html' if path == '/' else path[1:]
    compressed = (assets / (name + '.gz')).read_bytes()
    data = gzip.decompress(compressed)
    assert len(compressed) == record['gzip'], path
    assert len(data) == record['bytes'], path
    assert hashlib.sha256(data).hexdigest() == record['sha256'], path
    decoded[path] = data
    assert json.dumps(path) + ' => Some(' in (assets / 'assets.rs').read_text(), path
assert json.loads(decoded['/axeos-build.json'])['mock'] is False
assert '/LICENSE-ESP-Miner.txt' in decoded
assert '/3rdpartylicenses.txt' in decoded
index = decoded['/'].decode()
assert '<app-root>' in index
for reference in re.findall(r'(?:src|href)="([^"]+)"', index):
    if reference == '/' or '://' in reference or reference.startswith('#'):
        continue
    assert '/' + reference.lstrip('/') in manifest, reference
for path, data in decoded.items():
    if not path.endswith('.css'):
        continue
    for reference in re.findall(r'url\([\'"]?([^\)\'\"]+)', data.decode()):
        if reference.startswith('data:'):
            continue
        absolute = str(PurePosixPath(path).parent / reference).split('?')[0].split('#')[0]
        assert absolute in manifest, absolute
print(f'{len(manifest)} production assets checked: gzip, hashes, HTML, fonts and licenses.')
