# Vérifications de l’intégration AxeOS

- 46 tests du workspace Rust réussis : logique de minage, alimentation, stockage, HTTP, WebSocket et adaptateur officiel.
- Les tests de l’adaptateur vérifient les noms et types obligatoires du contrat `web/openapi.yaml`, les unités et l’absence de secrets.
- Les formulaires réseau, matériel et pool officiels sont exercés : commutateurs booléens/numériques, échappement JSON, mots de passe masqués, limites et refus atomique des fonctions absentes.
- Colonnes des graphiques, références temporelles en millisecondes, lacunes historiques et limites de taille contrôlées.
- Handshake WebSocket RFC 6455, clé invalide, origine, masquage, ping et réception fragmentée testés sur ordinateur.
- Clippy strict du workspace et du firmware Gamma réussi.
- Firmware compilé en release avec `--locked` pour Gamma, Ultra et Max. Pour Gamma, les sections chargées représentent environ 1,75 Mo ; la partition application fournie dispose de 4 128 768 octets.
- Import GitHub exécuté sur une copie isolée : nouveau commit préparé, aucun push. Un fichier manquant bloque l’import avant le clone ou le commit.
- Application Angular officielle compilée en production, client OpenAPI régénéré, notices de licence extraites.
- 174 fichiers source de l’interface comparés à la livraison précédente : aucune réécriture des écrans ou styles.
- 15 ressources de production embarquées, 596 257 octets gzip. Hachages, décompression, scripts HTML, feuilles de style, polices et licences vérifiés.
- Le compilateur du firmware utilise ces ressources de production ; l’ancienne interface interne Rust/WASM est retirée.
- ZIP accompagné d’un manifeste SHA-256 et d’un outil préparant les mises à jour dans une nouvelle copie Git, sans suppression dans le dépôt de travail de l’utilisateur ni push automatique.

La validation sur carte physique et dans un navigateur connecté au vrai firmware reste à effectuer. Aucun flash n’a été réalisé ici. Les tests de sérialisation/protocole et la compilation ne prouvent ni le bon fonctionnement des échanges TCP sur carte, ni un minage stable. Les fonctions encore absentes et les sentinelles des mesures indisponibles sont documentées dans `status.md`.

Le linker Xtensa peut émettre un avertissement de segment LOAD RWX ; les compilations aboutissent. Les journaux de compilation contiennent uniquement les identifiants de test `TestSSID` / `TestPASSWORD`, pas vos identifiants Wi-Fi.
