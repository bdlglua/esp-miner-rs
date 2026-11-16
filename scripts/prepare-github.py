#!/usr/bin/env python3
"""Prepare a verified update in a NEW clone; never delete files in your current repo."""
import argparse
import hashlib
import json
import shlex
import shutil
import subprocess
import tempfile
from pathlib import Path


def git(repo, *args):
    return subprocess.check_output(['git', '-C', str(repo), *args], text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('existing_repo', type=Path, help='Existing repository, used only to read origin and author identity')
    args = parser.parse_args()
    source = Path(__file__).resolve().parents[1]
    manifest_file = source / 'SOURCE-MANIFEST.json'
    if not manifest_file.is_file():
        raise SystemExit('Utiliser ce script depuis le ZIP complet extrait dans un dossier neuf.')
    manifest = json.loads(manifest_file.read_text())
    for name, digest in manifest.items():
        path = Path(name)
        if path.is_absolute() or '..' in path.parts or '.git' in path.parts:
            raise SystemExit(f'Chemin invalide : {name}')
        file = source / path
        if file.is_symlink() or not file.is_file() or hashlib.sha256(file.read_bytes()).hexdigest() != digest:
            raise SystemExit(f'Fichier absent ou modifié : {name}. Extraire à nouveau le ZIP complet.')
    for required in ['firmware/Cargo.toml', 'crates/axeos-core/src/official.rs', 'web/axeos/package.json', 'firmware/web-assets/official/assets.rs']:
        if required not in manifest:
            raise SystemExit(f'Archive incomplète : {required}')
    existing = args.existing_repo.expanduser().resolve()
    origin = git(existing, 'remote', 'get-url', 'origin')
    author = git(existing, 'config', 'user.name')
    email = git(existing, 'config', 'user.email')
    if not author or not email:
        raise SystemExit('Configurer user.name et user.email dans votre dépôt existant, sans sudo.')
    parent = Path(tempfile.mkdtemp(prefix='esp-miner-github-', dir=Path.home()))
    repo = parent / 'esp-miner-rs'
    subprocess.run(['git', 'clone', '--branch', 'main', '--no-checkout', origin, str(repo)], check=True)
    for name in [*manifest, 'SOURCE-MANIFEST.json']:
        destination = repo / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source / name, destination)
    # No checkout means the fresh clone had no source files to overwrite or remove.
    # Staging the validated complete source set records renames and retired files.
    git(repo, 'config', 'user.name', author)
    git(repo, 'config', 'user.email', email)
    git(repo, 'add', '-A')
    staged = set(subprocess.check_output(['git', '-C', str(repo), 'ls-files', '-z']).decode().rstrip('\0').split('\0'))
    if staged != set(manifest) | {'SOURCE-MANIFEST.json'}:
        raise SystemExit('La liste des fichiers indexés diffère du manifeste. Aucun envoi effectué.')
    subprocess.run(['git', '-C', str(repo), 'diff', '--quiet'], check=True)
    print(git(repo, 'diff', '--cached', '--shortstat'))
    changed = subprocess.run(['git', '-C', str(repo), 'diff', '--cached', '--quiet']).returncode
    if changed:
        subprocess.run(['git', '-C', str(repo), 'commit', '-m', 'Integrate official AxeOS with Rust firmware'], check=True)
    print(f'\nProjet vérifié : {repo}')
    print('Pour examiner les changements :')
    print(f'git -C {shlex.quote(str(repo))} show --stat HEAD')
    print('Pour envoyer sur GitHub :')
    print(f'git -C {shlex.quote(str(repo))} push origin HEAD:main')


if __name__ == '__main__':
    main()
