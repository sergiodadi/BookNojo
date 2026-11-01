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

## 7. Dépannage

- **`--db "~/Documents/..."` échoue** : si le chemin est entre guillemets,
  le shell n'étend pas `~` lui-même — l'outil le fait à votre place depuis
  la v0.3.0, donc ce cas est déjà géré.
- **Les couvertures ne s'affichent pas dans le navigateur** : la première
  fois qu'un navigateur accède à un dossier protégé par macOS (ex.
  `~/Documents`), une autorisation système est demandée une seule fois —
  l'accepter puis recharger la page.
- **Erreur de verrou SQLite** : fermez Book Collector avant de lancer un
  export, ou utilisez `testdata/` qui est une copie indépendante.

## 8. Pour aller plus loin

- `CHANGELOG.md` : historique détaillé, version par version.
- `./target/release/bc_export --help` : liste complète des options.
