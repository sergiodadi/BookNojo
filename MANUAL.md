# Manuel d'utilisation — bc_export

Export en lecture seule d'une collection Book Collector (Core Data/SQLite)
vers CSV, JSON, et une page web interactive reproduisant l'interface de
l'application.

> Document de travail, volontairement court pour l'instant. Il grandira au
> fil des demandes, comme `CHANGELOG.md`.

## 1. Présentation

`bc_export` lit directement le fichier `Book_data` à l'intérieur d'un
document `.bkcp` de Book Collector (Collectorz) et produit :

- un fichier **CSV** et un fichier **JSON** avec toutes les informations de
  chaque livre (titre, auteurs, éditeur, genres, couvertures, etc.) ;
- en option, une **page HTML autonome** qui se consulte dans n'importe quel
  navigateur, sans serveur, reprenant l'aspect de l'application (liste
  groupée par auteur/genre/..., fiche détail, statistiques).

La base est toujours ouverte en lecture seule (`SQLITE_OPEN_READ_ONLY`) :
l'outil ne peut pas modifier la collection originale.

## 2. Installation

Prérequis : Rust (`cargo`), installable via Homebrew si besoin
(`brew install rust`).

```bash
cd ~/Developer/book_collector_export
cargo build --release
```

Le binaire est alors `target/release/bc_export`.

## 3. Utilisation de base

Exporter toute la collection en CSV et JSON :

```bash
./target/release/bc_export \
  --db "~/Documents/Collectorz/Book Collector/MyCollection.bkcp" \
  --csv book_collection.csv \
  --json book_collection.json
```

`--db` accepte soit le dossier `.bkcp`, soit directement le fichier
`Book_data` qu'il contient.

## 4. Sous-collections : fiches et exemplaires

Un fichier `.bkcp` peut contenir **plusieurs sous-collections** (table
`ZCOLLECTION`). Deux commandes utiles :

```bash
# Lister les sous-collections et leurs compteurs
./target/release/bc_export --db "<chemin .bkcp>" --list-collections

# N'exporter qu'une seule sous-collection
./target/release/bc_export --db "<chemin .bkcp>" \
  --collection "book-collector-serge" \
  --csv books.csv --json books.json
```

Le compteur affiché par l'application est la **somme des quantités**
(`ZQUANTITY`) de la sous-collection active, pas le nombre de fiches — les
deux valeurs peuvent légitimement différer.

## 5. La page HTML interactive

Ajouter `--html` génère en plus une page autonome :

```bash
./target/release/bc_export --db "<chemin .bkcp>" \
  --csv /tmp/bc.csv --json /tmp/bc.json \
  --html ~/Documents/book_collection_preview.html
```

Elle s'ouvre avec n'importe quel navigateur (double-clic, ou `open`). Trois
zones, comme l'application :

- **à gauche** : regroupement (Auteur par défaut, ou Genre / Éditeur /
  Format / Langue / Pays via le menu déroulant), avec barre A→Z ;
- **en haut à droite** : liste des livres du groupe sélectionné —
  recherche, tri, pagination, bascule vue liste/grille ;
- **en bas à droite** : fiche détail (couverture, métadonnées, genres
  cliquables, résumé, notes).

Les onglets en haut permettent de changer de sous-collection sans relancer
la commande. Le bouton **Statistiques** affiche une vue agrégée (genres,
formats, langues, éditeurs, décennies) pour la sous-collection active. Les
séparateurs entre les 3 zones se redimensionnent à la souris. La
disposition et la dernière position de navigation sont mémorisées d'une
ouverture à l'autre (stockage local du navigateur).

Dans la colonne de gauche, cliquer sur le **nombre** à droite de la ligne
"Tous" bascule la liste des valeurs en dessous (auteurs, genres, etc.)
entre un tri alphabétique (défaut) et un tri par nombre de livres
décroissant — recliquer y revient (v0.42.0).

**Interface bilingue (v0.46.0)** : un sélecteur **FR / EN** dans l'en-tête
(à côté du numéro de version) bascule toute l'interface — regroupement,
liste, fiche détail, statistiques, formulaire "Ajouter un livre" — entre
français et anglais, mémorisé d'une ouverture à l'autre. Les messages
renvoyés par l'application NBC elle-même (confirmations d'écriture,
erreurs de sauvegarde, etc.) restent en français quelle que soit la
langue choisie — seule l'interface générée côté navigateur est traduite
pour l'instant.

## 6. Copie de travail et tests

Pour ne jamais lire ni risquer de modifier le fichier original pendant les
tests, le projet garde une copie en lecture seule dans `testdata/` :

```bash
./refresh-testdata.sh      # resynchronise testdata/ depuis la vraie collection
cargo test --release       # lance les tests unitaires ET d'intégration
```

**Tests d'intégration (v0.47.0)** : `tests/add_edit_delete_round_trip.rs`
exerce les vrais pipelines d'ajout/modification/suppression contre une
copie temporaire et jetable de `testdata/` (jamais `testdata/` lui-même,
jamais `testdata-rw/`) — créée et détruite automatiquement à chaque
exécution de `cargo test`, rien à préparer à la main. Complète les tests
unitaires existants, qui ne couvrent que de petites fonctions isolées, pas
une vraie écriture de bout en bout.

Pour des itérations plus rapides (notamment sur l'app NBC, § 8), un petit
échantillon dérivé de `testdata/` — jamais directement la vraie collection :

```bash
./build-sample-testdata.sh   # crée testdata-sample/ (~100 fiches, dont tous
                              # les livres de 1-2 auteurs bien fournis)
```

Structurellement valide (vérifié en l'ouvrant dans Book Collector), mais
reste des données personnelles — jamais committé (`.gitignore`).

## 7. (Expérimental) Préparer l'ajout d'un livre — étapes 1 à 6 sur 6

> ⚠️ En cours de construction, par étapes. `bc_add resolve`, `plan` et
> `guard` ne font **aucune écriture** (base ouverte en lecture seule).
> Depuis l'étape 4, `bc_add commit` **écrit réellement** dans `Book_data`.
> Par défaut, cette écriture est réservée à une copie dédiée et
> inscriptible (`testdata-rw/`) : un garde-fou refuse tout chemin qui
> ressemble à `~/Documents/Collectorz/...`. Depuis l'étape 6, il est
> possible de passer outre avec `--force`, mais un second garde-fou prend
> le relais : taper **OUI** de façon interactive est alors obligatoire,
> dans le terminal, à chaque exécution — jamais automatisable par erreur.

### Étape 1 — `bc_add resolve`

Prend un fichier JSON décrivant un nouveau livre (champs de l'onglet
"Main" de la fenêtre "Add Book" de l'application) et vérifie, pour chaque
valeur de type liste (auteur, éditeur, genre, format, langue, pays, sujet,
emplacement), si elle existe déjà dans la collection ou si elle serait
créée.

### Produire ce JSON : le formulaire dans la page HTML

Pas besoin d'écrire le JSON à la main : la page générée par
`--html` (§ 5) a un bouton **➕ Ajouter un livre** dans l'en-tête. Le
formulaire est organisé en sous-onglets, comme la fenêtre "Add Book" de
Book Collector : **Principal**, **Détails**, **Personnel**, **Crédits &
Personnages**, **Contenus**, **Intrigue & Notes**, **Liens** (tous les
champs des étapes 1 et 5, § 7). Auto-complétion sur les valeurs déjà
utilisées dans la collection pour tous les champs de type liste (éditeur,
format, langue, pays, emplacement, auteur, genre, sujet, série, état,
édition, imprimeur, volume, type de papier, propriétaire, magasin) — pas
encore pour les crédits/personnages (`bc_export` n'exporte pas ces
tables, vides dans la collection de test — voir § 7, étape 5). Changer
d'onglet ne perd aucune saisie : un seul envoi du formulaire récupère les
champs de tous les onglets.

Ce formulaire se comporte différemment selon où il tourne : dans **l'export
HTML statique** de `bc_export` décrit ici, il **ne contacte aucun serveur
et n'écrit rien** — cliquer sur "Générer new_book.json" télécharge
simplement le fichier, à passer ensuite à `bc_add` comme décrit ci-dessous.
Dans **l'app NBC** (§ 8), le même formulaire écrit directement dans la
collection ouverte, avec une confirmation avant écriture — voir § 8 pour
le détail.

### Ou à la main

```bash
./target/release/bc_add resolve \
  --db testdata/MyCollection.bkcp \
  --input new_book.json \
  --output resolved-book.json
```

Exemple de `new_book.json` :

```json
{
  "title": "Le Nom du Vent",
  "authors": ["Patrick Rothfuss"],
  "publisher": "Bragelonne",
  "format": "Broché",
  "language": "français",
  "country": "France",
  "genres": ["Fantasy"],
  "collection": "book-collector-serge"
}
```

La sous-collection (`collection`) doit déjà exister — contrairement aux
autres champs, `bc_add` ne sait pas en créer une nouvelle.

### Étape 2 — `bc_add plan`

À partir du fichier produit par `resolve`, calcule et **affiche** le SQL
d'insertion exact (nouvelles clés primaires, lignes à créer, mises à jour
des compteurs `Z_PRIMARYKEY`) — sans jamais l'exécuter.

```bash
./target/release/bc_add plan \
  --db testdata/MyCollection.bkcp \
  --resolved resolved-book.json \
  --output plan.sql
```

La sortie se termine par une liste explicite de ce que ce plan **ne**
couvre **pas** encore (ex. la ligne compagnon `ZSEARCHSTRING`, le tri de
titre, les identifiants de synchronisation CLZ Cloud) — à lire avant de
passer aux étapes suivantes.

### Étape 3 — `bc_add guard`

Vérifie que les conditions sont réunies pour écrire sans risque, et
produit une sauvegarde horodatée de la base (copie, donc toujours sans
modifier `Book_data` lui-même) :

```bash
./target/release/bc_add guard --db testdata/MyCollection.bkcp
```

Deux vérifications :
- **Book Collector n'est pas lancé** (`pgrep`) — écrire pendant que l'app
  tourne risquerait un fichier WAL incohérent, ou que l'app écrase nos
  changements à sa prochaine sauvegarde.
- **Sauvegarde automatique** de `Book_data` (et de `-wal`/`-shm` si
  présents) dans `backups/` à côté de la base, horodatée.

### Étape 4 — `bc_add commit`

Exécute le SQL produit par `plan`, **dans une transaction unique** (tout ou
rien — en cas d'erreur, aucune modification n'est conservée). C'est la
première commande qui écrit réellement dans `Book_data`.

```bash
# D'abord une copie inscriptible dédiée (jamais testdata/, lui reste
# en lecture seule) :
./refresh-testdata-rw.sh

./target/release/bc_add commit \
  --db testdata-rw/MyCollection.bkcp \
  --plan plan.sql
```

Avant d'écrire, `commit` vérifie dans l'ordre :
1. que le chemin de la base **ne ressemble pas** à la vraie collection
   (`~/Documents/Collectorz/...`) — refus immédiat sinon, sans option de
   contournement accidentelle (`--force` existe mais est réservé à
   l'étape 6, qui n'a pas encore été abordée) ;
2. que Book Collector **n'est pas lancé** (comme `guard`) ;
3. crée une sauvegarde horodatée de `Book_data` (comme `guard`) ;
4. ouvre la base en lecture-écriture et exécute tout le SQL de `plan.sql`
   dans une seule transaction (`run_write_transaction` dans `lib.rs`).

Vérifié en conditions réelles sur `testdata-rw/` : `PRAGMA integrity_check`
reste à `ok` après coup, et Book Collector, rouvert sur cette copie,
affiche le compteur de livres incrémenté de 1, sans aucune alerte de
corruption. La fiche du nouveau livre n'apparaît pas encore dans la
**recherche texte** de l'application — normal, `ZSEARCHSTRING` fait partie
des champs "non couverts" listés par `plan` (§ Étape 2) ; elle reste en
revanche consultable en parcourant/triant la liste.

### Étape 5 — les autres onglets de "Add Book"

`resolve` et `plan` couvrent maintenant, en plus de l'onglet Main (§
Étape 1), tous les champs de la fenêtre "Add Book" qui correspondent à une
colonne ou une relation connue — sauf Couvertures (gestion de fichiers,
hors périmètre de `bc_add`). Tous ces champs sont optionnels dans le JSON
d'entrée ; seuls ceux fournis sont pris en compte.

- **Détails** : `edition`, `printed_by`, `series`, `volume`, `paper_type`
  (listes, comme `publisher`/`format`...), `height`, `width`, `dewey`,
  `lccn`, `dedication` (texte libre), `first_edition`, `printing`,
  `abridged`, `audio_length_minutes`, `original_title`,
  `original_subtitle`, `original_publisher`, `original_country`,
  `original_language`, `original_publication_year/month/day`.
- **Personnel** : `condition`, `owner` (personne), `store`,
  `current_value`, `purchase_price`, `cover_price`,
  `purchase_year/month/day`, `rare`, `read_it`, `read_times`,
  `read_year/month/day`, `my_rating`.
- **Intrigue & Notes** : `plot`, `notes`.
- **Crédits & Personnages** : `credits` (liste de noms — le rôle de
  chaque crédit n'est pas encore capturé, voir ci-dessous) et
  `characters` (liste de `{"name": ..., "alias": ...}`).
- **Contenus** : `contents`, liste de
  `{"title": ..., "subtitle": ..., "number_of_pages": ..., "authors": [...]}`
  — plusieurs histoires/nouvelles dans un même livre, chacune avec ses
  propres auteurs.
- **Liens** : `links`, liste de `{"url": ..., "description": ...}`.

Exemple minimal pour ces nouveaux champs (à ajouter au JSON de l'étape 1) :

```json
{
  "...": "... champs de l'onglet Main ...",
  "plot": "Résumé de l'histoire.",
  "notes": "Exemplaire dédicacé.",
  "condition": "Mint",
  "credits": ["Nom du traducteur"],
  "characters": [{ "name": "Nom du personnage", "alias": "Surnom" }],
  "contents": [{ "title": "Titre de la nouvelle", "authors": ["Nom de l'auteur"] }],
  "links": [{ "url": "https://...", "description": "Description du lien" }]
}
```

Limites connues de cette étape (affichées aussi par `bc_add plan`) :
- Le **rôle** d'un crédit (Illustrateur, Traducteur...) n'est pas
  renseigné — la correspondance entre son nom et l'identifiant numérique
  interne (`ZROLEID`) n'a pas été trouvée en base.
- Les tables utilisées pour Crédits/Personnages/Contenus (`ZCREATOR`,
  `ZCHARACTER`, `ZSTORY`) étaient **vides** dans la collection de test :
  leur plan SQL suit par analogie des motifs vérifiés ailleurs (auteurs,
  liens), mais n'a pas pu être comparé à des lignes réelles préexistantes
  comme pour le reste de l'étape 5. Vérifié par SQL direct (jointures,
  `PRAGMA integrity_check`) sur `testdata-rw/`, pas encore inspecté
  visuellement fiche par fiche dans l'application (navigation par liste
  peu fiable en automatisation d'accessibilité sur cette fenêtre).

**v0.37.0 — rôle des crédits et champs personnalisés.**
Trois champs que l'audit de schéma seul ne pouvait pas résoudre (sans table
de correspondance ni données réelles) sont désormais résolus :

- **Rôle d'un crédit** (`ZROLEID`) : la correspondance nom↔identifiant
  n'existe que comme constantes compilées dans l'app, désormais connue
  (11 rôles). L'onglet Crédits
  est désormais une liste (comme Personnages), avec un rôle optionnel.
- **`user_lookup1`/`user_lookup2`** (Personnel) : confirmés comme des champs
  de type liste standard, même motif que les autres champs de ce type.
- **`user_text1`/`user_text2`** (Personnel) : confirmés comme texte libre.

**v0.36.0 — audit complet des colonnes `ZCOLLECTIBLE`/`ZSTORY`** (suite à
un champ manquant signalé par l'utilisateur, "Issue N°") : comparaison de
chaque colonne du schéma contre ce qui était déjà couvert. Ajoutés : `issue_number`
(`ZISSUENUMBER`, confirmé par des données réelles — ~49% des fiches de
`testdata/`), `box_set` (`ZBOXSET`, case à cocher Détails), `location_number`
(`ZLOCNR`, champ texte Principal, confirmé par des données réelles — numéro
d'emplacement libre, pas une liste), `start_page` par contenu (`ZSTORY.ZSTARTPAGE`).
Volontairement **non implémentés** (listés en détail par `bc_add plan`,
section "Field audit") : les champs personnalisés `ZUSERTEXT1`/`ZUSERTEXT2`/
`ZUSERLOOKUP1`/`ZUSERLOOKUP2` (aucune ligne non vide dans `testdata/`, sémantique
non vérifiable) ; le reste des champs de `ZSTORY` qui dupliquent la section
"Édition originale" + Plot/Notes/Série/Note du livre (par analogie seulement,
`ZSTORY` est vide dans `testdata/`, et ~10 champs supplémentaires par contenu
semblaient disproportionnés sans données réelles pour les confirmer).

### Étape 6 — activation sur la vraie collection

`bc_add commit` refuse par défaut tout chemin qui ressemble à
`~/Documents/Collectorz/...` (voir Étape 4). `--force` lève ce refus, mais
**n'écrit toujours pas directement** : un second garde-fou,
indépendant du premier, prend le relais — une confirmation tapée à la
main dans le terminal :

```
⚠️  ATTENTION : .../MyCollection.bkcp/Book_data se trouve sous
.../Documents/Collectorz — ceci est la VRAIE collection Book Collector,
pas une copie de test.
   Une sauvegarde horodatée sera prise avant d'écrire, mais cette opération
   modifie directement vos données réelles.
   Tapez exactement OUI pour continuer (autre chose annule) :
```

Toute réponse différente de `OUI` (lettres capitales exactes) annule
l'opération — aucune écriture n'a lieu, même avec `--force`. Les deux
garde-fous (le chemin ET la confirmation tapée) sont indépendants et
testés séparément (`cargo test`, y compris une régression spécifique à un
`$HOME` passant par un lien symbolique, comme `/var` → `/private/var` sur
macOS — un cas qui, avant correction, pouvait faire échouer silencieusement
la détection du chemin).

Avant d'utiliser `--force` sur la vraie collection :
1. Fermez Book Collector.
2. Faites une sauvegarde manuelle en plus de celle, automatique, que
   `commit` effectue (ex. copier tout le dossier `.bkcp`).
3. Relisez le `plan.sql` produit par l'étape 2 — c'est la dernière
   occasion de repérer un problème avant une écriture réelle.
4. Lancez `commit --force` et tapez `OUI` seulement si tout est en ordre.

```bash
./target/release/bc_add commit \
  --db "~/Documents/Collectorz/Book Collector/MyCollection.bkcp" \
  --plan plan.sql \
  --force
```

### Usage courant — `bc_add add`

Une fois les 4 étapes comprises et validées séparément, pas besoin de les
relancer à la main à chaque livre : `bc_add add` les enchaîne en une seule
commande (resolve → plan → commit).

```bash
./target/release/bc_add add \
  --db collection/MyCollection.bkcp \
  --input new_book.json
```

Elle accepte aussi `--also-db` pour répéter tout le pipeline contre une
**seconde** base — typiquement la vraie collection Book Collector, tant
qu'elle existe encore (voir § "Pourquoi une base indépendante" ci-dessous).
Chaque base est résolue et planifiée **séparément** : ce n'est jamais le
même SQL recopié d'une base à l'autre, parce que les clés primaires et les
valeurs déjà connues (éditeurs, genres...) divergent dès que deux bases
évoluent indépendamment.

```bash
./target/release/bc_add add \
  --db collection/MyCollection.bkcp \
  --also-db "~/Documents/Collectorz/Book Collector/MyCollection.bkcp" \
  --input new_book.json \
  --force
```

`--force` s'applique aux deux bases si l'une ou l'autre ressemble à la
vraie collection ; la confirmation `OUI` (étape 6) reste exigée à chaque
exécution. Si la base principale échoue, rien n'est tenté sur la seconde.
Si c'est la seconde qui échoue (ex. confirmation refusée, ou Book
Collector encore ouvert), la première reste acquise — les fichiers
`resolved`/`plan` de la tentative sont conservés dans
`bc_add_runs/<horodatage>/` pour être rejoués plus tard avec
`bc_add commit`, sans tout recommencer.

### Pourquoi une base indépendante de Book Collector

Book Collector est une application Intel qui dépend de Rosetta 2 pour
fonctionner sur Apple Silicon. Apple a annoncé la fin programmée de
Rosetta 2 : support complet jusqu'à macOS 27 "Golden Gate" (automne 2026),
largement retiré à partir de macOS 28 (automne 2027). Book Collector a
donc une espérance de vie d'environ un an sur un Mac à jour, sauf mise à
jour de l'éditeur vers Apple Silicon natif.

L'objectif à terme est une base **indépendante** de l'application — amorcée
une fois depuis la vraie collection, puis alimentée uniquement par ces
outils, sans jamais dépendre de Book Collector pour continuer à grandir.
`--also-db` sert la période de transition : continuer à alimenter la vraie
application tant qu'elle tourne encore, en parallèle de la nouvelle base.

Le nom de projet provisoire pour "les scripts actuels + la future
application" est **NBC** (un nom définitif sera choisi plus tard) —
remplacement progressif de Book Collector ("BC"). L'extension retenue
pour cette base persistante est **`.col`**.

### `bc_add import` — le transfert BC vers NBC

Amorce une nouvelle base `.col` indépendante à partir d'une collection
existante (base + dossier `Images/`). Ne sert qu'**une seule fois** :
refuse d'écraser une destination qui existe déjà.

```bash
./target/release/bc_add import \
  --source "~/Documents/Collectorz/Book Collector/MyCollection.bkcp"
# -> collection/MyCollection.col (destination par défaut)

# Ou avec une destination explicite :
./target/release/bc_add import \
  --source "~/Documents/Collectorz/Book Collector/MyCollection.bkcp" \
  --dest ~/MaCollectionNBC.col
```

L'extension n'affecte aucun code : `resolve_db_path` ne regarde que la
présence d'un fichier `Book_data` à l'intérieur, pas le nom du dossier.
`bc_export`/`bc_add` fonctionnent sur un `.col` exactement comme sur un
`.bkcp`.

### Langue des messages (`--lang`)

```bash
./target/release/bc_add --lang en import --source ... --dest ...
```

Amorce d'internationalisation : pour l'instant, seule `import` a des
messages en français et en anglais (`fr` par défaut). Le reste de l'outil
reste en français — à étendre progressivement, pas tout d'un coup.

### Prévu, pas encore fait : menus graphiques

Pour la future application macOS (§ ci-dessus, empaquetage Tauri) : un
menu **Files** avec un sous-menu **Import a file...** qui ouvrirait le
sélecteur de fichier natif macOS (équivalent graphique de `bc_add
import`), et un menu **Help**. Nécessitent que l'étape Tauri soit
commencée — pas encore le cas.

## 8. (Expérimental) Application macOS "NBC"

> ⚠️ Tout début. Projet séparé, dans `app/` — n'affecte pas `bc_export`/
> `bc_add` ni leur propre `Cargo.toml`.

Le but à terme (voir § 7, "Pourquoi une base indépendante") est une
application macOS installée, à icône, dans `/Applications` — construite
avec [Tauri](https://tauri.app) (backend Rust + interface web), pour
réutiliser le code déjà écrit plutôt que tout réécrire.

**Étape 1/6 (faite)** : squelette vide, juste pour prouver que la
mécanique fonctionne.

**Étape 2/6 (faite)** : l'interface existante (`templates/page.html`,
`style.css`, `app.js`) s'affiche dans la fenêtre.

**Étape 3/6 (faite)** : l'app charge la collection **en direct**, via une
vraie commande Tauri (`load_collection`) qui appelle le même code que
`bc_export` (`src/lib.rs`) — plus de fichier de données à régénérer à la
main.

**Étape 4/6 (faite)** : menus natifs — **File > Import a collection…**
ouvre le sélecteur macOS natif pour choisir une autre collection
(`.bkcp`/`.col`), et **NBC Help > About NBC** affiche la version. NBC
permet d'importer une collection de livres gérée par Book Collector for
Mac v20.3.1 — un `.bkcp` d'une autre version est refusé à l'import (sa
version est lue dans `preferences.plist`), puisque le schéma SQL de ce
projet ne cible que cette version précise.

⚠️ **Piège rencontré (corrigé en v0.32.1)** : le sélecteur utilisait
d'abord `pick_folder()` (sélection de dossier) en pensant que `.bkcp`/
`.col` devait être traité comme un dossier. En réalité macOS enregistre
`.bkcp` comme un **paquet** (type document), donc NSOpenPanel le traite
comme un *fichier* — avec un sélecteur limité aux dossiers
(`canChooseFiles: false`), le paquet apparaît grisé, impossible à
sélectionner. Rien à voir avec le choix d'extension `.col` vs `.bkcp` :
le même problème se serait posé avec `.col`. Corrigé en passant à
`pick_file()` avec un filtre d'extensions (`bkcp`, `col`) — le paquet
devient sélectionnable comme un élément unique, comme dans Finder.

```bash
cargo build --release        # si bc_export n'est pas déjà compilé (depuis la racine)
./app/generate-dev-data.sh   # synchronise app/src/{index.html,app.js,style.css}
cd app
cargo tauri dev      # lance une fenêtre de développement (rechargement à chaud)
cargo tauri build    # compile une vraie .app (et un .dmg), dans
                      # app/src-tauri/target/release/bundle/
```

⚠️ **Important** : `./app/generate-dev-data.sh` doit être relancé après
**toute** modification de `templates/app.js`/`style.css` avant de tester
— sinon `cargo tauri dev` tourne avec une copie obsolète dans
`app/src/`, sans erreur visible (juste un comportement qui ne correspond
plus au code). Piège déjà rencontré une fois (v0.32.0), source d'une
perte de temps de diagnostic ; vérifiable avec
`diff <(md5 -q templates/app.js) <(md5 -q app/src/app.js)`.

Le chemin de la collection chargée au démarrage reste `DEV_COLLECTION_PATH`
(`templates/app.js`, vers `testdata-sample/`) **seulement comme valeur de
repli** au tout premier lancement — "File > Import a collection…" le
remplace et le mémorise (`localStorage`) pour les lancements suivants.

**"Ajouter un livre" écrit désormais réellement dans la collection**
(v0.34.0), sans passer par le fichier `new_book.json`/`bc_add` manuel décrit
au § 7 — ce détour n'avait de sens que pour un usage technique, pas pour un
futur utilisateur de l'app. Le bouton devient "💾 Enregistrer" (au lieu de
"⬇️ Générer new_book.json") dès que `window.__TAURI__` existe ; l'export
HTML statique de `bc_export` garde son comportement de téléchargement
inchangé. Avant d'écrire, une boîte de confirmation résout le livre contre
la collection ouverte et liste chaque valeur de type auteur/éditeur/genre/
sujet/etc. qui serait créée pour la première fois (badge "nouveau") — pour
repérer une faute de frappe avant qu'elle devienne une entrée permanente.
Deux commandes IPC, `prepare_add_book` (lecture seule) et `commit_add_book`
(écrit, une fois confirmé), toutes deux implémentées en appelant
directement `book_collector_export::resolve_book`/`plan_book_sql`/
`write_plan_sql` (même code que `bc_add resolve`/`plan`/`commit`, partagé
via `src/lib.rs`) — et refusent **sans aucune exception possible** un
chemin qui ressemble à la vraie collection Book Collector, contrairement à
`bc_add commit --force` en CLI qui peut la forcer avec une confirmation
tapée au clavier.

**v0.44.0** : après l'écriture, l'app recharge désormais systématiquement
la collection et revient à la bibliothèque (comme le fait déjà "Modifier
ce livre") — avant, un ajout réussi n'était signalé que par un message
dans le formulaire, sans recharger la liste en mémoire : le livre était
bien en base, mais introuvable en recherche jusqu'au rechargement suivant
de la collection. Une boîte de dialogue confirme maintenant explicitement
le succès de l'ajout.

**v0.45.0** : "🗑️ Supprimer ce livre" ne demandait en réalité jamais
confirmation — le plugin dialog de Tauri remplace `window.confirm` par
une version asynchrone (une `Promise`, toujours "vraie" telle quelle),
donc un `if (!window.confirm(...))` sans `await` ne bloquait jamais rien
et la suppression continuait immédiatement. Corrigé (`await` ajouté) ;
confirmé que c'est le seul endroit de l'app qui supprime réellement des
données persistées.

**v0.45.1** : après ce correctif, "Supprimer" ne faisait plus rien du
tout. Cause : `app/src-tauri/capabilities/default.json` n'accordait pas
`dialog:default` à la fenêtre — nécessaire dès qu'une boîte de dialogue
est déclenchée **depuis le JavaScript** (`window.confirm`/`window.alert`),
contrairement aux dialogues déclenchés côté Rust (About NBC, erreur de
version) qui n'en ont pas besoin. Sans cette permission, l'appel était
silencieusement rejeté, et comme le `await window.confirm(...)` n'était
pas dans le `try/catch` de la suppression, l'erreur ne s'affichait pas.
Corrigé : permission ajoutée, et l'appel à `window.confirm` déplacé dans
le même `try/catch` que la suppression, pour qu'une défaillance future
s'affiche au lieu d'échouer en silence.

`app/src/index.html` reste gitignoré (généré par `generate-dev-data.sh`
depuis `app/src/index.template.html`, committé lui) mais ne contient plus
de données personnelles depuis l'étape 3 — exclu par précaution tout de
même, au cas où un futur changement y réintroduirait des données.

**Couvertures** : la fenêtre Tauri ne peut pas charger une image via une
URL `file://` brute (sandbox de sécurité de la WebView) — contrairement à
l'export HTML classique, ouvert directement en `file://` dans un
navigateur. Contournement : le protocole `asset` de Tauri, activé dans
`app/src-tauri/tauri.conf.json` + une fonction `resolveCoverSrc()` dans
`templates/app.js` qui ne s'active que si `window.__TAURI__` existe (donc
jamais dans l'export HTML classique).

**v0.43.0 — portée du protocole `asset` rendue dynamique.** Jusqu'ici
cette portée était une liste figée de chemins propres à la machine de
développement (`testdata/`, `~/Documents/Collectorz/`, etc.) — ça
fonctionnait ici, mais aurait silencieusement caché les couvertures de
toute collection ouverte ailleurs (bloquant pour un futur utilisateur
installant l'app depuis GitHub, dont la collection vivra forcément à un
autre endroit). `tauri.conf.json` a maintenant une portée vide
(`"scope": []`, refus par défaut) ; `load_collection`
(`app/src-tauri/src/lib.rs`) autorise désormais dynamiquement, à chaque
ouverture de collection, uniquement le dossier `.bkcp`/`.col` réellement
ouvert — fonctionne pour n'importe quel emplacement, sans liste à
maintenir à l'avance.

**v0.38.0 — Supprimer un livre.** Bouton "🗑️ Supprimer ce livre" dans le
panneau de détail (app NBC uniquement) — confirmation native, sauvegarde
horodatée avant écriture, mêmes garde-fous que l'ajout (jamais la vraie
collection, refus si Book Collector est lancé). Supprime la fiche et ses
propres lignes (crédits, personnages, contenus, liens, jointures genres/
sujets/tags) sans jamais toucher aux valeurs partagées (éditeur, genre,
auteur...) même si le livre en était le seul utilisateur.

**v0.39.0 — Modifier un livre.** Bouton "✏️ Modifier ce livre", à côté de
"Supprimer", ouvre le même formulaire "Ajouter un livre" mais pré-rempli
avec les valeurs actuelles de la fiche (titre "Modifier : …", bouton
"Enregistrer les modifications"), avec la même étape de confirmation avant
écriture. Supprime les anciennes relations de la fiche puis les recrée avec
les valeurs courantes — toute nouvelle valeur "liste" tapée dans le
formulaire est créée exactement comme lors d'un ajout. **Le chantier
"modifier/supprimer un livre" est maintenant complet.**

**v0.40.0 — Icône de l'app (étape 5/6, faite).** Le logo générique du
scaffold Tauri est remplacé par un glyphe de livre ouvert blanc sur fond
bleu accent, cohérent avec le reste de la refonte visuelle. Régénérable
via `cd app && cargo tauri icon app-icon.png` si le design doit changer
(source 1024×1024 conservée dans `app/app-icon.png`).

Étape suivante, pas encore commencée : signature/notarisation (étape 6 —
pas urgent, ne compte que pour une distribution à quelqu'un d'autre).

**v0.41.0 — Fiche détail complétée ("Détails produit"/"Détails
personnels"/"Crédits"/"Personnages"/"Contenus").** Déclenché par une
comparaison directe NBC/Book Collector sur un même livre : "Product
Details" et "Personal Details" manquaient entièrement de la fiche NBC.
L'audit a aussi mis à jour ~9 champs (série, édition, imprimeur, volume,
type de papier, propriétaire, magasin, lu, date d'ajout) déjà résolus en
SQL depuis des travaux antérieurs mais jamais branchés sur l'affichage —
silencieusement invisibles jusqu'ici. La fiche détail affiche maintenant
trois groupes clé/valeur ("Détails produit", "Édition originale",
"Détails personnels", champs vides omis) et quatre listes répétables
("Crédits", "Personnages", "Contenus", "Liens").

⚠️ **Limite de vérification connue** : les sections Crédits/Personnages/
Contenus n'ont pas pu être confirmées à l'écran faute de données — aucune
fiche de `testdata/` n'a de crédit/personnage/contenu (`ZCREATOR`/
`ZCHARACTER`/`ZSTORY` vides), et créer une fiche de test dans
`testdata-rw/` était bloqué par le garde-fou "Book Collector est
actuellement lancé" au moment des travaux. Le chargement Rust réutilise
les mêmes jointures déjà vérifiées pour l'édition de fiche, et le rendu
JS suit le même motif que "Liens" (confirmé) — à vérifier dès qu'une
fiche réelle avec ce type de données sera disponible, ou en créant une
fiche de test dans `testdata-rw/` une fois Book Collector fermé.

## 9. Dépannage

- **`--db "~/Documents/..."` échoue** : si le chemin est entre guillemets,
  le shell n'étend pas `~` lui-même — l'outil le fait à votre place depuis
  la v0.3.0, donc ce cas est déjà géré.
- **Les couvertures ne s'affichent pas dans le navigateur** : la première
  fois qu'un navigateur accède à un dossier protégé par macOS (ex.
  `~/Documents`), une autorisation système est demandée une seule fois —
  l'accepter puis recharger la page.
- **Erreur de verrou SQLite** : fermez Book Collector avant de lancer un
  export, ou utilisez `testdata/` qui est une copie indépendante.

## 10. Pour aller plus loin

- `CHANGELOG.md` : historique détaillé, version par version.
- `./target/release/bc_export --help` : liste complète des options.
