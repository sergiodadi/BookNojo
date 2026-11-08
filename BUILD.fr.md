# Compiler BookNojo depuis les sources

> 🇬🇧 [Read in English](BUILD.md)

Ce document explique comment compiler les outils en ligne de commande et
l'application macOS depuis le code source, et comment reproduire
l'environnement de développement. Pour **utiliser** le logiciel une fois
compilé, voir [`MANUAL.fr.md`](MANUAL.fr.md) (l'application — suffisant pour un
usage quotidien) et [`CLI.fr.md`](CLI.fr.md) (les outils en ligne de
commande `bc_export`/`bc_add`, optionnels).

## Prérequis

- **Rust** (`cargo`) — installable via Homebrew si besoin :
  `brew install rust`.
- **Tauri CLI**, uniquement pour l'application macOS (pas nécessaire pour
  les outils CLI seuls) :
  ```bash
  cargo install tauri-cli --version "^2.0.0"
  ```

Tout le code et les manifests vivent dans **`project/`** — la racine du
dépôt ne contient que la documentation (`README.md`, `BUILD.md`,
`MANUAL.md`, `CLI.md`, `CHANGELOG.md`, chacun avec son équivalent
français `.fr.md`). Toutes les commandes ci-dessous supposent que vous
êtes dans `project/`.

## Outils en ligne de commande (`bc_export`, `bc_add`)

```bash
cd booknojo/project
cargo build --release
```

Les binaires sont alors `target/release/bc_export` et
`target/release/bc_add` (donc `project/target/release/...` depuis la
racine du dépôt).

```bash
cargo test --release    # lance les tests unitaires ET d'intégration
```

## Application macOS

```bash
cd project                   # si pas déjà fait
cargo build --release        # si pas déjà fait
./app/generate-dev-data.sh   # synchronise app/src/{index.html,app.js,style.css}
                              # depuis templates/ et app/src/index.template.html
cd app
cargo tauri dev      # fenêtre de développement, rechargement à chaud
cargo tauri build    # compile une vraie .app (et un .dmg), dans
                      # app/src-tauri/target/release/bundle/
```

⚠️ **Important** : `./app/generate-dev-data.sh` doit être relancé après
**toute** modification de `templates/app.js`/`style.css` ou de
`app/src/index.template.html`, avant `cargo tauri dev`/`build` — sinon la
fenêtre tourne avec une copie obsolète dans `app/src/`, sans erreur
visible. Vérifiable avec `diff <(md5 -q templates/app.js) <(md5 -q app/src/app.js)`
(depuis `project/`).

`cargo build --release` (depuis `project/`) ne produit **pas**
d'application installable — seul `cargo tauri build` (dans
`project/app/`) le fait. Pensez à le relancer avant de distribuer une
nouvelle version.

### Icône de l'application

Régénérable depuis une image source 1024×1024 (`app/app-icon.png`) :

```bash
cd project/app
cargo tauri icon app-icon.png
```

## Données de test (développement uniquement)

Ces copies et scripts ne sont nécessaires que pour contribuer au projet —
pas pour utiliser le logiciel sur sa propre collection. Depuis
`project/` :

```bash
./refresh-testdata.sh        # testdata/ : copie en lecture seule (chmod 444)
./refresh-testdata-rw.sh     # testdata-rw/ : copie inscriptible, pour tester les écritures
./build-sample-testdata.sh   # testdata-sample/ : extrait (~100 fiches) pour itérer vite
```

`sample-collection/` (committée dans le dépôt, entièrement fictive — voir
[`MANUAL.fr.md`](MANUAL.fr.md) § 1) est régénérable avec :

```bash
cargo run --release --example generate_sample_collection
```

Aucun de ces scripts ne touche jamais le fichier réel de l'utilisateur
(`~/Documents/Collectorz/Book Collector/...`) — règle du projet, vérifiée
par des tests dédiés.

## Structure du dépôt

```
README.md, BUILD.md, MANUAL.md,     documentation (racine du dépôt),
CLI.md, CHANGELOG.md                chacun avec son .fr.md en français

project/
├─ src/            bibliothèque partagée + binaires CLI (bc_export, bc_add)
├─ app/            application macOS (Tauri) — réutilise src/ via une
│                  dépendance de chemin
├─ templates/      page HTML/JS/CSS partagée entre l'export statique
│                  (bc_export --html) et l'application
├─ examples/       génération de la collection d'exemple (sample-collection/)
├─ tests/          tests d'intégration (ajout/modification/suppression)
├─ Cargo.toml      manifeste du crate racine (book_collector_export)
└─ refresh-testdata*.sh, build-sample-testdata.*   scripts de données de test
```

## Annexe : modèle de données Book Collector

> ⚠️ **Donné à titre indicatif** — ce catalogue de champs peut être
> inexact ou incomplet. Il décrit l'export XML intégré de Book Collector
> (menu Export, format XML), pas le format de stockage réel utilisé par
> le reste de ce projet.

Chaque livre exporté en XML déclare, en tête de fichier, la liste des
champs qu'il connaît (balise `<field name="..." id="..." label="...">`).
108 champs au total, dans l'ordre où Book Collector les déclare :

| Id interne | Libellé |
|---|---|
| `dfTitleSort` | Sort Title |
| `dfForewordAuthor` | Foreword Author |
| `dfDewey` | Dewey |
| `dfSubTitle` | Sub Title |
| `dfPhotographer` | Photographer |
| `dfAudioBookLinks` | Audio Book Files |
| `dfCountry` | Country |
| `dfPurchaseYear` | Purchase Year |
| `dfISBN` | ISBN |
| `dfNarrator` | Narrator |
| `dfBookLinks` | Book Files |
| `dfLanguage` | Language |
| `dfReadingYear` | Reading Year |
| `dfLastModified` | Last Modified |
| `dfFormat` | Format |
| `dfeBookLinks` | eBook Files |
| `dfGenre` | Genre |
| `dfSeries` | Series |
| `dfLCCN` | LoC Classification |
| `dfImageLinks` | Interior Art Files |
| `dfSubject` | Subject |
| `dfIssueNr` | Issue No. |
| `dfPageCount` | No. Of Pages |
| `dfPrinting` | Printing |
| `dfCollectionStatus` | Collection Status |
| `dfCharacters` | Characters |
| `dfBoxSet` | Box Set |
| `dfPublisher` | Publisher |
| `dfDimWidth` | Width |
| `dfOtherLinks` | Other Files |
| `dfIndex` | Index |
| `dfOrigPublicationYear` | Original Publication Year |
| `dfVolume` | Volume |
| `dfPublicationDate` | Publication Date |
| `dfDimHeight` | Height |
| `dfPublicationYear` | Publication Year |
| `dfQuantity` | Quantity |
| `dfReadTimes` | Times Read |
| `dfCredits` | Credits |
| `dfAudioLength` | Length |
| `dfOrigTitle` | Original Title |
| `dfSubmissionDate` | Last Submission Date |
| `dfMyRating` | My Rating |
| `dfReadingDate` | Reading Date |
| `dfEdition` | Edition |
| `dfOrigSubTitle` | Original Sub Title |
| `dfBPBookID` | Clz Book ID |
| `dfReadIt` | Read It |
| `dfFirstEdition` | First Edition |
| `dfCondition` | Condition |
| `dfLoaner` | Loaned To |
| `dfOrigPublisher` | Original Publisher |
| `dfOwner` | Owner |
| `dfPrintedBy` | Printed By |
| `dfCurrentPrice` | Current Value |
| `dfLoanDate` | Loan Date |
| `dfOrigPublicationDate` | Original Publication Date |
| `dfLocation` | Location |
| `dfPaperType` | Paper Type |
| `dfDueDate` | Due Date |
| `dfOrigCountry` | Original Country |
| `dfPlot` | Plot |
| `dfDedication` | Dedication |
| `dfIsOverdue` | isOverdue |
| `dfOrigLanguage` | Original Language |
| `dfNotes` | Notes |
| `dfPurchaseDate` | Purchase Date |
| `dfReturnDate` | Return Date |
| `dfExtras` | Extras |
| `dfLinks` | Local Links |
| `dfPurchasePrice` | Purchase Price |
| `dfSyncFrontCover` | Custom Image |
| `dfLoanNotes` | Loan Notes |
| `dfOnlineLinks` | Online Links |
| `dfStore` | Store |
| `dfAddedDate` | Added Date |
| `dfAuthor` | Author |
| `dfTag` | Tags |
| `dfLoanEmail` | Loaned To Email |
| `dfAbridged` | Abridged |
| `dfCoverPrice` | Cover Price |
| `dfCollection` | Collection |
| `dfTranslator` | Translator |
| `dfCoverFront` | FroBMnt Cover |
| `dfUserRole1` | User Role 1 |
| `dfLoanAddress` | Loaned To Address |
| `dfRare` | Rare |
| `dfSyncBackCover` | Custom Back Cover |
| `dfCoverArtist` | Cover Artist |
| `dfCoverBack` | Back Cover |
| `dfUserRole2` | User Role 2 |
| `dfLoanTitle` | Loan Title |
| `dfStartPage` | Start Page |
| `dfEditor` | Editor |
| `dfBackdrop` | Backdrop |
| `dfUserRole3` | User Role 3 |
| `dfFrequency` | Frequency |
| `dfIllustrator` | Illustrator |
| `dfConnectHash` | ConnectHash |
| `dfAuthorFirstLetter` | Author First Letter |
| `dfTitleFirstLetter` | Title First Letter |
| `dfTitle` | Title |
| `dfGhostwriter` | Ghostwriter |
| `dfLoCNr` | LoC Control Number |
| `dfInCollectionBit` | In Collection (0/1) |
| `dfThumbFilePath` | Thumbnail |
| `dfBPBookLastReceivedRevision` | BPBookLastReceivedVersion |
| `dfDynRating` | My Rating |

Chaque livre, au-delà des champs simples ci-dessus, contient aussi des
listes répétables (un auteur peut en avoir plusieurs, un livre peut avoir
plusieurs crédits, etc.) :

- `<authors>` → `<author>` (rôle + `<person>` : nom affiché, nom de tri,
  prénom, nom, date de naissance/décès)
- `<credits>` → crédits avec rôle, même structure `<person>`
- `<characters>` → personnages
- `<genres>`, `<subjects>`, `<tags>` → listes de valeurs simples
- `<links>` → `<link>` (url, description, type)
- `<original>` → bloc "Édition originale" (titre, sous-titre, éditeur,
  pays, langue, date de publication)
