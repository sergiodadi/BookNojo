#!/usr/bin/env bash
# Synchronise app/src/{index.html,app.js,style.css} depuis templates/ et
# app/src/index.template.html — nécessaire avant `cargo tauri dev`/`build`,
# et à relancer après toute modification de templates/app.js|style.css ou
# du gabarit.
#
# Depuis l'étape 3 du plan BookNojo, l'app charge la collection EN DIRECT via la
# commande Tauri `load_collection` (voir app/src-tauri/src/lib.rs et
# templates/app.js § boot()) — il n'y a donc plus de données à embarquer
# dans index.html : le bloc `<script id="data">` reste présent (pour que
# bc_export, qui réutilise le même gabarit de page, continue à fonctionner
# à l'identique) mais vide, puisque l'app ne le lit jamais
# (`window.__TAURI__` existe, donc `loadAllBooks()` prend la branche
# `invoke()`, jamais ce bloc).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"
TEMPLATE="$SCRIPT_DIR/src/index.template.html"
OUT_HTML="$SCRIPT_DIR/src/index.html"

cp "$ROOT_DIR/templates/app.js" "$SCRIPT_DIR/src/app.js"
cp "$ROOT_DIR/templates/style.css" "$SCRIPT_DIR/src/style.css"
sed 's/__DATA_JSON__/[]/' "$TEMPLATE" > "$OUT_HTML"

echo "OK: app/src/{index.html,app.js,style.css} synchronisés."
