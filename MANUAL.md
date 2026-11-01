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

## 7. (Expérimental) Préparer l'ajout d'un livre — étapes 1 et 2 sur 6

> ⚠️ En cours de construction, par étapes. Ces deux étapes ne font **aucune
> écriture** dans la base — `bc_add` ouvre `Book_data` en lecture seule, au
> même titre que `bc_export`, quelle que soit la sous-commande. L'écriture
> réelle (étapes suivantes) ne sera ajoutée qu'après des garde-fous
> explicites (sauvegarde automatique, vérification que Book Collector n'est
> pas lancé, transaction unique) et sera d'abord testée sur une copie
> inscriptible dédiée, jamais sur la vraie collection.

### Étape 1 — `bc_add resolve`

Prend un fichier JSON décrivant un nouveau livre (champs de l'onglet
"Main" de la fenêtre "Add Book" de l'application) et vérifie, pour chaque
valeur de type liste (auteur, éditeur, genre, format, langue, pays, sujet,
emplacement), si elle existe déjà dans la collection ou si elle serait
créée.

### Produire ce JSON : le formulaire dans la page HTML

Pas besoin d'écrire le JSON à la main : la page générée par
`--html` (§ 5) a un bouton **➕ Ajouter un livre** dans l'en-tête. Le
formulaire reprend les champs ci-dessus, avec auto-complétion sur les
valeurs déjà utilisées dans la collection (pour que l'orthographe
corresponde exactement et que `bc_add` reconnaisse bien une valeur
existante). Il **ne contacte aucun serveur et n'écrit rien** : cliquer sur
"Générer new_book.json" télécharge simplement le fichier, à passer ensuite
à `bc_add` comme décrit ci-dessous.

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
