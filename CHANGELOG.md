# Changelog

Toutes les versions notables de `bc_export` sont documentées ici.
Format inspiré de [Keep a Changelog](https://keepachangelog.com/fr/1.0.0/).
Un numéro de version est ajouté après chaque demande traitée.

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
