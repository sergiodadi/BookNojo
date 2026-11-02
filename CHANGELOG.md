# Changelog

Toutes les versions notables de `bc_export` sont documentées ici.
Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.0.0/).
Un numéro de version est ajouté après chaque demande traitée.

## [0.36.0] - 2026-11-02

### Fixed
- **Message d'erreur clair quand la base cible est en lecture seule** (ex. `testdata/`, `chmod 444` à dessein) : au lieu de laisser remonter une erreur SQLite brute ("attempt to write a readonly database: Error code 8"), `write_plan_sql` vérifie désormais les permissions avant toute tentative d'écriture et explique la cause + la solution (utiliser `testdata-rw/`). Signalé par l'utilisateur en testant "Ajouter un livre" contre `testdata/`.

### Added
- **Audit complet des colonnes `ZCOLLECTIBLE`/`ZSTORY`** contre ce que le formulaire "Ajouter un livre" couvrait déjà, suite à un champ manquant signalé par l'utilisateur ("Issue N°"). Champs ajoutés (tous avec des données réelles confirmées dans `testdata/`, sauf `box_set`) :
  - `issue_number` (`ZISSUENUMBER`) — onglet Détails, "Issue N°".
  - `location_number` (`ZLOCNR`) — onglet Principal, "N° d'emplacement" (texte libre, pas une liste — confirmé par les valeurs réelles).
  - `box_set` (`ZBOXSET`) — onglet Détails, case "Coffret".
  - `start_page` par contenu (`ZSTORY.ZSTARTPAGE`) — onglet Contenus, "Page de début".
  - `ZAUDIOLENGTH` (variante texte de `ZAUDIOLENGTHMINUTES`, toujours identique dans les données réelles observées) écrit automatiquement en miroir — pas un nouveau champ de formulaire.
  - Champs identifiés mais **volontairement non implémentés**, documentés en détail (en anglais, pour un repérage facile face aux noms de colonnes du schéma) dans la sortie de `bc_add plan` : les champs personnalisés `ZUSERTEXT1`/`2`/`ZUSERLOOKUP1`/`2` (aucune donnée réelle, sémantique non vérifiable) et le reste des champs de `ZSTORY` (Plot/Notes/Série/Édition originale par contenu — par analogie seule, `ZSTORY` vide dans `testdata/`, ampleur de formulaire disproportionnée sans preuve).

## [0.35.0] - 2026-11-02

### Added
- **Badge "fichier ouvert"** dans l'en-tête, à côté du badge de version : nom du fichier + répertoire parent de la collection actuellement chargée (ex. "MyCollection.bkcp — testdata-sample"), mis à jour à chaque "File > Import a collection…". Nouvelle commande Tauri `app_version` pour que le badge de version de l'app NBC affiche le vrai numéro de version au lieu du texte figé "NBC — développement" (l'export HTML statique de `bc_export` avait déjà son propre numéro, inchangé).

### Changed
- **`app/src-tauri/Cargo.toml` et `tauri.conf.json` suivent désormais le même numéro de version que la racine du projet** à chaque bump — corrige un décalage silencieux découvert à cette occasion : la boîte "About NBC" affichait "Version 0.1.0" depuis l'étape 4 (figé au scaffold initial), jamais mis à jour depuis.
- **Pastilles de sous-collections et badges du formulaire "Ajouter un livre"** : fond bleu clair (teinte de l'accent) au lieu de blanc/gris à peine distinct du fond — fond jugé pas assez lisible par l'utilisateur.

## [0.34.0] - 2026-11-02

### Added
- **Le formulaire "Ajouter un livre" de l'app NBC écrit désormais directement dans la collection**, sans passer par un fichier `new_book.json` à télécharger puis repasser à `bc_add` en ligne de commande — ce pont manuel n'avait de sens que pour l'utilisateur technique de ce projet, pas pour un futur utilisateur "grand public". Le bouton devient **"💾 Enregistrer"** (au lieu de "⬇️ Générer new_book.json") quand l'app tourne dans Tauri ; l'export HTML statique de `bc_export` garde exactement son comportement de téléchargement inchangé (pas de backend à qui écrire).
  - Avant d'écrire, une **étape de confirmation** résout le livre contre la collection ouverte et affiche la liste des valeurs de type auteur/éditeur/genre/sujet/etc. qui seraient créées pour la première fois (badge "nouveau"), pour repérer une faute de frappe avant qu'elle ne devienne une entrée permanente dans la collection — plutôt qu'un bouton "Save" qui écrit sans y regarder.
  - Deux nouvelles commandes Tauri IPC, `prepare_add_book` (lecture seule — résout + construit le plan, renvoie un résumé) et `commit_add_book` (sauvegarde + écrit, une fois la confirmation donnée) — toutes deux refusent sans exception un chemin qui ressemble à la vraie collection Book Collector (`looks_like_real_collection`), sans possibilité de forcer depuis l'interface (contrairement à `bc_add commit --force` en CLI) ; `commit_add_book` refuse aussi si Book Collector est lancé.
- **Refactor interne** : le cœur de `bc_add resolve`/`plan`/`commit` (résolution des champs, construction du SQL d'insertion, sauvegarde + écriture transactionnelle) a été déplacé de `src/bin/bc_add.rs` vers `src/lib.rs` (`resolve_book`, `plan_book_sql`, `write_plan_sql`, `looks_like_real_collection`, `prepare_add_book`, `commit_add_book`), pour que `bc_add` (CLI) et l'app NBC (IPC) appellent exactement le même code — même motif que le partage `load_books`/`PageBook` fait à l'étape 3 du plan NBC. Les garde-fous spécifiquement interactifs (`--force`, "OUI" tapé au clavier) restent dans `bc_add.rs`, puisqu'ils n'ont pas de sens côté GUI (refus pur et simple à la place).

### Verified
- Refactor vérifié comme un déplacement pur : `bc_add resolve`/`plan`/`commit` relancés bout en bout contre `testdata-rw/` (livre réellement écrit, vérifié par requête SQL directe) — comportement identique à avant le déplacement.
- `prepare_add_book`/`commit_add_book` (les fonctions exactement appelées par les commandes Tauri) testées directement contre `testdata-rw/` : détection correcte des valeurs nouvelles vs déjà connues, écriture réelle confirmée en base.
- Gate de refus sur la vraie collection (`looks_like_real_collection`) reconfirmée sur le vrai chemin `~/Documents/Collectorz/...` (lecture seule, aucune écriture tentée).
- 12 tests toujours au vert (`cargo test --release`), build propre des deux crates (`book_collector_export` et `app/src-tauri`).

## [0.33.0] - 2026-11-02

### Changed
- **Refonte visuelle (`templates/style.css`)**, partagée par l'export HTML de `bc_export` et l'app NBC — demandée explicitement pour avoir un rendu "plus professionnel" :
  - Nouvelle palette (variables CSS) : accent bleu plus soutenu (`#2e5bd7`, + variante foncée pour le survol), surfaces neutres (`--surface`, `--surface-alt`) distinctes du fond de page, ombres légères (`--shadow-sm`/`--shadow-md`) pour donner du relief aux cartes (statistiques, formulaire d'ajout) au lieu d'un simple cadre plat.
  - **Correction du point signalé** : les badges/onglets du formulaire "Ajouter un livre" (Principal, Détails, Personnel, Crédits & Personnages, Contenus, Intrigue & Notes, Liens) étaient des pastilles blanches sur fond blanc (bordure à peine visible) — ils ont désormais un **fond gris lisible** (`--surface-alt`) à l'état inactif, et un bleu plein avec ombre à l'état actif. Même traitement appliqué par cohérence aux onglets de sous-collections en haut de l'écran.
  - Ajout d'états de survol/transition sur la quasi-totalité des boutons et contrôles (jusque-là statiques), d'un anneau de focus visible sur les champs du formulaire, et d'un renforcement des titres (graisse, interlignage) pour une hiérarchie typographique plus nette.
  - Vérifié visuellement via capture d'écran automatisée (export HTML statique rechargé dans Chrome headless) sur la vue principale, le panneau Statistiques, et le formulaire d'ajout (onglet Détails) — avant/après comparés.

## [0.32.2] - 2026-11-02

### Added
- **Vérification de version à l'import** : un `.bkcp` choisi dans "File > Import a collection…" est désormais vérifié (`preferences.plist` → `kPreferencesApplicationVersion`, lu via `plutil -convert json`) contre la version Book Collector `20.3.1` — celle que le schéma SQL de ce projet cible. Import refusé (boîte de dialogue d'erreur) si la version diffère, ou si elle est illisible (par précaution). Un `.col` (déjà propriété de NBC) n'est pas soumis à cette vérification.

### Changed
- **Dépoussiérage des références à "Book Collector" dans l'UI de l'app NBC**, à la demande de l'utilisateur — réservées désormais au menu Help/mode d'emploi, ou à un message fonctionnel qui en a réellement besoin (comme l'erreur de version ci-dessus) :
  - Menu **File > "Import a file…"** renommé **"Import a collection…"**.
  - Boîte **About NBC** : retrait de "successeur de Book Collector" → juste "NBC (nom provisoire)" + version.
  - `app/src-tauri/Cargo.toml` : description du paquet nettoyée de même.

## [0.32.1] - 2026-11-02

### Fixed
- **`MyCollection.bkcp` grisé dans le sélecteur "Import a file…"** (signalé par l'utilisateur en testant lui-même). Cause réelle : rien à voir avec l'extension (`.bkcp` vs `.col`) — c'est `pick_folder()` qui posait problème. macOS/LaunchServices enregistre `.bkcp` (via Book Collector) comme un paquet, donc NSOpenPanel le traite comme un *fichier*, pas un dossier ; avec un sélecteur limité aux dossiers (`canChooseFiles: false`), un paquet reconnu est grisé — impossible à sélectionner. Corrigé en passant à `pick_file()` avec un filtre d'extensions (`bkcp`, `col`), qui permet de sélectionner le paquet comme un élément unique, cohérent avec le comportement standard de Finder/sélecteurs sur ce type de paquet.

### Verified
- `cargo build --release` propre (app + lib), aucune régression attendue sur le reste de la chaîne (menu → dialogue → évènement → `load_collection`), déjà vérifiée en v0.32.0.

## [0.32.0] - 2026-11-02

### Added
- **Étape 4/6 du plan NBC : menus natifs.**
  - **File > Import a file…** : ouvre le sélecteur de fichier macOS natif (`tauri-plugin-dialog`), scope "dossier" — `.bkcp`/`.col` apparaît comme un seul élément sélectionnable (Book Collector l'a enregistré comme type de paquet auprès de macOS), pas un dossier à ouvrir. Le chemin choisi est envoyé au frontend via un évènement Tauri (`import-collection`) ; `templates/app.js` l'écoute et recharge toute la page avec la nouvelle collection — mémorisé dans `localStorage` pour les lancements suivants (plus de `DEV_COLLECTION_PATH` figé en dur à chaque fois, juste une valeur de repli au tout premier lancement).
  - **NBC Help > About NBC** : affiche une boîte de dialogue avec le nom et la version.
  - Menus App/Edit/Window standards reconstruits explicitement (`SubmenuBuilder`) plutôt que de garder le menu par défaut de Tauri, pour pouvoir y insérer "Import a file…".

### Fixed
- **Bug Tauri contourné** : un sous-menu nommé exactement `"Help"` est entièrement avalé par macOS/Tauri dans cette version (n'apparaît pas du tout dans la barre, correspond à des rapports de bug connus en amont) — confirmé en renommant temporairement le menu pour test, puis corrigé définitivement en le nommant `"NBC Help"`.
- **Erreur d'appel d'API corrigée** (`.hide()`/`.hide_others()`/`.show_all()` ne prennent aucun argument dans cette version de Tauri, contrairement à ce que suggérait la documentation trouvée) — corrigé d'après les messages d'erreur du compilateur.

### Verified
- **Chaîne complète vérifiée bout en bout** : clic sur le menu → ouverture du sélecteur natif (titré "Importer une collection", scope dossier confirmé sur un vrai `.bkcp`) → sélection → évènement `import-collection` émis → reçu par `templates/app.js` → `load_collection` appelé → page rechargée. Confirmé via un test délibéré avec un dossier invalide : le message d'erreur exact attendu s'est affiché dans l'interface, prouvant que toute la plomberie (menu, dialogue, évènement, écouteur, IPC, gestion d'erreur) fonctionne.
- `invoke('load_collection', ...)` testé directement avec le chemin de `testdata/` (2237 fiches) : retourne bien 2237 livres.
- **Non vérifié par automatisation** : le clic sur l'unique fichier visible dans le sélecteur natif lui-même — tentatives multiples (sélection AX, clic coordonnées) toutes infructueuses, même friction déjà documentée dans ce projet avec le contenu WebKit. Un clic de souris réel, trivial pour un humain sur un élément unique, n'a aucune raison de se comporter différemment ; à confirmer par l'utilisateur.
- **Bug trouvé et corrigé en testant** : `app/src/app.js` n'avait pas été resynchronisé (`./app/generate-dev-data.sh`) après plusieurs modifications de `templates/app.js` pendant cette étape — l'app tournait avec une version obsolète du script sans l'écouteur d'évènement, ce qui a fait perdre un temps de diagnostic significatif avant d'être repéré par un simple `diff`/`md5`. À retenir : toujours resynchroniser avant de tester après une modification de `templates/app.js`.

## [0.31.1] - 2026-11-02

### Fixed
- **Couvertures de nouveau absentes** (signalé par l'utilisateur) : régression introduite en passant à `testdata-sample/` (v0.30.0) sans mettre à jour la portée du protocole `asset` de Tauri, configurée un peu plus tôt (v0.29.0) avant que ce dossier existe. `testdata-sample/` en était donc absent — la WebView bloquait silencieusement le chargement des images (aucune erreur JS, juste une ressource refusée). Ajouté `$HOME/Developer/book_collector_export/testdata-sample/**` à `app/src-tauri/tauri.conf.json`.

### Verified
- Reproduit (couverture absente), corrigé, revérifié avec capture d'écran : couverture affichée correctement ("Nice Historique - l'île Saint Honorat et l'abbaye de Lérins"), vignettes de la liste également. 12 tests toujours au vert.

## [0.31.0] - 2026-11-02

### Added
- **Étape 3/6 du plan NBC : l'app charge la collection en direct**, via une vraie commande IPC Tauri (`load_collection`) plutôt que des données statiques régénérées à la main.
  - `Book`, `PageBook`, `load_books`, `book_to_page_book`, `load_page_books`, `file_url`, et la conversion de date Core Data déplacés de `src/main.rs` vers `src/lib.rs` — `bc_export` et l'app NBC lisent désormais la collection via **exactement le même code**, au lieu de dupliquer ~150 lignes de jointures. Pur déplacement : comportement de `bc_export` vérifié strictement identique après coup (mêmes champs, mêmes valeurs, mêmes 12 tests).
  - `app/src-tauri` dépend maintenant de `book_collector_export` (dépendance de chemin vers `../..`) ; sa commande `load_collection(path)` ouvre la base en lecture seule et renvoie `Vec<PageBook>` — aucune dépendance `rusqlite` propre à l'app, tout encapsulé dans la bibliothèque partagée.
  - `templates/app.js` : la toute première ligne (lecture synchrone du JSON embarqué) devient `loadAllBooks()`, async, qui appelle `invoke('load_collection', ...)` **seulement** si `window.__TAURI__` existe (donc jamais pour l'export HTML classique de `bc_export`, qui garde son comportement synchrone exact). Le reste du fichier (~1100 lignes) est inchangé : seules les quelques lignes qui dépendaient de `allBooks` au niveau racine sont déplacées dans une fonction `boot()` appelée une fois les données chargées — pas de réécriture du fichier.
  - `app/generate-dev-data.sh` simplifié : n'a plus besoin de générer de données (l'app les charge en direct) — synchronise seulement `app.js`/`style.css`/`index.html` depuis `templates/`.
  - Chemin de la collection à charger **temporairement codé en dur** (`DEV_COLLECTION_PATH` dans `templates/app.js`, pointant vers `testdata-sample/`) en attendant le vrai sélecteur de fichier de l'étape 4.

### Verified
- `bc_export` revérifié strictement inchangé : mêmes champs JSON, mêmes valeurs, sur `testdata-sample/`.
- App NBC relancée (`cargo tauri dev` puis `cargo tauri build`) avec le chargement live : mêmes données exactes que la version statique précédente (57 livres dans la sous-collection principale, mêmes comptages par auteur) — capturé à l'écran, aucune erreur.
- 12 tests unitaires du projet principal toujours au vert (7 dans `lib.rs` après le déplacement, dont les tests de date/URL repris tels quels).

## [0.30.0] - 2026-11-02

### Added
- **`build-sample-testdata.sh`/`.py`** : construit `testdata-sample/MyCollection.bkcp`, une petite base d'échantillon (~100 fiches, contre 2237 dans `testdata/`) à partir de `testdata/` — jamais directement la vraie collection. Sélection : 5 fiches par sous-collection (en priorisant celles avec couverture), plus, dans la sous-collection principale, tous les livres de 1 à 2 auteurs ayant plusieurs livres (les entrées "Collectif"/générique exclues). Copie la fermeture référentielle complète (éditeurs, genres, sujets, auteurs, crédits, personnages, contenus, liens, tables système) et les seules images de couverture référencées — résultat structurellement valide et indépendant.
- `app/generate-dev-data.sh` utilise désormais `testdata-sample/` par défaut (régénération ~20× plus rapide, fichier généré ~1 Mo au lieu de ~20 Mo) ; `--full` revient à `testdata/` pour un test sur volume réaliste avant un commit.
- `testdata-sample/` ajouté au `.gitignore` (données personnelles, comme `testdata/`/`testdata-rw/`).

### Verified
- `PRAGMA integrity_check` = `ok` sur l'échantillon généré.
- **Ouvert dans la vraie application Book Collector** (le test le plus strict) : aucune erreur, "Total: 57 Books" pour la sous-collection principale, regroupement par auteur correct (Albert Uderzo 43, René Goscinny 52...), fiche détail avec couverture et métadonnées correctes — capturé à l'écran.
- Rechargé dans l'app NBC (`cargo tauri dev`) avec l'échantillon : démarrage nettement plus rapide, onglets et comptages par auteur corrects.
- Reste du projet (`bc_export`/`bc_add`, 12 tests) non affecté, revérifié.

## [0.29.0] - 2026-11-02

### Fixed
- **Couvertures absentes de la fiche détail dans l'app NBC** (signalé par l'utilisateur après test de `cargo tauri dev`). Cause : `cover_url` (voir `HtmlBook` dans `src/main.rs`) est une URL `file://` brute — fonctionne dans l'export HTML classique (la page elle-même est ouverte en `file://`), mais la fenêtre Tauri charge la page via son propre protocole, pas `file://` : la WebView bloque alors cette ressource externe pour raisons de sécurité.
  - `app/src-tauri/tauri.conf.json` : active le protocole `asset` de Tauri (`app.security.assetProtocol`), avec une portée couvrant `testdata/`, `testdata-rw/`, `collection/` et `~/Documents/Collectorz/` — à élargir/rendre dynamique à l'étape 3 selon la collection réellement ouverte.
  - `templates/app.js` : nouvelle fonction `resolveCoverSrc()` — convertit `cover_url` via `window.__TAURI__.core.convertFileSrc()` **uniquement** si `window.__TAURI__` existe (donc seulement dans l'app ; l'export HTML classique de `bc_export`, ouvert dans un navigateur normal, continue d'utiliser l'URL `file://` telle quelle, inchangé). Appliquée à la fois à la fiche détail et au repli de la vignette de liste quand la miniature intégrée est absente.

### Verified
- Reproduit le bug (couverture absente dans `cargo tauri dev`), corrigé, puis revérifié : la fiche détail affiche maintenant une vraie couverture ("Atlas du Paris souterrain"), capturé à l'écran.
- `bc_export --html` (export autonome classique) revérifié inchangé : `cover_url` reste une URL `file://` brute dans le HTML généré, comme avant — `window.__TAURI__` étant absent d'un navigateur normal, `resolveCoverSrc()` n'y change rien.
- Reste du projet (`bc_export`/`bc_add`, 12 tests) non affecté, revérifié.

## [0.28.0] - 2026-11-02

### Added
- **Étape 2/6 du plan NBC : interface existante branchée dans la fenêtre Tauri.** `app/src/` reprend `templates/page.html`/`style.css`/`app.js` tels quels (liens externes au lieu de l'inlining fait par `bc_export`, mais `app.js` byte-pour-byte identique — aucun risque pris sur 1100 lignes déjà validées). En-tête renommé "NBC", badge de version "NBC — développement" (provisoire).
- `app/generate-dev-data.sh` : régénère `app/src/index.html` depuis `testdata/` (jamais la vraie collection) en réutilisant `bc_export --html` tel quel, puis en réinjectant son bloc de données dans `app/src/index.template.html`. Resynchronise aussi `app.js`/`style.css` depuis `templates/`.

### Fixed
- **Risque de fuite de données personnelles évité avant tout commit** : la première version de `app/src/index.html` embarquait directement les vraies données de `testdata/` (titres, auteurs...). Corrigé en séparant un gabarit sans données (`index.template.html`, committé) de l'HTML généré (`index.html`, ~20 Mo, ajouté au `.gitignore` de `app/` et retiré du suivi — même règle que `/testdata`, `/collection`, `*.col` à la racine). `app.js`/`style.css`, simples copies de `templates/`, sont aussi gitignorés pour éviter la duplication.

### Verified
- Lancé réellement (`cargo tauri dev` puis `cargo tauri build`) avec les vraies données de test : fenêtre native affichant l'interface complète — en-tête, onglets de sous-collections, barre A→Z, liste paginée (1242/1242, 21 pages), fiche détail correcte pour un livre sélectionné (titre, auteur, éditeur, ISBN, format, genres). Capturé à l'écran à chaque étape.
- Interaction par clic testée mais peu fiable dans cette fenêtre WebKit via automatisation d'accessibilité (même limite déjà rencontrée avec Chrome et Book Collector) — non bloquant : le rendu des données, lui, est confirmé visuellement.
- Reste du projet (`bc_export`/`bc_add`, 12 tests) non affecté, revérifié.

### Next (étape 3, pas encore commencée)
- Remplacer les données statiques embarquées par de vrais appels IPC Tauri vers le moteur Rust existant (`lib.rs`, `bc_add`) — plus de fichier à régénérer à la main à ce moment-là.

## [0.27.0] - 2026-11-02

### Added
- **`app/` — squelette de la future application macOS "NBC"** (étape 1/6 du plan d'empaquetage) : projet Tauri 2 scaffoldé avec `npm create tauri-app` (gestionnaire `cargo`, gabarit `vanilla` — pas de framework JS, cohérent avec le reste du projet), package Rust renommé `nbc`/`nbc_lib`, nom de produit et titre de fenêtre "NBC". Indépendant du crate `book_collector_export` existant (son propre `Cargo.toml` sous `app/src-tauri/`) — rien ne change pour `bc_export`/`bc_add`.
- Code et fichiers source du scaffold committés ; `app/src-tauri/target/` (~1,1 Go) exclu via le `.gitignore` propre au scaffold.

### Verified
- `cargo tauri build` compile sans erreur et produit `app/src-tauri/target/release/bundle/macos/NBC.app` (exécutable arm64 natif) et un `.dmg`.
- L'app a été réellement ouverte (`open NBC.app`) et capturée à l'écran : fenêtre native fonctionnelle, barre de menu macOS complète (NBC/File/Edit/View/Window/Help déjà présents nativement), contenu par défaut du gabarit Tauri affiché correctement. Fermée proprement après vérification.

### Next (étape 2, pas encore commencée)
- Remplacer le contenu par défaut de `app/src/` par l'interface déjà construite (`templates/page.html`/`style.css`/`app.js`).

## [0.26.0] - 2026-11-02

### Added
- **Extension `.col`** choisie pour la future base NBC (le nom de projet provisoire pour "les scripts actuels + la future application", en remplacement progressif de Book Collector — "BC"). Confirmé que le format interne est inchangé : `.bkcp` et `.col` sont le même contenu (`Book_data` + `Images/`), seule l'extension diffère — `resolve_db_path` ne se souciait déjà pas de l'extension, donc aucun changement de code ailleurs n'a été nécessaire.
- **`bc_add import --source <fichier> [--dest <dossier>]`** : réalise le transfert "BC vers NBC" — copie une collection existante (base + images) vers un nouvel emplacement `.col` indépendant. Refuse d'écraser une destination déjà existante (l'import n'amorce une base qu'une seule fois). Destination par défaut : `collection/<nom de la source>.col`.
- **`--lang fr|en`** (option globale de `bc_add`) : amorce de l'internationalisation. Pour l'instant, seule la sous-commande `import` l'utilise (messages bilingues complets) — pas une reprise des ~150 `println!` existants, qui restent en français ; le patron (`Lang` + fonctions de message) est posé pour étendre progressivement.
- `--version`/`--help` existaient déjà sur les deux binaires (fournis automatiquement par `clap`) — vérifiés et confirmés plutôt que redupliqués.

### Planned (pas encore implémenté)
- Menu graphique **Files > Import a file...** (ouvre un sélecteur de fichier macOS) et menu **Help** : prévus pour la future application macOS empaquetée (voir la note Rosetta de la v0.25.0) — nécessitent l'étape Tauri, pas encore commencée. Notés en mémoire du projet pour ne pas être perdus d'ici là.

### Verified
- Import testé deux fois (`--lang` fr par défaut et en explicite) depuis `testdata/` (lecture seule) vers des destinations jetables : 2237 fiches correctement copiées à chaque fois, `bc_export --list-collections` relit directement le `.col` obtenu sans aucune modification de code.
- Tentative de réimport sur une destination déjà créée : refusée proprement, message bilingue.
- 2 nouveaux tests unitaires (destination par défaut, analyse de `--lang`) ; 12 tests au total, tous au vert.

## [0.25.0] - 2026-11-01

### Added
- **`bc_add add`** : enchaîne resolve+plan+commit en une seule commande, au lieu des 4 étapes séparées — l'usage courant désormais que les 4 étapes ont été validées individuellement.
- **`--also-db`** sur `bc_add add` : répète tout le pipeline (resolve+plan+commit) contre une seconde base, par exemple la vraie collection Book Collector tant qu'elle existe encore — en préparation d'une future base persistante propre au projet (extension/emplacement encore à définir), destinée à terme à remplacer la vraie collection quand Book Collector cessera de fonctionner (voir note Rosetta ci-dessous). Chaque base est résolue et planifiée **indépendamment** (jamais une recopie du même SQL — les clés primaires et valeurs déjà connues divergent dès que deux bases évoluent séparément). Si la base principale échoue, rien n'est tenté sur la seconde ; si la seconde échoue (ex. confirmation refusée), la première reste acquise et les fichiers resolved/plan de la tentative sont conservés pour être rejoués plus tard avec `bc_add commit`.
- Les fichiers intermédiaires de chaque run sont écrits dans un dossier horodaté (`bc_add_runs/<horodatage>/` par défaut), un par base (`principale.*`/`aussi.*`), inspectables après coup.

### Context
- Recherche web confirmant la fin programmée de Rosetta 2 (macOS 27 "Golden Gate", automne 2026, est la dernière version à le supporter pleinement ; macOS 28, automne 2027, le retire largement) — Book Collector, une app Intel, cessera donc probablement de fonctionner d'ici environ un an. C'est le moteur de `--also-db` : préparer la transition vers une base indépendante de l'app pendant qu'elle fonctionne encore.

### Verified
- Testé de bout en bout avec un `$HOME` factice entièrement isolé (jamais le vrai répertoire utilisateur) tenant lieu de "vraie collection" : base principale seule (succède), `--also-db` sans `--force` (refusée proprement, base principale déjà acquise), `--also-db --force` + `OUI` (les deux bases acceptent le livre, `PRAGMA integrity_check` = `ok` sur les deux, une seule ligne insérée dans chacune).
- 10 tests unitaires toujours au vert.

## [0.24.0] - 2026-11-01

### Added
- **Étape 6/6 du plan "ajout d'un livre"** : `bc_add commit --force` (seule façon d'écrire sur un chemin qui ressemble à la vraie collection) exige désormais, en plus, une **confirmation tapée interactivement** ("OUI", exactement) avant toute écriture réelle — deux garde-fous indépendants (le chemin ET la confirmation), aucun des deux ne suffit seul. `confirm_real_write()` isole cette logique (entrée/sortie paramétrées) pour rester testable sans terminal réel.

### Fixed
- **Bug réel découvert en testant l'étape 6** : `looks_like_real_collection` comparait un chemin canonicalisé (toujours résolu par `resolve_db_path`) à un préfixe `$HOME` non canonicalisé. Sur un `$HOME` passant par un lien symbolique (ex. `/var` → `/private/var` sur macOS), la comparaison échouait silencieusement et **tout le garde-fou de la vraie collection était contourné** — y compris le refus par défaut déjà en place depuis l'étape 4, pas seulement la nouvelle confirmation. Corrigé en canonicalisant aussi le préfixe avant comparaison.

### Verified
- 3 nouveaux tests unitaires (`bc_add`) : confirmation stricte (accepte seulement "OUI", refuse "oui"/"yes"/vide/Entrée seule), détection correcte sous un `$HOME` normal, et régression spécifique au cas symlink qui aurait détecté le bug ci-dessus. 10 tests au total, tous au vert.
- Scénario de bout en bout rejoué 3 fois avec un **`$HOME` factice isolé** (jamais le vrai répertoire de l'utilisateur) : refus sans `--force` (inchangé), refus avec `--force` + réponse incorrecte (fichier intact), succès avec `--force` + "OUI" (sauvegarde créée, écriture effectuée) — sur un fichier fictif, à aucun moment sur la vraie collection.
- `bc_add resolve/plan/commit` sur `testdata-rw/` rejoués sans changement de comportement (régression).

## [0.23.0] - 2026-11-01

### Added
- `bc_export` exporte désormais 6 champs supplémentaires (CSV et JSON) : `edition`, `printed_by`, `volume`, `paper_type`, `owner`, `store` — mêmes jointures que `publisher`/`format`/`condition`... (`owner` via `ZPERSONDELEGATE`, les cinq autres via `ZLOOKUPITEMDELEGATE`).
- Le formulaire HTML « Ajouter un livre » (onglets Détails/Personnel) a maintenant de l'auto-complétion sur ces 6 champs, en plus de ceux déjà couverts — complète la limite notée dans la v0.22.0.

### Verified
- Testé sur `testdata/` : 16 fiches avec `edition` renseignée, 1653 avec `owner` = "Serge" (cohérent avec l'unique ligne `ZOWNER` de la collection), 0 avec `store` (cohérent avec `ZSTORE` vide) — les nouvelles colonnes CSV/JSON correspondent aux données réelles.
- 7 tests unitaires toujours au vert ; vérifications statiques du formulaire (syntaxe JS, balises équilibrées) refaites après l'ajout des nouveaux `<datalist>`.

## [0.22.0] - 2026-11-01

### Added
- Le formulaire **« Ajouter un livre »** de la page HTML couvre maintenant tous les champs de l'étape 5 (`bc_add resolve`/`plan`), via des sous-onglets dans le panneau : Principal, Détails, Personnel, Crédits & Personnages, Contenus, Intrigue & Notes, Liens.
  - Crédits : champ à puces, comme auteurs/genres/sujets.
  - Personnages : petite liste avec nom + alias.
  - Contenus : sous-formulaire (titre, sous-titre, pages, auteurs à puces) avec bouton « Ajouter ce contenu », qui accumule une liste d'entrées.
  - Liens : liste url + description.
  - Les sept panneaux restent tous montés dans le DOM (juste masqués en CSS) : un seul envoi du formulaire récupère les champs de tous les onglets, pas seulement de celui affiché au moment du clic.
  - Les champs `original_publisher`/`original_country`/`original_language` réutilisent les mêmes suggestions (`<datalist>`) que les champs principaux ; `series` et `condition` ont aussi des suggestions (déjà présents dans le JSON exporté). Les autres nouveaux champs liste (édition, imprimeur, volume, type de papier, propriétaire, magasin) n'ont pas encore de suggestions — `bc_export` n'exporte pas encore ces valeurs.
- `setupRepeatableList()` dans `app.js`, pendant de `setupChipField()` pour les entrées à plusieurs sous-champs (personnages, contenus, liens) ; `setupChipField()` retourne désormais un handle (`render`/`clear`) pour permettre de vider le mini-formulaire « auteurs de ce contenu » après chaque ajout.

### Verified
- `node --check` sur `app.js` : syntaxe valide.
- Vérification statique (script Node ad hoc, pas de navigateur) : toutes les balises HTML du template du formulaire sont équilibrées (div/label/form/button/span/select/textarea/datalist/h2/h3/p), et chaque `name=` de champ correspond exactement à un appel `str()/num()/flt()/bool()` dans `onAddFormSubmit` — aucun champ manquant ni orphelin.
- **Non vérifié visuellement dans un navigateur** : Chrome refuse les clics synthétiques via Accessibility sur cette machine (même limite déjà rencontrée plus tôt dans ce projet) — je n'ai pas pu cliquer sur les nouveaux onglets moi-même pour confirmer leur rendu. À vérifier à l'ouverture par l'utilisateur.

## [0.21.0] - 2026-11-01

### Added
- **`bc_add resolve`/`plan`** (étape 5) couvrent maintenant les onglets Détails, Personnel, Crédits & Personnages, Contenus, Intrigue/Notes et Liens de la fenêtre "Add Book" (Couvertures exclue — gestion de fichiers, hors périmètre) :
  - Détails : édition, imprimeur, série, volume, type de papier, dimensions, Dewey, LCCN, dédicace, 1ère édition, tirage, abrégé, durée audio, titre/sous-titre/éditeur/pays/langue/date de publication originaux.
  - Personnel : état, propriétaire, magasin, valeur actuelle, prix d'achat/de couverture, date d'achat, rare, lu, nombre de lectures, date de lecture, note.
  - Intrigue & Notes : résumé (plot) et notes — colonnes directes de `ZCOLLECTIBLE`.
  - Crédits & Personnages : nouvelles tables `ZCREATOR`/`ZCHARACTER` (même motif flyweight que les auteurs) — le rôle d'un crédit (`ZROLEID`) n'est pas résolu (mapping nom↔identifiant non trouvé en base).
  - Contenus : `ZSTORY` (plusieurs histoires dans un livre), chacune avec ses propres auteurs via `ZAUTHOR.ZSTORY`.
  - Liens : `ZLINK` + compagnon `ZITEMDELEGATE`, motif confirmé sur les 832 liens réels déjà présents dans la collection de test.
- `PersonKind` généralise `LookupKind` pour les champs "personne" (auteurs, crédits, personnages, propriétaire) ; `plan_person_fk`/`plan_person_list` factorisent la création de nouvelles personnes (le code d'auteur, jusque-là en ligne, utilise désormais ces mêmes helpers).

### Verified
- Testé de bout en bout sur `testdata-rw/` avec un livre d'exemple exerçant TOUS les nouveaux champs : `resolve` → `plan` → `guard` → `commit`, `PRAGMA integrity_check` = `ok`, chaque nouvelle table (`ZCREATOR`, `ZCHARACTER`, `ZSTORY`, `ZLINK`) contient exactement la ligne attendue, les jointures (crédit, personnage avec alias, contenu avec son propre auteur) se résolvent correctement en SQL. Book Collector rouvre le fichier sans alerte de corruption et affiche le bon compteur de livres.
- Non vérifié visuellement dans l'app elle-même (navigation par liste non fiable via automatisation d'accessibilité sur cette fenêtre, même limite déjà rencontrée à l'étape 4) : la fiche n'a pas pu être inspectée onglet par onglet dans l'interface. Les tables `ZCREATOR`/`ZCHARACTER`/`ZSTORY` étant vides dans la collection de test, leur plan SQL est construit par analogie (voir le détail "non couvert" affiché par `bc_add plan`), pas recoupé avec des lignes réelles préexistantes.

## [0.20.0] - 2026-11-01

### Added
- **`bc_add commit`** (étape 4) : exécute le SQL produit par `plan` dans une transaction unique (tout ou rien) — première commande qui écrit réellement dans `Book_data`. Avant toute écriture : refus si le chemin ressemble à la vraie collection (`~/Documents/Collectorz/...`, sauf `--force`, non utilisé avant l'étape 6), vérification que Book Collector n'est pas lancé, et sauvegarde horodatée automatique (réutilise les fonctions de `guard`).
- `refresh-testdata-rw.sh` : copie dédiée **inscriptible** (`testdata-rw/`), distincte de `testdata/` qui reste en lecture seule — sert uniquement aux tests d'écriture de `bc_add commit`, jamais la vraie collection.

### Verified
- Testé de bout en bout sur `testdata-rw/` : `bc_add commit` insère le livre de test ("Le Nom du Vent"), `PRAGMA integrity_check` reste `ok`, et Book Collector rouvert sur cette copie affiche le compteur de livres incrémenté de 1 (1242 → 1243), sans alerte de corruption. La recherche texte ne trouve pas encore la fiche (`ZSEARCHSTRING` non couvert, déjà documenté par `plan`), mais la fiche est bien présente et correcte en parcourant la liste.

## [0.19.0] - 2026-11-01

### Added
- **`bc_add guard`** (étape 3) : vérifie que Book Collector n'est pas lancé (`pgrep`) et crée une sauvegarde horodatée de `Book_data` (+ `-wal`/`-shm` si présents) avant toute écriture future. Ne modifie jamais `Book_data` lui-même — seule une copie est créée dans `backups/`.
- `is_book_collector_running()`, `backup_sqlite_store()` et `run_write_transaction()` dans `lib.rs` : les deux premières sont exercées par `bc_add guard` dès maintenant ; la troisième (transaction unique tout-ou-rien pour l'écriture réelle) est écrite et prête, mais pas encore appelée par aucune commande.

### Verified
- Testé en conditions réelles sur `testdata/` : Book Collector étant ouvert au moment du test, `guard` l'a correctement détecté et a refusé de recommander une écriture, tout en créant la sauvegarde avec succès.

## [0.18.0] - 2026-11-01

### Added
- `bc_add` devient multi-commandes : `bc_add resolve` (étape 1, inchangée) et **`bc_add plan`** (étape 2) — calcule et affiche le SQL d'insertion complet (nouvelles valeurs flyweight, fiche `ZCOLLECTIBLE`, jointures auteurs/genres/sujets, mises à jour `Z_PRIMARYKEY.Z_MAX`) à partir du résultat de `resolve`, **sans jamais l'exécuter**.
- La sortie de `plan` liste explicitement ce qu'elle ne couvre pas encore (`ZSEARCHSTRING`, tri de titre, identifiants de synchro CLZ Cloud, prénom/nom séparés pour les auteurs).
- `entity_ent_id`/`entity_max_pk` génériques dans `lib.rs` (généralisent l'ancien `book_entity_id`), pour allouer de nouvelles clés primaires sans collision.

### Changed
- CLI de `bc_add` : `--db/--input/--output` passent sous la sous-commande `resolve` (`bc_add resolve --db ... --input ...` au lieu de `bc_add --db ...`).

## [0.17.0] - 2026-11-01

### Added
- Formulaire **« Ajouter un livre »** dans la page HTML générée (bouton dans l'en-tête, à côté de Statistiques) : reprend les champs de l'onglet Main de Book Collector, avec auto-complétion basée sur les valeurs déjà présentes dans la collection chargée (éditeur, genres, auteurs, format, langue, pays, emplacement, sujets). Purement côté client — ne contacte aucun serveur, ne touche à aucune donnée : le bouton « Générer new_book.json » télécharge simplement le fichier, à passer ensuite à `bc_add` (§ 7 du manuel).
- Panneaux Statistiques / Ajouter un livre / Collection généralisés sous un seul commutateur (`state.panel`), plutôt que des bascules indépendantes.

## [0.16.0] - 2026-11-01

### Added
- Nouveau binaire **`bc_add`** (étape 1/6 du plan "ajout d'un livre") : prend un JSON décrivant un nouveau livre et résout chaque champ "liste" (auteur, éditeur, genre, format, langue, pays, sujet, emplacement) contre les tables flyweight existantes — réutilisation si la valeur existe déjà, sinon marquée "à créer". **Aucune écriture** dans la base (ouverture `SQLITE_OPEN_READ_ONLY`, comme `bc_export`).
- Section 7 du manuel décrivant cette première étape et le format d'entrée JSON.

### Changed
- Extraction d'une bibliothèque partagée (`src/lib.rs`) regroupant les fonctions communes aux deux binaires (`expand_tilde`, `resolve_db_path`, `resolve_collection_pk`, et les nouvelles `resolve_lookup_pk`/`resolve_person_pk`) — `bc_export` et `bc_add` ne dupliquent plus ce code.
- **Règle désormais actée** : tout développement/test se fait exclusivement sur `testdata/MyCollection.bkcp` — la vraie base de l'utilisateur (`~/Documents/Collectorz/...`) n'est plus jamais utilisée, pas même en lecture.

## [0.15.0] - 2026-11-01

### Added
- Option `--version` / `-V` en ligne de commande (clap, lit `Cargo.toml`).
- Focus clavier plus visible : la ligne atteinte par les flèches défile automatiquement dans la vue et affiche un repère (liseré bleu) en plus du fond coloré ; contour `:focus-visible` sur les contrôles natifs (boutons, champs, menus).

## [0.14.0] - 2026-11-01

### Added
- `MANUAL.md` : premier manuel d'utilisation (présentation, installation, usage de base, sous-collections, page HTML, copie de test, dépannage), versionné dans git comme le reste du projet.

## [0.13.0] - 2026-11-01

### Added
- Dépôt git initialisé (`cargo new` l'avait déjà fait, mais sans commit) : premier commit `v0.12.1` taggé, `.gitignore` étendu pour exclure `testdata/` (données personnelles, ~400 Mo) en plus de `target/`.
- `refresh-testdata.sh` : resynchronise `testdata/MyCollection.bkcp` depuis la vraie collection et la repasse en lecture seule, pour ne plus avoir à le faire à la main quand la collection réelle évolue.

## [0.12.1] - 2026-11-01

### Changed
- Le badge de version reprend maintenant le même style que le bouton Statistiques (pilule avec bordure, fond blanc), précédé d'une icône « Ⓥ ».

## [0.12.0] - 2026-11-01

### Added
- Numéro de version de l'outil (`bc_export v0.12.0`, lu depuis `Cargo.toml` à la compilation via `env!("CARGO_PKG_VERSION")`) affiché dans l'en-tête de la page générée, à côté du bouton Statistiques.

## [0.11.1] - 2026-11-01

### Fixed
- `--db` avec un chemin relatif (ex. `testdata/MyCollection.bkcp`) produisait des URL `file://` invalides pour les couvertures (`file://testdata/...`), donc une fiche détail sans image alors que les vignettes en base64 s'affichaient normalement. `resolve_db_path` canonicalise désormais systématiquement le chemin en absolu. Trouvé en vérifiant visuellement l'export généré depuis la nouvelle copie `testdata/`.

## [0.11.0] - 2026-11-01

### Added
- Copie de travail de la collection dans `testdata/MyCollection.bkcp`, en lecture seule (`chmod 444`) — les tests et développements futurs se font sur cette copie, plus jamais sur le fichier original dans `~/Documents`.
- Ce fichier `CHANGELOG.md`.

## [0.10.0] - 2026-11-01

### Changed
- Le template HTML/CSS/JS de la page exportée, auparavant une chaîne Rust géante, est sorti dans trois fichiers dédiés (`templates/page.html`, `templates/style.css`, `templates/app.js`), inclus à la compilation via `include_str!` — le binaire reste autonome, mais le code redevient éditable normalement (coloration syntaxique, pas d'accolades doublées).

### Added
- 7 tests unitaires (`cargo test`) sur les fonctions les plus sensibles : conversion de date Core Data (dont un cas vérifié contre une vraie valeur `ZDATEADDED`), encodage `file://`, expansion de `~`.

## [0.9.0] - 2026-11-01

### Added
- Vraies miniatures de couverture générées à l'export (redimensionnement + réencodage JPEG qualité 70, embarquées en `data:` URI) au lieu de charger les couvertures pleine résolution dans les listes.
- Persistance complète de l'état de navigation via `localStorage` (sous-collection, regroupement, lettre A-Z, recherche, tri, mode liste/grille, page, livre sélectionné).
- En-têtes de colonnes triables (Titre / Auteur) au-dessus de la liste.
- Navigation clavier (flèches haut/bas) dans la liste de livres.
- Page Statistiques agrégées par sous-collection : totaux, complétude, répartitions par genre/format/langue/éditeur/décennie.

## [0.8.0] - 2026-11-01

### Added
- Séparateurs redimensionnables à la souris entre les 3 panneaux (gauche/droite, et haut/bas à droite), avec mémorisation de la disposition.

## [0.7.0] - 2026-11-01

### Changed
- Réorganisation de la page en 3 panneaux reproduisant l'interface réelle de Book Collector : regroupement à gauche (Auteur par défaut, avec barre A-Z et choix du champ de regroupement), liste des livres du groupe en haut à droite, fiche détail en bas à droite.

## [0.6.0] - 2026-11-01

### Added
- Vue grille des couvertures (bascule Liste/Grille).
- Champs supplémentaires dans la fiche détail : sujets, tags, résumé, notes, valeur actuelle, prix d'achat, emplacement, état, nombre de pages.
- Filtre par genre, y compris en cliquant directement sur un tag de genre dans la fiche détail.

## [0.5.0] - 2026-11-01

### Added
- Onglets pour basculer entre les sous-collections directement dans la page exportée.
- Tri (titre, auteur, année, éditeur).
- Pagination de la liste (60 livres/page) avec miniatures de couverture par ligne.

## [0.4.0] - 2026-11-01

### Added
- Première version de l'export HTML : page unique autonome avec liste de livres et fiche détail façon Book Collector (fond sombre, couverture, métadonnées).

## [0.3.0] - 2026-11-01

### Fixed
- Expansion manuelle de `~/...` dans les chemins passés en argument : le shell ne le fait pas quand l'argument est entre guillemets, ce qui faisait échouer `--db "~/Documents/..."`.

## [0.2.0] - 2026-11-01

### Added
- Option `--collection` pour filtrer l'export sur une sous-collection précise.
- Option `--list-collections` pour lister les sous-collections disponibles avec leurs compteurs.
- Colonne `quantity` (nombre d'exemplaires), pour retrouver exactement le compteur affiché par l'application.

## [0.1.0] - 2026-11-01

### Added
- Export initial de la collection Book Collector (base Core Data/SQLite) en CSV et JSON, en lecture seule, avec résolution des auteurs, éditeurs, genres et chemins de couverture.
