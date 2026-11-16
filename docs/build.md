# Compilation et installation

## Firmware Rust

Installer Rust, espup et espflash. Installer la cible ESP32-S3 avec `espup install -t esp32s3`, puis charger le fichier d’environnement indiqué par espup.

Depuis la racine du dépôt :

```bash
cd firmware
WIFI_SSID="votre_reseau" WIFI_PASSWORD="votre_mot_de_passe" \
  cargo build --locked --release --features=bitaxe-gamma
```

Les identifiants sont des variables de compilation, pas un fichier à publier sur GitHub. Pour Max ou Ultra, remplacer la feature par `bitaxe-max` ou `bitaxe-ultra`.

La première installation du serveur web Rust nécessite la table de partitions fournie :

```bash
espflash flash --partition-table partitions.csv \
  target/xtensa-esp32s3-none-elf/release/esp-miner-rs
```

Après le démarrage, utiliser l’adresse IP indiquée par les logs RTT ou par les baux DHCP du routeur. Le firmware utilise encore l’interface Rust interne, **pas** l’application Angular officielle. Lire [status.md](status.md).

Pour réinitialiser les réglages persistants sans effacer l’application :

```bash
espflash erase-region 0x9000 0x2000
```

Les réglages du réseau, du pool et du matériel sont alors effacés. Au prochain démarrage, le Wi-Fi de compilation est réutilisé.

## AxeOS officiel : aperçu visuel

Node.js 22+ est nécessaire. Depuis la racine :

```bash
cd web/axeos
npm ci
npm run generate:api
npm start
```

Ouvrir http://localhost:4200. Cette commande utilise le mode mock officiel, sans périphérique réel. Les styles, écrans, icônes et polices proviennent du dépôt amont.

Pour un paquet statique de démonstration :

```bash
python scripts/build-axeos.py --demo
python -m http.server 8080 --directory web/axeos/dist/axe-os
```

Ouvrir http://localhost:8080. Le routage Angular utilise les fragments `#/...`.

La compilation sans `--demo` sélectionne la configuration de production et l’API réelle d’ESP-Miner. Elle ne rend pas le serveur Rust compatible à elle seule.

## Client Rust interne

Cette interface est conservée pour garder le firmware existant compilable. Ce n’est pas l’interface officielle AxeOS.

Les ressources déjà compilées sont incluses dans `firmware/web-assets/`. Pour les reconstruire : installer Python 3.11+, la cible `wasm32-unknown-unknown` et wasm-bindgen-cli à la version de `web/embedded-rust/Cargo.lock`, puis :

```bash
python scripts/build-embedded-web.py
```

## Tests

```bash
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo test --locked --manifest-path web/embedded-rust/Cargo.toml --all-features
```

La compilation et les tests sur ordinateur ne remplacent pas la validation sur une carte réelle.
