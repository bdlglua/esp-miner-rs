#!/usr/bin/env python3
"""Build the browser-only Rust/WASM visual demo, without altering firmware assets."""
import argparse, json, os, shutil, subprocess, tomllib
from pathlib import Path
root=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser()
parser.add_argument('--out',type=Path,default=root/'web/embedded-rust/demo-dist')
args=parser.parse_args()
web=root/'web/embedded-rust'
cli=os.environ.get('WASM_BINDGEN','wasm-bindgen')
if not shutil.which(cli):raise SystemExit('Installer wasm-bindgen-cli ou définir WASM_BINDGEN.')
subprocess.run(['cargo','build','--locked','--manifest-path',str(web/'Cargo.toml'),'--release','--target','wasm32-unknown-unknown','--features','demo'],check=True)
lock=tomllib.loads((web/'Cargo.lock').read_text())
expected=next(p['version'] for p in lock['package'] if p['name']=='wasm-bindgen')
actual=subprocess.check_output([cli,'--version'],text=True).strip().split()[-1]
if actual!=expected:raise SystemExit(f'wasm-bindgen-cli {expected} requis, version détectée : {actual}')
out=args.out.resolve();out.mkdir(parents=True,exist_ok=True)
subprocess.run([cli,str(web/'target/wasm32-unknown-unknown/release/axeos_web.wasm'),'--target','web','--out-name','axeos_web','--out-dir',str(out),'--no-typescript'],check=True)
for name in ['index.html','boot.js','style.css']:shutil.copyfile(web/'assets'/name,out/name)
p=out/'index.html';s=p.read_text().replace('AxeOS Rust · Bitaxe','AxeOS Rust · Démonstration').replace('Chargement de l’interface WebAssembly…','Chargement de la démonstration Rust/WebAssembly…');p.write_text(s)
# Only the files served by the page are retained.
for p in out.iterdir():
 if p.name not in ['index.html','boot.js','style.css','axeos_web.js','axeos_web_bg.wasm']:p.unlink()
print(json.dumps({'directory':str(out),'mode':'browser-only demo','bytes':sum(p.stat().st_size for p in out.iterdir())},indent=2))
