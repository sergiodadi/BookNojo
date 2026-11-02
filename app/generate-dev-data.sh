#!/usr/bin/env bash
# Régénère app/src/index.html (gitignoré — données personnelles) depuis
# app/src/index.template.html (committé, sans données) et testdata/ (jamais
# la vraie collection). Nécessaire avant `cargo tauri dev`/`build`, et à
# relancer après `./refresh-testdata.sh` ou toute modification du template
# ou de templates/app.js|style.css.
#
# Étape 2 du plan NBC : données statiques, pour prouver que l'interface
# s'affiche correctement dans la fenêtre Tauri. L'étape 3 les remplacera
# par de vrais appels IPC vers le moteur Rust (lib.rs/bc_add), plus de
# fichier statique à régénérer à ce moment-là.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
TESTDATA="$ROOT_DIR/testdata/MyCollection.bkcp"
BC_EXPORT="$ROOT_DIR/target/release/bc_export"
TEMPLATE="$SCRIPT_DIR/src/index.template.html"
OUT_HTML="$SCRIPT_DIR/src/index.html"
TMP_JSON="$(mktemp -t nbc_dev_data).json"
TMP_CSV="$(mktemp -t nbc_dev_data).csv"
TMP_SOURCE_HTML="$(mktemp -t nbc_dev_source).html"

cleanup() { rm -f "$TMP_JSON" "$TMP_CSV" "$TMP_SOURCE_HTML"; }
trap cleanup EXIT

if [ ! -d "$TESTDATA" ]; then
  echo "Erreur: $TESTDATA introuvable — lancez d'abord ./refresh-testdata.sh" >&2
  exit 1
fi
if [ ! -x "$BC_EXPORT" ]; then
  echo "Erreur: $BC_EXPORT introuvable — lancez d'abord cargo build --release" >&2
  exit 1
fi

cp "$ROOT_DIR/templates/app.js" "$SCRIPT_DIR/src/app.js"
cp "$ROOT_DIR/templates/style.css" "$SCRIPT_DIR/src/style.css"

echo "Export des données de test (testdata/, jamais la vraie collection)..."
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
