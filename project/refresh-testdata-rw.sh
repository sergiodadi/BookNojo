#!/usr/bin/env bash
# Resynchronise testdata-rw/MyCollection.bkcp depuis la vraie collection
# Book Collector — copie INSCRIPTIBLE, dédiée aux tests d'écriture
# (bc_add commit, étape 4). Contrairement à testdata/ (refresh-testdata.sh),
# volontairement laissée en lecture-écriture.
#
# Ne touche jamais au fichier original : simple copie (cp -R), en lecture
# seule côté source.
set -euo pipefail

SRC="/Users/serge/Documents/Collectorz/Book Collector/MyCollection.bkcp"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEST_DIR="$SCRIPT_DIR/testdata-rw"
DEST="$DEST_DIR/MyCollection.bkcp"

if [ ! -d "$SRC" ]; then
  echo "Erreur: collection introuvable: $SRC" >&2
  exit 1
fi

mkdir -p "$DEST_DIR"

if [ -d "$DEST" ]; then
  echo "Suppression de l'ancienne copie inscriptible..."
  chmod -R u+w "$DEST"
  rm -rf "$DEST"
fi

echo "Copie depuis: $SRC"
cp -R "$SRC" "$DEST"
chmod -R u+w "$DEST"

echo "OK: $DEST mis à jour ($(du -sh "$DEST" | cut -f1), inscriptible)."
echo "Rappel : copie de test pour bc_add commit — jamais la vraie collection."
