# Changelog

> 🇬🇧 [Read in English](CHANGELOG.md)

Historique des **versions publiées sur GitHub** (`sergiodadi/BookNojo`),
une entrée par tag/release.

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
