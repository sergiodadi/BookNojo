# Manuel d'utilisation — application BookNojo

> 🇬🇧 [Read in English](MANUAL.md)

Ce document explique comment utiliser l'application macOS **BookNojo**
une fois installée. Pour la télécharger, voir [`README.fr.md`](README.fr.md) ;
pour la compiler depuis les sources, voir [`BUILD.fr.md`](BUILD.fr.md).

## 1. Ouvrir une collection

- **File > Import a collection…** ouvre le sélecteur de fichier natif
  macOS pour choisir une collection — un dossier `.bkcp` (Book Collector)
  ou `.col` (BookNojo). Le choix est mémorisé automatiquement : au lancement
  suivant, BookNojo rouvre la même collection sans rien demander.
- Une collection `.bkcp` d'une version de Book Collector différente de
  celle contre laquelle BookNojo a été construit (20.3.1) est refusée à
  l'import, avec un message clair plutôt qu'un plantage ou une corruption
  silencieuse.
- Au tout premier lancement, avant tout import, BookNojo ouvre une collection
  d'exemple entièrement fictive (10 livres inventés, aucune donnée
  personnelle) — pour qu'il y ait toujours quelque chose à voir, sans
  jamais exposer de vraies données par défaut.
- L'en-tête affiche en permanence le nom du fichier ouvert et la version
  de l'application.

## 2. Consulter la bibliothèque

- **Trois zones** : à gauche le regroupement, en
  haut à droite la liste des livres du groupe sélectionné, en bas à
  droite la fiche détail du livre choisi. Les séparateurs entre les trois
  zones se redimensionnent à la souris, et la disposition est mémorisée
  d'une ouverture à l'autre.
- **Regroupement** par Auteur (par défaut), Genre, Éditeur, Format,
  Langue ou Pays, avec une barre alphabétique A→Z pour sauter directement
  à une lettre.
- Cliquer sur le nombre à côté de "Tous" bascule la liste des groupes
  entre tri alphabétique et tri par nombre de livres décroissant.
- **Liste des livres** : recherche texte, tri, pagination, bascule entre
  vue liste et vue grille (vignettes de couverture).
- **Plusieurs sous-collections** dans un même fichier (ex. "Livres" /
  "BD") — des onglets en haut permettent de passer de l'une à l'autre
  sans rien recharger à la main.
- **Statistiques** : une vue agrégée (genres, formats, langues,
  éditeurs, décennies) pour la sous-collection actuellement affichée.
- **Fiche détail d'un livre** : couverture, titre/auteurs/éditeur et
  autres informations principales, puis — quand ils sont renseignés —
  Détails produit (dimensions, Dewey/LCCN, premier tirage, édition
  limitée...), Édition originale, Détails personnels (état, prix, date
  d'achat, note personnelle, lu ou non...), Crédits, Personnages, Contenus
  (nouvelles/histoires à l'intérieur d'un recueil), Liens, Intrigue et
  Notes. Les champs vides ou les sections sans aucune valeur ne
  s'affichent pas.

## 3. Ajouter, modifier, supprimer un livre

- **Ajouter un livre** : un formulaire complet organisé en sous-onglets
  (Principal, Détails, Personnel, Crédits & Personnages, Contenus,
  Intrigue & Notes, Liens), avec auto-complétion sur les valeurs déjà
  utilisées dans la collection (auteur, éditeur, genre, format, langue,
  pays, série, etc.). Avant d'écrire quoi que ce soit, une boîte de
  confirmation résout le livre contre la collection ouverte et liste
  chaque valeur qui serait créée pour la première fois (badge "nouveau")
  — pour repérer une faute de frappe avant qu'elle devienne une entrée
  permanente de la collection. L'écriture n'a lieu qu'après confirmation.
- **Modifier un livre** : depuis sa fiche détail, le même formulaire
  s'ouvre pré-rempli avec les valeurs actuelles ; la sauvegarde remplace
  les anciennes valeurs par les nouvelles, avec la même étape de
  confirmation.
- **Supprimer un livre** : depuis sa fiche détail, après confirmation.
  Seules les lignes propres à ce livre sont retirées — une valeur
  partagée (un éditeur, un genre...) n'est jamais supprimée, même si ce
  livre en était le seul utilisateur.
- Après tout ajout, modification ou suppression, la bibliothèque est
  automatiquement rechargée : le livre apparaît (ou disparaît)
  immédiatement dans la liste et la recherche, sans action manuelle.

## 4. Interface bilingue

- Un sélecteur **FR / EN** dans l'en-tête bascule toute l'interface —
  regroupement, liste, fiche détail, statistiques, formulaire
  d'ajout/modification — entre français et anglais. Le choix est
  mémorisé d'une ouverture à l'autre.
- Les messages renvoyés par l'application elle-même après une action
  (confirmations, erreurs) restent en français quelle que soit la langue
  choisie dans l'interface — seule l'interface générée côté navigateur
  est traduite pour l'instant.

## 5. Sécurité des données

- La vraie collection Book Collector de l'utilisateur n'est jamais
  modifiée par accident : toute écriture (ajout, modification,
  suppression) est précédée d'une sauvegarde horodatée du fichier, et
  refusée d'emblée si Book Collector est lancé en même temps (pour éviter
  un conflit d'écriture).
- BookNojo peut ouvrir la vraie collection en **lecture** pour la consulter,
  mais aucune des actions d'écriture ci-dessus n'a d'exception possible
  pour elle depuis l'application — contrairement aux outils en ligne de
  commande, pensés eux pour un usage technique plus avancé (voir
  [`CLI.fr.md`](CLI.fr.md)).

## 6. Ce que l'application ne fait pas encore

- **Recherche en ligne** pour pré-remplir une fiche à partir d'un ISBN
  (OpenLibrary/Google Books/BnF) — repoussé, pas commencé.
- Pas encore signée/notariée par un compte développeur Apple — sans
  conséquence pour un usage personnel, mais à faire avant de distribuer
  l'application à quelqu'un d'autre.

## 7. Pour aller plus loin

- [`BUILD.fr.md`](BUILD.fr.md) : compiler l'application depuis les sources.
- [`CHANGELOG.fr.md`](CHANGELOG.fr.md) : historique des versions publiées sur
  GitHub.

Pour un usage quotidien, l'application seule suffit — vous n'avez pas
besoin de connaître les commandes en mode terminal. Pour les outils en
ligne de commande (`bc_export`/`bc_add`), réservés à un usage plus
technique (export CSV/JSON/HTML, scripts, transition depuis Book
Collector), voir [`CLI.fr.md`](CLI.fr.md).
