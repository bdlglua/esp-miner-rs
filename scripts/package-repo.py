#!/usr/bin/env python3
"""Create a clean source ZIP without Git history, downloaded dependencies or build caches."""
import argparse,hashlib,json,subprocess,zipfile
from pathlib import Path
root=Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('output',type=Path,help='Destination ZIP, outside the repository')
args=parser.parse_args();output=args.output.resolve()
if root==output or root in output.parents:raise SystemExit('Choisir une destination située hors du dépôt.')
if not (root/'.git').is_dir():raise SystemExit('Initialiser Git avec git init avant de créer le ZIP.')
raw=subprocess.check_output(['git','ls-files','--cached','--others','--exclude-standard','-z'],cwd=root)
files=sorted(set(p for p in raw.decode().split('\0') if p))
output.parent.mkdir(parents=True,exist_ok=True)
with zipfile.ZipFile(output,'w',zipfile.ZIP_DEFLATED,compresslevel=9) as z:
 manifest={}
 for name in files:
  p=root/name
  if p.is_symlink() or not p.is_file():continue
  if any(part in {'.git','node_modules','target','.angular','__pycache__','dist'} for part in Path(name).parts):continue
  if name=='SOURCE-MANIFEST.json':continue
  z.write(p,Path(root.name)/name)
  manifest[name]=hashlib.sha256(p.read_bytes()).hexdigest()
 z.writestr(str(Path(root.name)/'SOURCE-MANIFEST.json'),json.dumps(manifest,indent=2)+'\n')
print(output)
