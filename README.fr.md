# BookNojo

> 🇬🇧 [Read in English](README.md)

Gestionnaire de collection de livres — importe la base de données d'une
collection [Book Collector](https://www.collectorz.com/book) (Collectorz)
dans sa propre base, et évolue progressivement vers une base de données
**indépendante**, pour continuer à fonctionner le jour où Book Collector ne
le pourra plus.

> **Statut : v1.0.0b1, bêta, usage personnel.** Projet en
> développement actif, pas encore distribué à qui que ce soit d'autre que
> son auteur.

## Pourquoi ce projet

Book Collector pour Mac est une application Intel qui dépend de Rosetta 2.
Apple a annoncé la fin programmée de Rosetta 2 (support complet jusqu'à
macOS 27, largement retiré à partir de macOS 28, ~automne 2027) — Book
Collector a donc une espérance de vie limitée sur un Mac à jour. BookNojo
amorce une base de données indépendante à partir d'une collection
existante, puis continue à l'alimenter avec ses propres outils, sans
dépendre de Book Collector pour grandir.

## Ce que ça fait aujourd'hui

- **Application macOS** (icône, fenêtre, menus natifs) pour consulter,
  ajouter, modifier et supprimer des livres dans une collection — avec
  confirmation avant toute écriture et sauvegarde automatique.
- Regroupement par auteur/genre/éditeur/format/langue/pays, recherche,
  statistiques, fiche détail complète (crédits, personnages, contenus,
  liens...).
- Interface bilingue français/anglais.
- Export en ligne de commande vers **CSV**, **JSON**, et une **page HTML**
  autonome reproduisant l'interface de l'application.

➡️ Guide complet d'utilisation de l'application : [`MANUAL.fr.md`](MANUAL.fr.md).
Des outils en ligne de commande existent aussi (export CSV/JSON/HTML,
scripts) — optionnels, pas nécessaires pour un usage quotidien : voir
[`CLI.fr.md`](CLI.fr.md).

## Installation

Télécharger le `.dmg` de la [dernière release](https://github.com/sergiodadi/BookNojo/releases/latest),
l'ouvrir, et glisser BookNojo.app dans Applications.

> ⚠️ **« BookNojo » est endommagé et ne peut pas être ouvert ?** BookNojo
> n'est pas signé avec un certificat Apple Developer, donc Gatekeeper
> (macOS) le met en quarantaine et affiche ce message trompeur — l'app
> n'est pas réellement endommagée. Pour corriger, lancer une fois dans le
> Terminal, puis ouvrir l'app normalement :
> ```bash
> xattr -cr /Applications/BookNojo.app
> ```
> Si ça ne suffit pas, clic droit (ou Ctrl+clic) sur l'app puis **Ouvrir**
> au lieu d'un double-clic.

Pour compiler depuis les sources plutôt que d'utiliser le `.dmg`, voir
[`BUILD.fr.md`](BUILD.fr.md).

> Testé uniquement sous **macOS Tahoe (26)** — non vérifié sur d'autres
> versions de macOS.

## Historique des versions

Voir [`CHANGELOG.fr.md`](CHANGELOG.fr.md) — une entrée par version publiée sur
GitHub.

## Licence

[MIT](LICENSE).

---

Ces paquets ont été construits, testés et publiés avec l'aide de Claude, l'assistant IA d'Anthropic.

Book Collector et Collectorz/CLZ sont des marques de leurs détenteurs respectifs. BookNojo est un projet indépendant, non affilié à ces sociétés.
