# Compilation et installation

## Firmware Gamma 601

Installer Rust, espup et espflash. Installer la cible avec `espup install -t esp32s3`, puis charger le fichier d’environnement indiqué par espup.

Les ressources AxeOS officielles de production sont déjà incluses : Node.js n’est pas nécessaire pour compiler le ZIP fourni.

```bash
cd firmware
WIFI_SSID="votre_reseau" WIFI_PASSWORD="votre_mot_de_passe" \
  cargo build --locked --release --features=bitaxe-gamma
```

Pour Max ou Ultra, remplacer la feature par `bitaxe-max` ou `bitaxe-ultra`. Les identifiants Wi-Fi initiaux sont des variables de compilation ; ne pas publier de fichier contenant vos mots de passe.

Installer par USB avec la table de partitions fournie :

```bash
espflash flash --partition-table partitions.csv \
  target/xtensa-esp32s3-none-elf/release/esp-miner-rs
```

Ouvrir `http://IP_DE_LA_CARTE/`, en utilisant l’IPv4 DHCP donnée par votre routeur. Vous obtenez la véritable interface AxeOS, avec l’API Rust. Configurer le worker et le pool, enregistrer puis redémarrer. Les essais sur carte physique restent à faire ; lire [status.md](status.md).

Pour effacer uniquement les réglages persistants :

```bash
espflash erase-region 0x9000 0x2000
```

Cette commande efface les réglages réseau/pool/matériel. Le prochain démarrage reprend les valeurs de compilation.

## Reconstruire AxeOS officiel

Node.js 22+ et Python 3.11+ sont nécessaires. Depuis la racine :

```bash
cd web/axeos
npm ci
cd ../..
python3 scripts/build-axeos.py
python3 scripts/check-axeos.py
```

Le script génère le client OpenAPI, compile Angular en production, conserve les notices de licence, puis reconstruit `firmware/web-assets/official/`. Recompiler ensuite le firmware pour y inclure les nouvelles ressources.

Les sources de l’interface n’ont pas été réécrites. La construction active l’extraction des licences tierces. Un manifeste identifie le mode de compilation ; l’outil d’intégration refuse une application mock.

## Aperçu visuel sur ordinateur

```bash
cd web/axeos
npm ci
npm run generate:api
npm start
```

Ouvrir http://localhost:4200. Il s’agit du mode mock officiel, avec des données fictives. Pour un aperçu statique depuis la racine :

```bash
python3 scripts/build-axeos.py --demo
python3 -m http.server 8080 --directory web/axeos/dist/axe-os
```

Ouvrir http://localhost:8080. Le routage utilise les fragments `#/...`. `--demo` laisse les ressources de production embarquées intactes.

## Vérifications

```bash
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
cargo fmt --manifest-path firmware/Cargo.toml --all -- --check
python3 scripts/check-axeos.py
```

Depuis `firmware/`, avec la chaîne Xtensa configurée :

```bash
WIFI_SSID="TestSSID" WIFI_PASSWORD="TestPASSWORD" \
  cargo clippy --locked --release --features=bitaxe-gamma -- -D warnings
```

Les tests de l’adaptateur utilisent les champs obligatoires du vrai contrat OpenAPI et des formulaires officiels. Ils vérifient les unités, secrets, limites, réglages incompatibles, colonnes des graphiques et échanges WebSocket. Ils ne remplacent pas un essai sur une Gamma réelle.

## Mettre à jour votre dépôt GitHub depuis le ZIP

Extraire le nouveau ZIP dans un dossier neuf. Depuis ce dossier, exécuter :

```bash
python3 scripts/prepare-github.py /home/esp-miner-rs-github
```

Le script vérifie tous les fichiers contre le manifeste SHA-256, lit l’URL et l’identité Git de votre dépôt existant, puis prépare un commit dans une **nouvelle copie**. Il n’efface rien dans votre dossier actuel et ne pousse rien automatiquement. Il affiche la commande exacte pour examiner le commit puis l’envoyer sur GitHub, sans `sudo` ni push forcé.
