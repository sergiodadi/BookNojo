#!/usr/bin/env bash
# Resynchronise testdata/MyCollection.bkcp depuis la vraie collection
# Book Collector, et la repasse en lecture seule. À relancer quand la
# collection réelle a changé et qu'on veut tester sur des données à jour.
set -euo pipefail

SRC="/Users/serge/Documents/Collectorz/Book Collector/MyCollection.bkcp"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEST_DIR="$SCRIPT_DIR/testdata"
DEST="$DEST_DIR/MyCollection.bkcp"

if [ ! -d "$SRC" ]; then
  echo "Erreur: collection introuvable: $SRC" >&2
  exit 1
fi

mkdir -p "$DEST_DIR"

if [ -d "$DEST" ]; then
  echo "Suppression de l'ancienne copie de test..."
  chmod -R u+w "$DEST"
  rm -rf "$DEST"
fi

echo "Copie depuis: $SRC"
cp -R "$SRC" "$DEST"

echo "Passage en lecture seule..."
chmod 444 "$DEST/Book_data"
[ -f "$DEST/Book_data-shm" ] && chmod 444 "$DEST/Book_data-shm"
[ -d "$DEST/Images" ] && chmod -R a-w "$DEST/Images"

echo "OK: $DEST mis à jour ($(du -sh "$DEST" | cut -f1), lecture seule)."
