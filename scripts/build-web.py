#!/usr/bin/env python3
"""Build the Rust/WASM UI and embed deterministic gzip assets in the firmware."""
import gzip, hashlib, json, os, shutil, subprocess, tomllib
from pathlib import Path

root=Path(__file__).resolve().parents[1]
web=root/'axeos-web'
cli=os.environ.get('WASM_BINDGEN','wasm-bindgen')
if not shutil.which(cli):
    raise SystemExit('Installer wasm-bindgen-cli (version indiquée dans axeos-web/Cargo.lock), ou définir WASM_BINDGEN.')
subprocess.run(['cargo','build','--locked','--manifest-path',str(web/'Cargo.toml'),'--release','--target','wasm32-unknown-unknown'],check=True)
lock=tomllib.loads((web/'Cargo.lock').read_text())
expected=next(p['version'] for p in lock['package'] if p['name']=='wasm-bindgen')
actual=subprocess.check_output([cli,'--version'],text=True).strip().split()[-1]
if actual!=expected:
    raise SystemExit(f'wasm-bindgen-cli {expected} requis, version détectée : {actual}')
package=web/'target/bindgen'
package.mkdir(parents=True,exist_ok=True)
subprocess.run([cli,str(web/'target/wasm32-unknown-unknown/release/axeos_web.wasm'),'--target','web','--out-name','axeos_web','--out-dir',str(package),'--no-typescript'],check=True)
out=root/'firmware/web-assets'
out.mkdir(parents=True,exist_ok=True)
for name in ['index.html','boot.js']:
    shutil.copyfile(web/'assets'/name,out/name)
for source,name in [(web/'assets/style.css','style.css'),(package/'axeos_web.js','axeos_web.js'),(package/'axeos_web_bg.wasm','axeos_web_bg.wasm')]:
    (out/(name+'.gz')).write_bytes(gzip.compress(source.read_bytes(),compresslevel=9,mtime=0))
assert gzip.decompress((out/'axeos_web_bg.wasm.gz').read_bytes()).startswith(b'\x00asm')
files=[p for p in sorted(out.iterdir()) if p.name!='manifest.json']
manifest={p.name:{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()} for p in files}
manifest['wasm-bindgen']=expected
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'embedded_bytes':sum(p.stat().st_size for p in files),'assets':manifest},indent=2))
