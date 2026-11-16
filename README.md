# ESP-Miner-rs · Bitaxe Gamma 601 + AxeOS officiel

Firmware Rust pour ESP32-S3 avec pilote BM1370, alimentation Gamma 601 et **véritable interface officielle AxeOS** embarquée. Les écrans Angular d’ESP-Miner sont conservés ; leur API est adaptée côté Rust.

Tableau de bord, mesures, historique, réglages réseau/pool/matériel, pause/reprise, redémarrage et journaux sont raccordés. L’intégration compile et dispose de tests sur ordinateur ; **la validation sur une Gamma physique reste à effectuer**. Certaines fonctions d’ESP-Miner restent absentes, notamment OTA, OLED, NTP, SV2 et pools multiples : consulter [l’état précis](docs/status.md).

## Organisation

```text
firmware/             Firmware ESP32-S3 et ressources AxeOS de production
crates/               Minage, alimentation Gamma, modèles et adaptateur API
web/axeos/            Sources Angular officielles AxeOS
web/openapi.yaml      Contrat API officiel d’ESP-Miner
vendor/bm13xx-rs/     Pilotes ASIC de l’archive fournie
scripts/              Compilation, vérification, ZIP et import GitHub sécurisé
docs/                 Installation, API, limitations et provenance
.github/workflows/    Vérifications Rust et compilation d’AxeOS
```

## Compiler et installer sur Gamma 601

Installer la chaîne ESP32-S3 avec espup et charger son fichier d’environnement. Les ressources officielles sont déjà incluses : Node.js n’est pas nécessaire pour compiler le firmware livré.

```bash
cd firmware
WIFI_SSID="votre_reseau" WIFI_PASSWORD="votre_mot_de_passe" \
  cargo build --locked --release --features=bitaxe-gamma
espflash flash --partition-table partitions.csv \
  target/xtensa-esp32s3-none-elf/release/esp-miner-rs
```

Ouvrir `http://IP_DE_LA_CARTE/` avec son IPv4 DHCP. Configurer le worker/pool et redémarrer. Les variantes Max et Ultra restent disponibles, avec une seule feature sélectionnée.

Lire [le guide de compilation](docs/build.md), [l’adaptateur API](docs/axeos-api.md) et [les limitations](docs/status.md) avant l’essai sur carte.

## Tester visuellement l’interface officielle

Avec Node.js 22+ :

```bash
cd web/axeos
npm ci
npm run generate:api
npm start
```

Ouvrir http://localhost:4200 : mode mock officiel, données fictives, sans matériel. Les écrans, styles, polices et icônes n’ont pas été refaits.

Depuis la racine, `python3 scripts/build-axeos.py` reconstruit l’application de production **et ses ressources embarquées**. `--demo` produit uniquement un aperçu mock et conserve les ressources de production intactes.

## Vérifications et ZIP

```bash
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
python3 scripts/check-axeos.py
python3 scripts/package-repo.py ../esp-miner-rs-axeos.zip
```

Le ZIP contient les sources et les ressources de production, sans caches ni dépendances téléchargées. `SOURCE-MANIFEST.json` permet de vérifier son contenu.

## Mettre à jour votre dépôt GitHub

Depuis le nouveau ZIP extrait dans un dossier neuf :

```bash
python3 scripts/prepare-github.py /home/esp-miner-rs-github
```

Ce script vérifie le manifeste, prépare un commit dans une nouvelle copie et affiche la commande d’envoi. Votre dossier existant reste intact. Ne pas utiliser `sudo` avec Git. Voir [le guide](docs/build.md).

## Licences

Le code Rust conserve ses licences MIT/Apache-2.0. AxeOS conserve la licence d’ESP-Miner dans [web/LICENSE-ESP-Miner](web/LICENSE-ESP-Miner). Les ressources de production embarquent cette licence et les notices des dépendances tierces. Les sources nécessaires à leur reconstruction sont incluses. Voir [la provenance](docs/upstream.json).
