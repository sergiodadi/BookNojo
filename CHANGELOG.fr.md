# Changelog

> 🇬🇧 [Read in English](CHANGELOG.md)

Historique des **versions publiées sur GitHub** (`sergiodadi/BookNojo`),
une entrée par tag/release.

## [v1.0.0b6] - 2026-11-22

- Le formulaire d'ajout de livre a maintenant un bouton optionnel
  « Choisir une image… » : la couverture est posée juste après l'ajout du
  livre, au lieu de seulement après coup via la liste « sans couverture »
  du panneau Statistiques. Si l'ajout de la couverture échoue, le livre
  est quand même ajouté — rattrapable ensuite via la liste « sans
  couverture » ou `bc_add set-cover`.
- Un bouton « 📂 Ouvrir une collection… » dans l'en-tête, à côté
  d'« Ajouter un livre » — la même action que File > Import a
  collection… dans le menu natif, juste accessible sans l'ouvrir.

## [v1.0.0b5] - 2026-11-21

- README : ajout d'une section « Copie d'écran » (deux captures de l'app)
  juste après Installation.
- README : reformulation de l'intro pour commencer par « Pourquoi ce
  projet », fusionnée avec le paragraphe d'ouverture.
- Le champ version de `tauri.conf.json` suit désormais le tag de release
  publié au lieu d'être incrémenté à chaque commit de dev — ce qui faisait
  silencieusement afficher dans le nom du `.dmg` (et dans les champs de
  version du bundle `.app` lui-même) un numéro de version de développement
  interne au lieu de la vraie version publiée.

## [v1.0.0b4] - 2026-11-16

- `DISPLAY_VERSION` corrigé pour suivre le tag de release publié — le
  badge d'en-tête et les deux surfaces « About BookNojo » (boîte de
  dialogue custom et panneau natif macOS) affichaient auparavant la
  version de développement interne brute.
- Ajout de `set-cover` (la commande CLI `bc_add set-cover`, et un
  nouveau point d'entrée dans l'app : le compteur « sans couverture »
  du panneau Statistiques se déplie en liste cliquable de livres,
  chacun ouvrant un sélecteur de fichier natif).
- Ajout de `bc_add edit-plan`/`delete-plan` — modifier et supprimer un
  livre en ligne de commande, possible auparavant seulement depuis
  l'application.

## [v1.0.0b3] - 2026-11-08

- README.md/README.fr.md : suppression du numéro de version dans la
  ligne de statut — elle devenait obsolète à chaque déplacement du tag
  public. La ligne indique maintenant juste que le projet est en
  développement actif et public.

## [v1.0.0b2] - 2026-11-08

- Le dépôt est maintenant **public**.
- README : correction de la ligne de statut, qui disait encore « pas
  encore distribué à qui que ce soit d'autre que son auteur » — plus
  exact.
- Identité de l'auteur du commit corrigée pour utiliser l'adresse
  no-reply de GitHub plutôt qu'un email personnel.

## [v1.0.0b1] - 2026-11-04

Première version bêta publique de BookNojo, avec un `.dmg` téléchargeable
(`BookNojo_0.65.0_aarch64.dmg`) — plus besoin de compiler depuis les
sources pour l'installer.
