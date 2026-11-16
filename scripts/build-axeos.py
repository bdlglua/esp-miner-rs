#!/usr/bin/env python3
"""Build the unmodified official AxeOS UI; --demo selects its mock environment."""
import argparse,copy,json,os,shutil,subprocess,sys
from pathlib import Path
root=Path(__file__).resolve().parents[1]
web=root/'web/axeos'
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--demo',action='store_true',help='Use official mock data, without a Bitaxe')
args=parser.parse_args()
if not (web/'node_modules').exists():
 raise SystemExit('Dépendances absentes. Exécuter : cd web/axeos && npm ci')
npm='npm.cmd' if os.name=='nt' else 'npm'
subprocess.run([npm,'run','generate:api'],cwd=web,check=True)
config=web/'angular.json';original=config.read_bytes();env=os.environ.copy()
env.setdefault('NG_BUILD_MAX_WORKERS','2')
try:
 mode='production'
 if args.demo:
  data=json.loads(original)
  configs=data['projects']['axe-os']['architect']['build']['configurations']
  configs['visual-demo']=copy.deepcopy(configs['production'])
  configs['visual-demo']['fileReplacements']=[{'replace':'src/environments/environment.ts','with':'src/environments/environment.mock.ts'}]
  config.write_text(json.dumps(data,indent=2)+'\n')
  mode='visual-demo'
 subprocess.run([npm,'exec','--','ng','build','--configuration',mode,'--extract-licenses'],cwd=web,env=env,check=True)
finally:
 config.write_bytes(original)
shutil.copyfile(root/'web/LICENSE-ESP-Miner',web/'dist/axe-os/LICENSE-ESP-Miner.txt')
(web/'dist/axe-os/axeos-build.json').write_text(json.dumps({'mock':args.demo,'revision':json.loads((root/'docs/upstream.json').read_text())['axeos']['revision']},indent=2)+'\n')
print('Fichiers générés dans web/axeos/dist/axe-os/')
if not args.demo:
 subprocess.run([sys.executable,str(root/'scripts/embed-axeos.py')],check=True)
else:
 print('Démonstration mock : les ressources du firmware ne sont pas remplacées.')
