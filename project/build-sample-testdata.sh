#!/usr/bin/env bash
# Construit testdata-sample/MyCollection.bkcp : une petite base d'échantillon
# (quelques livres par sous-collection + les livres complets de 1-2 auteurs
# choisis dans la sous-collection principale) à partir de testdata/ -- JAMAIS
# directement de la vraie collection. Beaucoup plus rapide à régénérer et à
# inspecter que la copie complète (2237 fiches -> une soixantaine).
#
# Relance ./refresh-testdata.sh avant celui-ci si tu veux un échantillon à
# jour par rapport à la vraie collection.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC="$SCRIPT_DIR/testdata/MyCollection.bkcp"
DEST_DIR="$SCRIPT_DIR/testdata-sample"
DEST="$DEST_DIR/MyCollection.bkcp"

if [ ! -d "$SRC" ]; then
  echo "Erreur: $SRC introuvable -- lancez d'abord ./refresh-testdata.sh" >&2
  exit 1
fi

if [ -d "$DEST" ]; then
  echo "Suppression de l'ancien échantillon..."
  chmod -R u+w "$DEST"
  rm -rf "$DEST"
fi
mkdir -p "$DEST_DIR"
mkdir -p "$DEST/Images"

python3 "$SCRIPT_DIR/build-sample-testdata.py" "$SRC" "$DEST"

echo "Passage en lecture seule..."
chmod 444 "$DEST/Book_data"
chmod -R a-w "$DEST/Images"

echo "OK: $DEST prêt ($(du -sh "$DEST" | cut -f1), lecture seule)."
