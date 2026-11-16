# État de l’intégration

| Élément | État |
| --- | --- |
| BM1370 / Gamma 601 | Pilote fourni, sélection bitaxe-gamma, 525 MHz / 1,15 V par défaut. Compilation vérifiée ; essai matériel restant. |
| TPS546 / EMC2101 | Initialisation, télémétrie, ventilation et protection thermique dans le firmware Rust. |
| Mineur Rust | Logique de jobs, vérification des nonces et Stratum V1 TCP. |
| Serveur HTTP Rust | API bornée, réglages persistants, commandes pause/reprise/redémarrage, journal. |
| Interface actuellement embarquée | Client Rust/WASM interne de web/embedded-rust, ressources dans firmware/web-assets. |
| Véritable interface AxeOS | Sources Angular officielles dans web/axeos ; démonstration officielle compilée et publiée. Écrans et styles non réécrits. |
| Connexion AxeOS officiel → firmware Rust | **Non réalisée.** Les modèles de réponses et certains chemins diffèrent. Le fait de compiler Angular ne connecte pas ces API. |
| OTA / Swarm / Stratum V2 / provisioning | Non implémentés dans le firmware Rust de ce dépôt. Leur présence visuelle dans AxeOS ne prouve pas leur fonctionnement côté firmware. |

## Travail restant pour embarquer AxeOS officiel

1. Raccorder les champs plats d’ESP-Miner à la télémétrie Rust : l’API Rust renvoie actuellement des objets `settings` et `telemetry` imbriqués.
2. Adapter les noms et formats des réglages, les statistiques et les informations ASIC selon `web/openapi.yaml`.
3. Prendre en charge les ressources Angular et leurs chemins dans le serveur embarqué ; actuellement les routes statiques ciblent le client Rust/WASM.
4. Implémenter les fonctions retenues du firmware ou répondre explicitement « non pris en charge » ; ne pas simuler un succès sur une vraie carte.
5. Tester le parcours complet sur Gamma 601 : réseau, alimentation, ventilation, jobs, shares et persistance.

Le mode mock d’AxeOS officiel est un aperçu visuel fourni par le projet amont. Les données affichées sont fictives et certaines actions n’y reproduisent pas un véritable périphérique.

La limitation préexistante des midstates BM1397 reste présente. La cible principale est la Gamma 601 avec version rolling matériel.
