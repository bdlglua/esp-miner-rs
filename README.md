# ESP-Miner-rs

Firmware Rust pour **Bitaxe Gamma 601 / BM1370**, avec l’interface officielle **AxeOS** embarquée sur l’ESP32-S3.

Le navigateur charge l’application Angular d’[ESP-Miner](https://github.com/bitaxeorg/ESP-Miner). Le firmware et l’adaptateur de son API sont écrits en Rust ; les écrans et les styles officiels sont conservés.

> **État du projet :** compilation et tests sur ordinateur effectués. La validation du firmware sur une Gamma physique reste à réaliser.

## Fonctions disponibles

- Pilote BM1370 et gestion de l’alimentation TPS546 sur Gamma 601.
- Mesures et ventilation avec l’EMC2101, protection thermique et arrêt en cas de défaut.
- Minage avec un pool Stratum V1 TCP.
- Tableau de bord AxeOS, historique des mesures et réglages persistants.
- Configuration du Wi-Fi, du pool, du worker, de la fréquence, de la tension et du ventilateur.
- Pause, reprise, redémarrage et journaux par WebSocket.
- Ressources web compressées, servies directement depuis la mémoire flash.

Les valeurs par défaut de la Gamma 601 sont **525 MHz** et **1 150 mV**. Les limites de configuration sont détaillées dans [l’état de l’intégration](docs/status.md).

## Compiler et installer

### Prérequis

- Une chaîne Rust pour ESP32-S3 installée avec **espup**, avec son fichier d’environnement chargé.
- **espflash** pour installer le firmware par USB.

Les ressources AxeOS de production sont déjà incluses dans le dépôt. Node.js est nécessaire uniquement pour reconstruire l’interface ou lancer son aperçu sur ordinateur.

### Compilation pour Gamma 601

```bash
git clone https://github.com/bdlglua/esp-miner-rs.git
cd esp-miner-rs/firmware

WIFI_SSID="votre_reseau" WIFI_PASSWORD="votre_mot_de_passe" \
  cargo build --locked --release --features=bitaxe-gamma
```

### Installation par USB

Depuis le dossier `firmware/` :

```bash
espflash flash --partition-table partitions.csv \
  target/xtensa-esp32s3-none-elf/release/esp-miner-rs
```

Après le démarrage, retrouvez l’adresse IPv4 de la carte dans les baux DHCP de votre routeur, puis ouvrez `http://IP_DE_LA_CARTE/`. Configurez votre pool et votre worker dans AxeOS, enregistrez puis redémarrez.

Les réglages réseau, pool, fréquence et tension prennent effet après redémarrage. Le Wi-Fi de compilation sert à la configuration initiale.

Les variantes `bitaxe-ultra` et `bitaxe-max` sont également présentes. Sélectionnez une seule variante ; la cible principale de cette intégration est la Gamma 601. Consultez le [guide de compilation](docs/build.md) pour les détails.

## Tester visuellement AxeOS sans carte

Avec **Node.js 22** et npm :

```bash
cd web/axeos
npm ci
npm run generate:api
npm start
```

Ouvrez **http://localhost:4200**. Cet aperçu utilise les données fictives du mode mock officiel et ne mine pas.

## Reconstruire l’interface embarquée

Depuis la racine du dépôt, avec Python 3.11+ et les dépendances npm installées :

```bash
python3 scripts/build-axeos.py
python3 scripts/check-axeos.py
```

La compilation de production met à jour `firmware/web-assets/official/`. Recompilez ensuite le firmware pour embarquer ces ressources.

Pour générer uniquement un aperçu statique avec des données fictives :

```bash
python3 scripts/build-axeos.py --demo
python3 -m http.server 8080 --directory web/axeos/dist/axe-os
```

Le mode `--demo` conserve les ressources de production du firmware.

## Limites actuelles

OTA, OLED, NTP, scan Wi-Fi, Stratum V2, TLS, pools multiples, Swarm et PID de ventilation personnalisable ne sont pas implémentés. La présence d’un bouton dans AxeOS ne signifie pas que la fonction correspondante est disponible dans ce firmware.

Certaines mesures sont indisponibles et utilisent des valeurs de remplacement compatibles avec l’interface officielle. Les détails, les fonctions prises en charge et les essais matériels restants figurent dans [docs/status.md](docs/status.md).

## Organisation

```text
firmware/             Firmware ESP32-S3 et ressources AxeOS embarquées
crates/               Minage, alimentation Gamma et adaptateur API
web/axeos/            Sources officielles de l’interface Angular
web/openapi.yaml      Contrat API d’ESP-Miner
vendor/bm13xx-rs/     Pilotes ASIC
scripts/              Compilation, vérification et création des archives
docs/                 Installation, API, limites et provenance
.github/workflows/    Vérifications Rust et compilation d’AxeOS
```

## Vérifications

Depuis la racine :

```bash
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
python3 scripts/check-axeos.py
```

Voir [les vérifications effectuées](docs/validation.md) et [le fonctionnement de l’API](docs/axeos-api.md).

## Licences et provenance

Le code Rust conserve ses licences [MIT](LICENSE-MIT) et [Apache-2.0](LICENSE-APACHE). L’interface AxeOS conserve la [licence d’ESP-Miner](web/LICENSE-ESP-Miner), ainsi que les notices de ses dépendances.

Les sources et leur révision d’origine sont indiquées dans [docs/upstream.json](docs/upstream.json).
