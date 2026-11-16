#!/usr/bin/env python3
"""Build the unmodified official AxeOS UI; --demo selects its mock environment."""
import argparse,copy,json,os,subprocess,sys
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
 subprocess.run([npm,'exec','--','ng','build','--configuration',mode],cwd=web,env=env,check=True)
finally:
 config.write_bytes(original)
print('Fichiers générés dans web/axeos/dist/axe-os/')
print('L’API officielle reste à raccorder au serveur Rust ; ces fichiers ne remplacent pas automatiquement les ressources du firmware.')
