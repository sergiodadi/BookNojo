# Outils en ligne de commande — `bc_export` / `bc_add`

> 🇬🇧 [Read in English](CLI.md)

> Pour un usage quotidien, ces commandes ne sont **pas nécessaires** :
> l'application (voir [`MANUAL.fr.md`](MANUAL.fr.md)) suffit pour consulter,
> ajouter, modifier et supprimer des livres. Ce document s'adresse à qui
> veut exporter une collection (CSV/JSON/HTML), scripter des ajouts, ou
> amorcer une base indépendante de Book Collector — un usage plus
> technique, optionnel.

## 1. Présentation

- **`bc_export`** lit une collection [Book Collector](https://www.collectorz.com/book)
  (Collectorz) en lecture seule et produit un export **CSV**, un export
  **JSON**, et en option une **page HTML autonome** reproduisant l'aspect
  de l'application (§ 3).
- **`bc_add`** ajoute, planifie ou transfère des livres en ligne de
  commande (§ 4), avec les mêmes garde-fous de sécurité que l'application.

`bc_export` ouvre toujours la base en lecture seule
(`SQLITE_OPEN_READ_ONLY`) : il ne peut jamais modifier la collection
originale.

Les commandes ci-dessous supposent que vous êtes dans le dossier
contenant les binaires compilés (`project/target/release/` si compilé
depuis les sources — voir [`BUILD.fr.md`](BUILD.fr.md)).

## 2. `bc_export` — export de base

```bash
./bc_export \
  --db "~/Documents/Collectorz/Book Collector/MyCollection.bkcp" \
  --csv book_collection.csv \
  --json book_collection.json
```

`--db` accepte soit le dossier `.bkcp`/`.col`, soit directement le fichier
`Book_data` qu'il contient.

### Sous-collections

Un fichier `.bkcp`/`.col` peut contenir **plusieurs sous-collections**.

```bash
# Lister les sous-collections et leurs compteurs
./bc_export --db "<chemin>" --list-collections

# N'exporter qu'une seule sous-collection
./bc_export --db "<chemin>" --collection "<nom>" \
  --csv books.csv --json books.json
```

Le compteur affiché par l'application est la **somme des quantités** de
la sous-collection active, pas le nombre de fiches — les deux valeurs
peuvent légitimement différer.

## 3. La page HTML interactive

```bash
./bc_export --db "<chemin>" \
  --csv /tmp/bc.csv --json /tmp/bc.json \
  --html ~/Documents/book_collection_preview.html
```

S'ouvre avec n'importe quel navigateur (double-clic, ou `open`), sans
serveur. Trois zones, comme l'application :

- **à gauche** : regroupement (Auteur par défaut, ou Genre / Éditeur /
  Format / Langue / Pays via le menu déroulant), avec barre A→Z. Cliquer
  sur le nombre à droite de "Tous" bascule la liste des valeurs entre tri
  alphabétique et tri par nombre de livres décroissant.
- **en haut à droite** : liste des livres du groupe sélectionné —
  recherche, tri, pagination, bascule vue liste/grille.
- **en bas à droite** : fiche détail (couverture, métadonnées, genres
  cliquables, résumé, notes).

Les onglets en haut changent de sous-collection sans relancer la
commande. Le bouton **Statistiques** affiche une vue agrégée (genres,
formats, langues, éditeurs, décennies) pour la sous-collection active. Un
sélecteur **FR / EN** dans l'en-tête bascule toute l'interface entre
français et anglais. La disposition, la dernière position de navigation
et la langue sont mémorisées d'une ouverture à l'autre (stockage local du
navigateur).

Le bouton **➕ Ajouter un livre** ouvre un formulaire (détail des champs
en § 4) ; dans cet export statique, il **ne contacte aucun serveur et
n'écrit rien** — "Générer new_book.json" télécharge simplement le fichier,
à passer ensuite à `bc_add` (§ 4). Dans l'application macOS, le même
formulaire écrit directement dans la collection ouverte (voir
[`MANUAL.fr.md`](MANUAL.fr.md)).

## 4. `bc_add` — ajouter un livre en ligne de commande

> ⚠️ **Testez toujours d'abord sur une copie de votre collection, jamais
> directement sur l'originale.** Par défaut, `bc_add` refuse d'écrire
> sur un chemin qui ressemble à la vraie collection Book Collector
> (`~/Documents/Collectorz/...`) — voir § "Garde-fous" ci-dessous. Si
> vous développez sur ce projet, `BUILD.fr.md` explique comment générer des
> copies de test dédiées.

### Usage courant — `bc_add add`

```bash
./bc_add add \
  --db "<chemin vers votre collection>" \
  --input new_book.json
```

Exemple minimal de `new_book.json` (champs de l'onglet "Main" de "Add
Book") :

```json
{
  "title": "Le Nom du Vent",
  "authors": ["Patrick Rothfuss"],
  "publisher": "Bragelonne",
  "format": "Broché",
  "language": "français",
  "country": "France",
  "genres": ["Fantasy"],
  "collection": "<nom de la sous-collection>"
}
```

La sous-collection (`collection`) doit déjà exister — contrairement aux
autres champs, `bc_add` ne sait pas en créer une nouvelle.

`bc_add add` enchaîne en une seule commande résolution, planification et
écriture (voir le détail étape par étape ci-dessous si besoin de
contrôler chaque étape séparément, par exemple pour relire le SQL avant
de l'exécuter).

Elle accepte aussi `--also-db` pour répéter tout le pipeline contre une
**seconde** base — typiquement la vraie collection Book Collector, tant
qu'elle existe encore (voir § "Pourquoi une base indépendante"). Chaque
base est résolue et planifiée **séparément** : jamais le même SQL
recopié d'une base à l'autre, puisque les clés primaires et les valeurs
déjà connues (éditeurs, genres...) divergent dès que deux bases évoluent
indépendamment.

```bash
./bc_add add \
  --db "<votre base BookNojo>" \
  --also-db "~/Documents/Collectorz/Book Collector/MyCollection.bkcp" \
  --input new_book.json \
  --force
```

`--force` s'applique aux deux bases si l'une ou l'autre ressemble à la
vraie collection ; la confirmation `OUI` (voir "Garde-fous") reste exigée
à chaque exécution. Si la base principale échoue, rien n'est tenté sur la
seconde. Si c'est la seconde qui échoue, la première reste acquise — les
fichiers `resolved`/`plan` de la tentative sont conservés dans
`bc_add_runs/<horodatage>/`, rejouables plus tard avec `bc_add commit`
sans tout recommencer.

### Tous les champs disponibles

Au-delà de l'onglet "Main" (exemple ci-dessus), tous les champs des
autres onglets de "Add Book" sont disponibles dans le JSON d'entrée, tous
optionnels — seuls ceux fournis sont pris en compte. Couvertures (gestion
de fichiers) n'est pas couvert.

- **Détails** : `edition`, `printed_by`, `series`, `volume`, `paper_type`
  (listes), `height`, `width`, `dewey`, `lccn`, `dedication` (texte
  libre), `first_edition`, `printing`, `abridged`, `issue_number`,
  `box_set`, `audio_length_minutes`, `original_title`,
  `original_subtitle`, `original_publisher`, `original_country`,
  `original_language`, `original_publication_year/month/day`.
- **Personnel** : `condition`, `owner` (personne), `store`,
  `current_value`, `purchase_price`, `cover_price`,
  `purchase_year/month/day`, `rare`, `read_it`, `read_times`,
  `read_year/month/day`, `my_rating`, `location_number`,
  `user_lookup1/2`, `user_text1/2`.
- **Intrigue & Notes** : `plot`, `notes`.
- **Crédits & Personnages** : `credits` (liste de `{"name": ...}`) et
  `characters` (liste de `{"name": ..., "alias": ...}`).
- **Contenus** : `contents`, liste de
  `{"title": ..., "subtitle": ..., "number_of_pages": ..., "start_page": ..., "authors": [...]}`
  — plusieurs histoires/nouvelles dans un même livre, chacune avec ses
  propres auteurs.
- **Liens** : `links`, liste de `{"url": ..., "description": ...}`.

```json
{
  "...": "... champs de l'onglet Main ...",
  "plot": "Résumé de l'histoire.",
  "notes": "Exemplaire dédicacé.",
  "condition": "Mint",
  "credits": [{ "name": "Nom du traducteur" }],
  "characters": [{ "name": "Nom du personnage", "alias": "Surnom" }],
  "contents": [{ "title": "Titre de la nouvelle", "authors": ["Nom de l'auteur"] }],
  "links": [{ "url": "https://...", "description": "Description du lien" }]
}
```

Champs volontairement non implémentés (listés en détail par
`bc_add plan`) : le **rôle d'un crédit** (`ZCREATOR.ZROLEID`) — aucune
étiquette correspondante n'existe dans `Book_data`, laissé NULL ;
`ZUSERDEFINEDVALUESXML` et les champs de `ZSTORY` autres que
`start_page` (Série/Intrigue/Notes/Édition originale par contenu) —
aucune donnée réelle n'a permis d'en confirmer le comportement exact.

### Étape par étape (`resolve`, `plan`, `guard`, `commit`)

Pour contrôler chaque étape séparément plutôt que `bc_add add` :

```bash
# 1. Résout les valeurs de type liste (auteur, éditeur, genre...) contre
#    la collection : existe déjà, ou serait créée.
./bc_add resolve --db "<chemin>" --input new_book.json --output resolved-book.json

# 2. Calcule et affiche le SQL d'insertion exact, sans jamais l'exécuter.
./bc_add plan --db "<chemin>" --resolved resolved-book.json --output plan.sql

# 3. Vérifie que Book Collector n'est pas lancé, et sauvegarde la base
#    (horodatée) avant toute écriture.
./bc_add guard --db "<chemin>"

# 4. Exécute le SQL de `plan`, dans une transaction unique (tout ou rien).
./bc_add commit --db "<chemin>" --plan plan.sql
```

`plan` termine sa sortie par la liste explicite de ce qu'il ne couvre pas
encore (ex. la ligne compagnon `ZSEARCHSTRING`, le tri de titre, les
identifiants de synchronisation CLZ Cloud) — à relire avant `commit`.

### Garde-fous

Avant d'écrire, `commit` (et donc `add`) vérifie dans l'ordre :

1. le chemin de la base **ne ressemble pas** à la vraie collection
   Book Collector (`~/Documents/Collectorz/...`) — refus immédiat sinon ;
2. Book Collector **n'est pas lancé** (écrire pendant qu'il tourne
   risquerait un fichier WAL incohérent, ou qu'il écrase vos
   changements à sa prochaine sauvegarde) ;
3. une sauvegarde horodatée de `Book_data` (et `-wal`/`-shm` si présents)
   est créée dans `backups/`, à côté de la base ;
4. le SQL s'exécute dans une seule transaction.

`--force` lève le refus n°1, mais **n'écrit toujours pas directement** :
une confirmation tapée à la main dans le terminal prend le relais :

```
⚠️  ATTENTION : .../MyCollection.bkcp/Book_data se trouve sous
.../Documents/Collectorz — ceci est la VRAIE collection Book Collector,
pas une copie de test.
   Une sauvegarde horodatée sera prise avant d'écrire, mais cette opération
   modifie directement vos données réelles.
   Tapez exactement OUI pour continuer (autre chose annule) :
```

Toute réponse différente de `OUI` (lettres capitales exactes) annule
l'opération — aucune écriture n'a lieu, même avec `--force`.

Avant d'utiliser `--force` sur la vraie collection :
1. Fermez Book Collector.
2. Faites une sauvegarde manuelle en plus de celle, automatique, que
   `commit` effectue (ex. copier tout le dossier `.bkcp`).
3. Relisez le `plan.sql` produit par l'étape "plan" — dernière occasion
   de repérer un problème avant une écriture réelle.
4. Lancez `commit --force` (ou `add --force`) et tapez `OUI` seulement si
   tout est en ordre.

### Pourquoi une base indépendante de Book Collector

Book Collector pour Mac est une application Intel qui dépend de Rosetta 2
pour fonctionner sur Apple Silicon. Apple a annoncé la fin programmée de
Rosetta 2 : support complet jusqu'à macOS 27 "Golden Gate" (automne 2026),
largement retiré à partir de macOS 28 (automne 2027). Book Collector a
donc une espérance de vie limitée sur un Mac à jour, sauf mise à jour de
l'éditeur vers Apple Silicon natif.

L'objectif à terme est une base **indépendante** de l'application —
amorcée une fois depuis la vraie collection (§ "`bc_add import`"), puis
alimentée uniquement par BookNojo, sans jamais dépendre de Book Collector
pour continuer à grandir. `--also-db` sert la période de transition :
continuer à alimenter la vraie application tant qu'elle tourne encore, en
parallèle de la nouvelle base. L'extension retenue pour cette base
persistante est **`.col`** — elle fonctionne exactement comme un `.bkcp`
(seule la présence d'un fichier `Book_data` à l'intérieur compte, jamais
le nom du dossier).

### `bc_add import` — le transfert BC vers BookNojo

Amorce une nouvelle base `.col` indépendante à partir d'une collection
existante (base + dossier `Images/`). Ne sert qu'**une seule fois** :
refuse d'écraser une destination qui existe déjà.

```bash
./bc_add import --source "~/Documents/Collectorz/Book Collector/MyCollection.bkcp"
# -> collection/MyCollection.col (destination par défaut)

# Ou avec une destination explicite :
./bc_add import \
  --source "~/Documents/Collectorz/Book Collector/MyCollection.bkcp" \
  --dest ~/MaCollectionBookNojo.col
```

### Langue des messages (`--lang`)

```bash
./bc_add --lang en import --source ... --dest ...
```

Amorce d'internationalisation : pour l'instant, seule `import` a des
messages en français et en anglais (`fr` par défaut). Le reste de l'outil
reste en français.

## 5. Dépannage

- **`--db "~/Documents/..."` échoue** : si le chemin est entre
  guillemets, le shell n'étend pas `~` lui-même — l'outil le fait à votre
  place.
- **Les couvertures ne s'affichent pas dans le navigateur** (export HTML
  statique) : la première fois qu'un navigateur accède à un dossier
  protégé par macOS (ex. `~/Documents`), une autorisation système est
  demandée une seule fois — l'accepter puis recharger la page.
- **Erreur de verrou SQLite** : fermez Book Collector avant de lancer un
  export ou un ajout.

## 6. Pour aller plus loin

- [`MANUAL.fr.md`](MANUAL.fr.md) : utiliser l'application — suffisant pour un
  usage quotidien, sans ces commandes.
- [`BUILD.fr.md`](BUILD.fr.md) : compiler les outils et l'application depuis
  les sources (voir aussi son annexe sur le modèle de données).
- [`CHANGELOG.fr.md`](CHANGELOG.fr.md) : historique des versions publiées sur
  GitHub.
- `./bc_export --help` : liste complète des options.
