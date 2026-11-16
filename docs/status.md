# État de l’intégration

Le firmware sert maintenant **la véritable application Angular AxeOS**, compilée en production depuis les sources officielles du commit indiqué dans `upstream.json`. Les écrans, styles, icônes et polices sont conservés. Le serveur et l’adaptation de l’API sont écrits en Rust. L’ancienne interface Rust/WASM a été retirée de cette livraison.

| Fonction | État |
| --- | --- |
| BM1370 / Gamma 601 | Pilote fourni ; 525 MHz et 1,15 V par défaut. Compilation vérifiée ; essai matériel restant. |
| TPS546 / EMC2101 | Initialisation, mesures, ventilation et arrêt en cas de défaut thermique. |
| AxeOS officiel embarqué | Ressources gzip servies depuis la flash, sans fichiers à héberger ailleurs. |
| Tableau de bord et système | Réponses plates aux noms et unités attendus par AxeOS. Mesures disponibles issues du firmware. |
| Actualisation et journaux | WebSocket `/api/ws/live`, WebSocket `/api/ws`, téléchargement des journaux. Deux flux simultanés maximum ; deux connexions HTTP restent disponibles. |
| Réseau | SSID, mot de passe et nom DHCP persistants ; redémarrage requis. Configuration Wi-Fi initiale à la compilation. |
| Pool | Un pool Stratum V1 TCP ; adresse, port, worker et mot de passe persistants. Les changements prennent effet après redémarrage. |
| Matériel | Fréquence, tension, ventilation automatique ou manuelle persistantes. Fréquence limitée à 525 MHz ; Gamma : 1000–1250 mV. |
| Commandes | Pause, reprise et redémarrage raccordés au firmware. La reprise refuse un défaut matériel verrouillé. |
| Graphiques | Historique de 60 échantillons à 5 secondes : hashrate, puissance et température ASIC. Colonnes sans historique disponibles sous forme de lacunes. |
| OTA, partition de démarrage, identification, scoreboard, scan Wi-Fi | Non implémentés : erreur explicite, aucun faux succès. Mise à jour par USB. |
| OLED, NTP, SV2, TLS, pools multiples, PID personnalisable, Swarm | Non implémentés. Les modifications correspondantes sont refusées ; les champs inchangés des formulaires complets restent acceptés. |

## Valeurs indisponibles

AxeOS attend des nombres et utilise notamment `toFixed()`. Le serveur utilise donc les sentinelles numériques compatibles : `-1` pour une température indisponible, `0` pour les autres nombres indisponibles. Le champ supplémentaire `unavailableFields` distingue ces sentinelles des mesures réelles. Les capacités absentes sont annoncées dans `capabilities`. Les écrans officiels n’exploitent pas encore ces extensions ; un zéro affiché n’est donc pas toujours une mesure.

Le hashrate moyen est estimé à partir des nonces vérifiés et de la difficulté ASIC, sur une fenêtre allant jusqu’à une minute. Les moyennes de 10 minutes/1 heure, statistiques par domaine, temps de réponse du pool, CPU, RSSI, données de bloc et statistiques cumulées entre redémarrages ne sont pas mesurés. `bestDiff` reflète la session, sans historique persistant des records. La ventilation utilise une courbe fixe 30–100 %, avec protection indépendante à 75 °C ; ce n’est pas le PID d’ESP-Miner.

## Validation matérielle restante

Aucun flash ni essai sur une Gamma physique n’a été effectué ici. Vérifier sur la carte : connexion Wi-Fi, accès par son IPv4 DHCP, températures, tension et ventilation, jobs Stratum et shares acceptées, sauvegarde puis redémarrage, pause/reprise et journaux. La compilation et les tests de protocole sur ordinateur ne prouvent pas un minage stable sur matériel.

Le mode mock officiel reste accessible pour un aperçu visuel sur ordinateur. Une compilation `--demo` ne remplace jamais les ressources de production du firmware. La limitation préexistante des midstates BM1397 reste présente ; la cible principale est Gamma 601.
