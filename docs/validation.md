# Vérifications de l’archive organisée

- 37 tests du workspace Rust réussis après déplacement des crates.
- 7 tests du client interne avec sa feature de simulation réussis après déplacement.
- Clippy strict du workspace réussi.
- Ressources internes WASM reconstruites depuis leur nouvel emplacement.
- Firmware Gamma compilé en release, avec `--locked`, depuis la nouvelle arborescence.
- Application Angular officielle compilée en mode démo avec génération du client OpenAPI depuis `web/openapi.yaml`.
- Chemins locaux des manifestes Cargo et des scripts vérifiés.
- Sources d’interface officielles comparées au commit amont ; écrans et styles conservés.
- Archive contrôlée : pas de métadonnées Git, dépendances téléchargées, caches, secrets d’hébergement ou chemins locaux de l’environnement.

Le linker Xtensa émet un avertissement de segment LOAD RWX ; le lien aboutit. Aucun flash ni essai sur une Gamma réelle n’a été effectué. Ces vérifications ne rendent pas l’API Rust compatible avec AxeOS officiel : le raccordement restant est décrit dans `status.md`.
