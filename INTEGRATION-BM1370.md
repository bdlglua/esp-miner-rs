# BM1370 — Bitaxe Gamma 601

Cette intégration cible la **Bitaxe Gamma 601**, avec un BM1370, un TPS546D24A et un EMC2101. Elle ne constitue pas une validation sur matériel réel.

## Modifications

- Variante Cargo `bitaxe-gamma`, exclusive de `bitaxe-max` et `bitaxe-ultra`.
- Pilote `bm1370::BM1370` provenant de votre archive, embarquée sans modification dans `vendor/bm13xx-rs`. Les pilotes et le protocole utilisent tous ce même instantané local.
- Réglages de départ : 525 MHz, 1,15 V, UART à 1 000 000 bauds, difficulté ASIC 256 et version rolling matériel `0x1fff_e000`.
- Les 16 identifiants de jobs du BM1370 et ses réponses à 16 petits cœurs par cœur sont gérés par la bibliothèque fournie.
- Alimentation Gamma par PMBus à l’adresse `0x24`. Aucun accès au DS4432, à l’INA260 ou au GPIO10 dans cette variante.
- Profil TPS546 monophasé pour alimentation 5 V, fréquence de découpage 650 kHz, seuils de courant 25/30 A et limite VOUT_MAX resserrée à 1,25 V. Les réglages sont volatils ; aucune écriture dans la NVM du régulateur.
- Température via la diode externe du BM1370 : EMC2101 `0x4c`, facteur d’idéalité `0x24`, compensation bêta `0x00`, sans décalage logiciel.
- ASIC maintenu en reset pendant l’initialisation. Le minage attend la vérification de la tension réelle, du statut TPS546 et de la température. Coupure PMBus sur défaut de télémétrie, de diode, de commande ventilateur ou surchauffe ASIC au-dessus de 75 °C ; redémarrage manuel nécessaire.

Les broches sont celles de la 601 : SDA GPIO47, SCL GPIO48, UART TX GPIO17, RX GPIO18 et NRST GPIO1 actif bas.

## Compiler

Installer la chaîne Xtensa avec `espup install -t esp32s3`, puis charger le fichier d’environnement indiqué par espup.

Pour cette version avec AxeOS Rust, suivre d’abord [AXEOS-RUST.md](AXEOS-RUST.md), notamment l’installation de la table de partitions. Configurer ensuite le pool et votre adresse/worker depuis l’interface web.

Depuis le dossier du projet :

```bash
cd firmware
WIFI_SSID="votre_reseau" WIFI_PASSWORD="votre_mot_de_passe" \
  cargo build --locked --release --features=bitaxe-gamma
```

Ou modifier les identifiants Wi-Fi du `justfile` puis lancer `just build gamma`.

Pour compiler/flasher avec le runner configuré : `just run gamma`. Le runner actuel est `probe-rs` avec logs RTT ; adapter `firmware/.cargo/config.toml` à votre outil de flash si nécessaire. Sélectionner une seule variante de carte.

## Vérifications sur ordinateur

```bash
cargo test --locked
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

Les tests couvrent les conversions PMBus, l’identité et le mode de tension du régulateur, le refus des tensions invalides, la relecture OPERATION, la télémétrie et l’absence d’écriture NVM/d’activation pendant la configuration. Un test avec UART simulée exécute l’énumération, l’initialisation, le changement de baudrate, le reset des cœurs, le réglage à 525 MHz, le version rolling, le bouclage des jobs et la réception d’un nonce BM1370.

## Validation matérielle restante

Vérifier sur votre Gamma 601 le démarrage à 1,15 V, les températures, le ventilateur, puis la réception de shares acceptées par votre pool. Les tests simulés et la compilation ne prouvent pas le débit de hachage ni la précision des mesures réelles. Le débit annoncé dans les logs est théorique.

Le profil Max conserve la limitation préexistante de calcul des midstates dans `bm13xx-chain`. Cette intégration concerne le BM1370 avec version rolling matériel.

## Références des paramètres

- [Configuration Gamma 601 ESP-Miner](https://github.com/bitaxeorg/ESP-Miner/blob/master/configs/config-601.csv)
- [Configuration des cartes et calibration EMC2101](https://github.com/bitaxeorg/ESP-Miner/blob/master/main/device_config.h)
- [Profil TPS546_CONFIG_DEFAULT et PMBus](https://github.com/bitaxeorg/ESP-Miner/blob/master/main/power/TPS546.c)

Paramètres consultés le 16 novembre 2026 ; le code Rust du pilote ASIC reste celui de votre archive.

## Résultats de l’intégration BM1370 initiale (avant AxeOS)

- 29 tests réussis : 21 tests de logique de minage, 6 tests d’alimentation PMBus et 2 tests BM1370.
- `cargo clippy --all-targets --all-features -- -D warnings` réussi sur le workspace hôte.
- Compilations `--locked --release` pour `bitaxe-gamma`, `bitaxe-max` et `bitaxe-ultra` réussies sur la cible `xtensa-esp32s3-none-elf`.
- Clippy `--locked --release --features=bitaxe-<variante> -- -D warnings` réussi pour les trois variantes.
- Formatage vérifié pour le workspace et le firmware.
- Outils : Rust stable 1.100.0 pour les tests hôtes ; Rust ESP 1.97.0-nightly (1.97.0.0) pour Xtensa.

Le linker Xtensa émet un avertissement concernant un segment ELF LOAD avec permissions RWX ; les éditions de liens aboutissent. Aucune carte n’a été flashée et aucun test de pool réel n’a été exécuté.
