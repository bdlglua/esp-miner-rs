# AxeOS Rust pour esp-miner-rs

Interface locale **écrite en Rust et compilée en WebAssembly**, intégrée au firmware ESP32-S3. Le navigateur exécute le code WASM ; l’ESP32 sert les fichiers depuis sa flash et expose l’API HTTP. Aucun hébergement externe ni CDN n’est nécessaire au fonctionnement.

Ce port indépendant reprend les usages d’AxeOS dans [ESP-Miner](https://github.com/bitaxeorg/ESP-Miner). Il utilise son propre client Rust et un sous-ensemble d’API ; ce n’est pas une traduction exhaustive de l’application Angular ni une compatibilité complète avec toutes les versions de son API.

## Fonctions raccordées au firmware

| Écran | Fonctionnement |
| --- | --- |
| Vue d’ensemble | Température, puissance, tension réelle du cœur sur Gamma, ventilateur, shares acceptées/rejetées, nonces et meilleure difficulté. Historique RAM de 5 minutes. Débit estimé sur les nonces valides des 60 dernières secondes, séparé du débit théorique. |
| Pool | Hôte DNS ou IPv4, port, adresse/worker et mot de passe Stratum V1 TCP. Aucun wallet d’exemple n’est utilisé. |
| Matériel | Fréquence, tension bornée selon la carte, ventilation automatique ou manuelle. La protection impose 100 % à partir de 70 °C et coupe à plus de 75 °C. |
| Réseau | SSID, mot de passe Wi-Fi et nom DHCP du mineur. |
| Journaux | 32 événements applicatifs récents et téléchargement texte. Les logs techniques complets restent sur RTT. |
| Système | Version, mémoire libre, état du stockage et redémarrage. |

Le thème clair/sombre est mémorisé dans le navigateur. Les formulaires restent éditables pendant l’actualisation des mesures. Les mots de passe ne sont jamais renvoyés par l’API : un champ laissé vide conserve la valeur actuelle ; la case « Réseau ouvert » efface explicitement le mot de passe Wi-Fi.

La ventilation est appliquée au cycle de contrôle suivant (au plus 5 s). Le réseau, le pool, la fréquence et la tension sont enregistrés immédiatement mais **appliqués après redémarrage**. Une bannière indique les changements en attente. Pause/reprise agit sur le reset de la chaîne ASIC ; l’alimentation reste active en pause. Un défaut matériel verrouillé ne peut pas être effacé par « Reprendre ».

Sur Gamma 601, la puissance est celle de la sortie ASIC du TPS546 (`VOUT × IOUT`), sans estimation ajoutée de la consommation du reste de la carte. Sur Max/Ultra, elle provient de l’INA260 à l’entrée. Une mesure indisponible est affichée « — ». Les shares cumulées sont conservées pendant les reconnexions au pool, puis remises à zéro au redémarrage.

## Première compilation et installation sur Gamma 601

Les fichiers WASM déjà compilés sont inclus dans `firmware/web-assets/`. **Il n’est pas nécessaire d’installer les outils WASM pour compiler le firmware fourni.**

1. Installer la chaîne Xtensa avec espup, charger son fichier d’environnement et installer espflash.
2. Compiler le firmware avec les identifiants Wi-Fi initiaux :

```bash
cd firmware
WIFI_SSID="votre_reseau" WIFI_PASSWORD="votre_mot_de_passe" \
  cargo build --locked --release --features=bitaxe-gamma
```

3. Installer le firmware **avec la table de partitions fournie** :

```bash
espflash flash --partition-table partitions.csv \
  target/xtensa-esp32s3-none-elf/release/esp-miner-rs
```

Cette table utilise les 4 premiers Mo de flash : configuration `axe_cfg` à `0x9000` sur 8 Ko, application à `0x10000`. Les ressources web sont incluses dans l’application. La configuration stock ESP-Miner/NVS n’est pas importée ; les identifiants Wi-Fi initiaux viennent des variables de compilation.

4. Ouvrir `http://ADRESSE_IP_DU_BITAXE/` sur le même réseau. L’adresse est visible dans les logs RTT ou dans les baux DHCP du routeur.
5. Dans **Pool**, renseigner votre adresse/worker (64 octets maximum, limite du client Stratum), enregistrer puis redémarrer.

`just build gamma` reste disponible après modification du Wi-Fi dans le justfile. Le runner probe-rs actuel peut mettre à jour l’application, mais n’installe pas la table de partitions : utiliser espflash pour la première installation avec AxeOS.

## Stockage et récupération

La configuration utilise deux secteurs distincts, un numéro de génération et un CRC32 couvrant l’en-tête et le contenu. Le secteur précédent n’est pas effacé avant la validation du nouveau. L’en-tête de validation est écrit après le contenu, puis relu. Le chargeur refuse les partitions absentes, incorrectes ou chevauchantes.

Si `axe_cfg` n’est pas trouvée, l’interface reste consultable avec les valeurs de compilation et les sauvegardes échouent explicitement. Elle n’écrit pas dans une partition NVS supposée.

En cas de paramètres Wi-Fi incorrects, effacer uniquement la configuration puis redémarrer pour retrouver le Wi-Fi de compilation :

```bash
espflash erase-region 0x9000 0x2000
```

Cette opération efface les réglages du pool et du matériel également. Le port ne fournit pas de point d’accès Wi-Fi de récupération.

## Reconstruire l’interface Rust/WASM

Prérequis : Python 3.11+, Rust stable, cible `wasm32-unknown-unknown` et wasm-bindgen-cli à la version du `axeos-web/Cargo.lock` (actuellement 0.2.130).

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked
python scripts/build-web.py
```

Le script compile le Rust, génère les bindings navigateur, compresse les ressources de façon déterministe et met à jour `firmware/web-assets/manifest.json`. Recompiler ensuite le firmware. La logique d’affichage, les formulaires, la navigation, les requêtes et la courbe sont en Rust. Le JavaScript livré se limite aux bindings générés et à l’amorçage WASM.

## Architecture

- `axeos-core/` : modèle borné `no_std`, validation des réglages, records flash et parseur HTTP testables sur ordinateur.
- `axeos-web/` : interface Rust/WASM avec wasm-bindgen et web-sys, sans Angular.
- `firmware/src/web.rs` : deux tâches HTTP Embassy sur le port 80, ressources gzip servies depuis la flash, buffers bornés et délais de connexion.
- `firmware/src/settings.rs` : accès à la partition `axe_cfg`.
- `firmware/src/state.rs` : snapshots de mesures, historique et journal bornés, sans verrou maintenu pendant une opération asynchrone.
- `firmware/src/{asic,pool,power}.rs` : remontée des données et application des commandes.

## API de ce port

| Méthode et chemin | Rôle |
| --- | --- |
| `GET /api/system/info` | Informations, réglages publics et télémétrie, en JSON. |
| `GET /api/system/statistics` | Historique des mesures, en JSON. |
| `GET /api/system/events` | Événements applicatifs, en JSON. |
| `GET /api/system/logs` | Téléchargement des événements en texte. |
| `PATCH /api/system` | Patch JSON des réglages ; sauvegarde validée avant modification de l’état. |
| `POST /api/system/pause` | Mise en pause et reset ASIC. |
| `POST /api/system/resume` | Reprise si aucun défaut matériel verrouillé. |
| `POST /api/system/restart` | Redémarrage après l’envoi de la réponse HTTP. |

Le nom configuré est annoncé au serveur DHCP (option 12) ; sa résolution DNS dépend du routeur.

Les mutations utilisent `Content-Type: application/json`. Les origines étrangères, hôtes inattendus, tailles excessives et requêtes ambiguës sont refusés. Le service vise le réseau local et ne fournit pas de compte utilisateur, TLS ou authentification distante.

## Fonctions d’ESP-Miner non portées

OTA et OTAWWW, Swarm, Stratum V2, TLS Stratum, pool de secours, mDNS, point d’accès de provisioning, scoreboard persistant, écran OLED et réglage des partitions de boot. La mise à jour se fait par USB. Les chemins OTA retournent explicitement HTTP 501 ; aucune interface ne simule leur réussite.

La limitation préexistante de calcul des midstates du BM1397 reste présente. La cible principale est la Gamma 601 / BM1370 avec version rolling matériel.

## Vérifications

```bash
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --manifest-path axeos-web/Cargo.toml
cargo clippy --locked --manifest-path axeos-web/Cargo.toml --all-targets -- -D warnings
cargo fmt --all -- --check
```

Tests : validation et atomicité des réglages, absence de secrets dans l’API, rejet des requêtes ambiguës et des mutations d’origine étrangère, records flash corrompus/incomplets, refus d’écraser NVS ou une partition chevauchante, rendu du contrat de données partagé, échappement HTML et courbes sans données fabriquées. Ils complètent les tests de minage et d’intégration BM1370 précédents.

Les compilations ne remplacent pas un essai sur carte : connexion HTTP/Wi-Fi réelle, persistance après coupure, mesures, commandes ASIC, redémarrage et shares acceptées doivent être vérifiés sur votre Gamma. Les interactions dans un navigateur graphique n’ont pas pu être testées avec les outils disponibles dans cette session.

### Résultats de cette livraison

- 41 tests réussis : 37 tests du workspace hôte et 4 tests de rendu/contrat du client Rust.
- Clippy strict réussi sur le workspace hôte, le client web et les trois variantes de firmware.
- Compilation WASM réussie ; le module de 160 364 octets est également accepté par le moteur WebAssembly de Node. Cette validation de format ne constitue pas un test d’interaction dans un navigateur.
- Ressources embarquées : 74 134 octets au total (WASM gzip, bindings gzip, CSS gzip et amorçage HTML/JS), sommes SHA-256 et décompression vérifiées.
- Compilations `--locked --release` réussies pour Gamma, Max et Ultra sur `xtensa-esp32s3-none-elf`.
- L’éditeur de liens Xtensa signale un segment LOAD RWX ; les liens aboutissent. Aucun essai sur carte ni flash réel n’a été effectué.
