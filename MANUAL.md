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

## 6. Copie de travail et tests

Pour ne jamais lire ni risquer de modifier le fichier original pendant les
tests, le projet garde une copie en lecture seule dans `testdata/` :

```bash
./refresh-testdata.sh      # resynchronise testdata/ depuis la vraie collection
cargo test --release       # lance les tests unitaires
```

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
tables, vides dans la collection de test — voir § 7, étape 5). Il **ne
contacte aucun serveur et n'écrit rien** : cliquer sur "Générer new_book.json" télécharge
simplement le fichier, à passer ensuite à `bc_add` comme décrit ci-dessous.
Changer d'onglet ne perd aucune saisie : un seul envoi du formulaire
récupère les champs de tous les onglets.

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

## 8. Dépannage

- **`--db "~/Documents/..."` échoue** : si le chemin est entre guillemets,
  le shell n'étend pas `~` lui-même — l'outil le fait à votre place depuis
  la v0.3.0, donc ce cas est déjà géré.
- **Les couvertures ne s'affichent pas dans le navigateur** : la première
  fois qu'un navigateur accède à un dossier protégé par macOS (ex.
  `~/Documents`), une autorisation système est demandée une seule fois —
  l'accepter puis recharger la page.
- **Erreur de verrou SQLite** : fermez Book Collector avant de lancer un
  export, ou utilisez `testdata/` qui est une copie indépendante.

## 9. Pour aller plus loin

- `CHANGELOG.md` : historique détaillé, version par version.
- `./target/release/bc_export --help` : liste complète des options.
