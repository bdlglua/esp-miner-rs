# Client web Rust interne

Ce module est conservé dans `web/embedded-rust` pour maintenir le firmware existant compilable. **Ce n’est pas l’interface officielle AxeOS** : celle-ci se trouve dans `web/axeos`.

Les routes de `firmware/src/web.rs` servent actuellement les ressources gzip de `firmware/web-assets`. Le client interne lit des réponses JSON contenant `settings` et `telemetry`. L’application Angular officielle attend d’autres structures définies dans `web/openapi.yaml` ; il reste à adapter ces contrats et les chemins des ressources avant de la substituer au client embarqué.

## Fonctions actuellement raccordées

- Télémétrie d’alimentation et de température, ventilation, compteurs de nonces et de shares.
- Historique en RAM et 32 événements applicatifs récents.
- Réglages persistants dans deux secteurs de la partition `axe_cfg`, avec génération et CRC32.
- Pause/reprise de la chaîne ASIC et redémarrage du microcontrôleur.

Le réseau, le pool, la fréquence et la tension enregistrés sont appliqués après redémarrage. La ventilation s’applique au cycle suivant. Les mots de passe ne sont pas renvoyés par les endpoints de consultation.

Le client interne possède aussi une feature `demo` de test en mémoire ; elle est désactivée par défaut. Pour l’aperçu fidèle d’AxeOS, utiliser le mode mock officiel de l’application Angular, pas ce module interne.

## Reconstruction

Avec Rust stable, la cible `wasm32-unknown-unknown`, Python 3.11+ et wasm-bindgen-cli correspondant au lockfile :

```bash
python scripts/build-embedded-web.py
```

Ce script reconstruit uniquement les ressources du client interne. `scripts/build-axeos.py` compile séparément les véritables sources Angular.
