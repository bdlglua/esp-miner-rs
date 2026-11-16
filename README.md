# ESP-Miner-rs · Bitaxe Gamma 601

Firmware Rust pour ESP32-S3 avec prise en charge du BM1370 et sources de l’interface officielle AxeOS.

**État actuel : l’interface officielle Angular fonctionne en démonstration, mais son API n’est pas encore raccordée au firmware Rust. Le firmware compile avec l’interface Rust interne conservée dans `web/embedded-rust`. Ce dépôt ne prétend pas fournir une intégration matérielle complète d’AxeOS officiel.**

## Organisation

```text
firmware/             Firmware ESP32-S3, configuration et ressources embarquées
crates/               Logique de minage, alimentation Gamma et API Rust
web/axeos/            Interface Angular officielle AxeOS
web/embedded-rust/    Interface Rust interne encore utilisée par le firmware
web/openapi.yaml      Contrat API officiel d’ESP-Miner
vendor/bm13xx-rs/     Pilotes ASIC de l’archive fournie
scripts/              Compilation des interfaces et création du ZIP
docs/                Installation, état de l’intégration et provenance
.github/workflows/    Vérifications Rust et compilation d’AxeOS
```

## Tester visuellement AxeOS officiel

Node.js 22 ou supérieur est nécessaire.

```bash
cd web/axeos
npm ci
npm run generate:api
npm start
```

Ouvrir http://localhost:4200. Le mode démo fourni par le projet officiel est activé ; aucun Bitaxe n’est requis. Les écrans et styles officiels n’ont pas été refaits.

Pour générer les fichiers statiques de démonstration depuis la racine :

```bash
python scripts/build-axeos.py --demo
```

Pour une compilation Angular de production : `python scripts/build-axeos.py`. Les fichiers produits restent dans `web/axeos/dist/axe-os/` et ne sont pas copiés automatiquement dans le firmware.

## Compiler le firmware Gamma 601

Installer la chaîne Xtensa avec espup et charger le fichier d’environnement qu’il fournit.

```bash
cd firmware
WIFI_SSID="votre_reseau" WIFI_PASSWORD="votre_mot_de_passe" \
  cargo build --locked --release --features=bitaxe-gamma
```

Les variantes `bitaxe-max` et `bitaxe-ultra` restent disponibles. Sélectionner une seule variante.

L’installation initiale nécessite `firmware/partitions.csv`. Voir [le guide de compilation](docs/build.md) et [l’état de l’intégration](docs/status.md) avant de flasher une carte.

## Vérifications sur ordinateur

```bash
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --manifest-path web/embedded-rust/Cargo.toml --all-features
```

## Mettre le projet sur GitHub

Extraire le ZIP puis ouvrir un terminal dans le dossier `esp-miner-rs` :

```bash
git init -b main
git add .
git commit -m "Initial import: BM1370 firmware and official AxeOS sources"
git remote add origin https://github.com/VOTRE_COMPTE/VOTRE_DEPOT.git
git push -u origin main
```

Créer d’abord un dépôt GitHub vide et remplacer l’URL par la vôtre. Les dossiers de compilation, dépendances téléchargées, caches et fichiers locaux sont ignorés. Aucun jeton GitHub ni mot de passe Wi-Fi personnel n’est inclus.

## Licences

Le code Rust conserve ses licences MIT/Apache-2.0. Les sources officielles AxeOS conservent la licence d’ESP-Miner dans [web/LICENSE-ESP-Miner](web/LICENSE-ESP-Miner), ainsi que les notices de leurs ressources. Voir [la provenance](docs/upstream.json).
