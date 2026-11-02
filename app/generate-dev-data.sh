#!/usr/bin/env bash
# Régénère app/src/index.html (gitignoré — données personnelles) depuis
# app/src/index.template.html (committé, sans données) et une copie de test
# (jamais la vraie collection). Nécessaire avant `cargo tauri dev`/`build`.
#
# Par défaut, utilise testdata-sample/ (~100 fiches, générées par
# ./build-sample-testdata.sh) — beaucoup plus rapide à régénérer et à
# inspecter que la collection de test complète. Passez --full pour utiliser
# testdata/ (2237 fiches) à la place, avant un commit par exemple, histoire
# de tester aussi sur un volume réaliste.
#
# Étape 2 du plan NBC : données statiques, pour prouver que l'interface
# s'affiche correctement dans la fenêtre Tauri. L'étape 3 les remplacera
# par de vrais appels IPC vers le moteur Rust (lib.rs/bc_add), plus de
# fichier statique à régénérer à ce moment-là.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
if [ "${1:-}" = "--full" ]; then
  TESTDATA="$ROOT_DIR/testdata/MyCollection.bkcp"
  REFRESH_HINT="./refresh-testdata.sh"
else
  TESTDATA="$ROOT_DIR/testdata-sample/MyCollection.bkcp"
  REFRESH_HINT="./build-sample-testdata.sh"
fi
BC_EXPORT="$ROOT_DIR/target/release/bc_export"
TEMPLATE="$SCRIPT_DIR/src/index.template.html"
OUT_HTML="$SCRIPT_DIR/src/index.html"
TMP_JSON="$(mktemp -t nbc_dev_data).json"
TMP_CSV="$(mktemp -t nbc_dev_data).csv"
TMP_SOURCE_HTML="$(mktemp -t nbc_dev_source).html"

cleanup() { rm -f "$TMP_JSON" "$TMP_CSV" "$TMP_SOURCE_HTML"; }
trap cleanup EXIT

if [ ! -d "$TESTDATA" ]; then
  echo "Erreur: $TESTDATA introuvable — lancez d'abord $REFRESH_HINT" >&2
  exit 1
fi
if [ ! -x "$BC_EXPORT" ]; then
  echo "Erreur: $BC_EXPORT introuvable — lancez d'abord cargo build --release" >&2
  exit 1
fi

cp "$ROOT_DIR/templates/app.js" "$SCRIPT_DIR/src/app.js"
cp "$ROOT_DIR/templates/style.css" "$SCRIPT_DIR/src/style.css"

echo "Export des données de test ($TESTDATA, jamais la vraie collection)..."
"$BC_EXPORT" --db "$TESTDATA" --csv "$TMP_CSV" --json "$TMP_JSON" --html "$TMP_SOURCE_HTML" >/dev/null

echo "Génération de $OUT_HTML..."
node -e "
const fs = require('fs');
const html = fs.readFileSync('$TMP_SOURCE_HTML', 'utf8');
const m = html.match(/<script id=\"data\" type=\"application\/json\">([\s\S]*?)<\/script>/);
if (!m) { console.error('bloc de données introuvable dans le HTML source'); process.exit(1); }
const tpl = fs.readFileSync('$TEMPLATE', 'utf8');
fs.writeFileSync('$OUT_HTML', tpl.replace('__DATA_JSON__', () => m[1]));
"

echo "OK: $OUT_HTML régénéré ($(du -sh "$OUT_HTML" | cut -f1))."
