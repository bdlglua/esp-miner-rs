# Adaptateur AxeOS officiel

Le contrat amont est conservé dans `web/openapi.yaml`. `crates/axeos-core/src/official.rs` adapte les modèles Rust internes à ce contrat, sans modifier le client Angular.

| Requête | Comportement |
| --- | --- |
| `GET /` et ressources Angular | Interface officielle gzip, polices et licences embarquées. |
| `GET /api/system/info` | Objet plat : `ASICModel`, `hashRate`, `coreVoltage`, `stratumURL`, `pools`, etc. Tensions en mV, courant en mA, puissance en W, hashrate en GH/s. |
| `GET /api/system/asic` | Gamma/BM1370 et choix de fréquence/tension dans les limites validées. Également adapté pour Max/Ultra. |
| `GET /api/system/statistics?columns=power,hashrate,asicTemp` | `currentTimestamp`, `labels`, `statistics`. Ordre demandé conservé, `timestamp` ajouté à la fin, temps en millisecondes depuis le démarrage. Les valeurs historiques inconnues sont `null`. |
| `PATCH /api/system` | Formulaires réseau, matériel et pool officiels ; validation atomique puis stockage à deux emplacements en flash. Réponse avec `restartRequired`. |
| `POST /api/system/restart` | Répond puis redémarre. |
| `POST /api/system/pause` et `/resume` | Pilotage réel de la pause ; aucun contournement des défauts matériels. |
| `GET /api/system/logs` | Journal texte téléchargeable. |
| WebSocket `/api/ws/live` | Messages JSON `{ "event": "update", "data": { ... } }` toutes les trois secondes. |
| WebSocket `/api/ws` | Trames texte du journal ; ping/pong et fermeture. |
| `GET /api/rust/system/info`, `/api/system/events` | Diagnostic interne conservé, hors contrat officiel. |

## Réglages et mots de passe

`wifiPass`, `stratumPassword` et le tableau `pools` de la véritable interface sont reconnus. Le pool unique occupe le slot 0, utilisé pour les deux indices de sélection ; aucune bascule vers un second pool n’est annoncée. Les mots de passe masqués `*****` ou absents préservent les secrets enregistrés. Une chaîne vide explicite efface le mot de passe. Aucun secret n’est renvoyé par l’API de lecture.

Les champs booléens des formulaires Angular sont acceptés, ainsi que les valeurs numériques 0/1 prévues par OpenAPI. Les champs inconnus, valeurs hors limites et demandes contradictoires sont refusés. Les réglages de fonctions absentes sont refusés en HTTP 501 ; leurs valeurs par défaut inchangées sont acceptées pour permettre la sauvegarde d’un formulaire complet.

Les changements réseau, pool, fréquence et tension nécessitent un redémarrage. La ventilation est lue pendant l’exécution. Le nom de machine sert au DHCP ; mDNS n’est pas implémenté. Utiliser l’IPv4 attribuée par le routeur.

## Transport

Requêtes bornées : en-têtes 1024 octets, corps JSON 2048 octets, délai total de réception 15 secondes. Pas d’uploads chunked ni d’OTA. Les uploads volumineux sont rejetés par le parseur avant leur traitement (413/415). Les opérations modifiantes et les upgrades WebSocket vérifient l’hôte et l’origine. Quatre connexions HTTP sont disponibles, dont au maximum deux réservées aux flux WebSocket.

Les fichiers sont compressés à la compilation et envoyés directement depuis la flash par morceaux. La CSP autorise les styles intégrés nécessaires à Angular, la base `/`, les polices locales et les connexions WebSocket ; les scripts proviennent de la carte.
