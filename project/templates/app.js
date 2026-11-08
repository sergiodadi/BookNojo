// In the BookNojo desktop app, there's no embedded `<script id="data">` block —
// the collection is loaded live through a Tauri command instead (see
// `boot()` at the bottom of this file). `window.__TAURI__` only exists
// inside that app (tauri.conf.json: withGlobalTauri), so it reliably tells
// apart that case from bc_export's static HTML export, which keeps working
// exactly as before (synchronous, embedded JSON, opened in any browser).
//
// Fallback when nothing better is known yet (first launch, nothing in
// localStorage): a small, entirely fictional sample collection (see
// sample-collection/, examples/generate_sample_collection.rs) — safe to
// ship as a default. This used to point at testdata-sample/, a ~100-book
// *extract of the developer's real collection*: harmless for local dev
// (never committed, never packaged) but wrong as the app's shipped
// default, since it meant a freshly installed BookNojo opened real personal
// data on first launch (found by the user after actually installing the
// built package, v0.49.0). "File > Import a collection..." picks a real
// one and overrides it from then on, remembered across relaunches the
// same way the rest of the UI state already is.
const DEV_COLLECTION_PATH = '/Users/serge/Developer/book_collector_export/project/sample-collection/MyCollection.bkcp';
let currentCollectionPath = DEV_COLLECTION_PATH;
try {
  const saved = localStorage.getItem('bc_collection_path');
  if (saved) currentCollectionPath = saved;
} catch (e) { /* ignore */ }

// UI language — a separate localStorage key from the rest of the UI state
// (bc_state), since it's a lower-level preference, not collection-specific
// navigation state. Everything user-visible goes through t() so toggling it
// re-renders the whole app consistently rather than leaving some panels
// translated and others not.
let currentLang = 'fr';
try {
  const savedLang = localStorage.getItem('bc_lang');
  if (savedLang === 'fr' || savedLang === 'en') currentLang = savedLang;
} catch (e) { /* ignore */ }

function t(key, vars) {
  let s = (I18N[currentLang] && I18N[currentLang][key]) ?? I18N.fr[key] ?? key;
  if (vars) for (const k of Object.keys(vars)) s = s.replaceAll(`{${k}}`, vars[k]);
  return s;
}

async function loadAllBooks() {
  if (window.__TAURI__) {
    return await window.__TAURI__.core.invoke('load_collection', { path: currentCollectionPath });
  }
  return JSON.parse(document.getElementById('data').textContent);
}

let allBooks, collCounts, collections;
const mainEl = document.querySelector('main');
const groupsEl = document.getElementById('groups');
const splitterVEl = document.getElementById('splitterV');
const rightColEl = document.getElementById('rightCol');
const splitterHEl = document.getElementById('splitterH');
const bookListPaneEl = document.getElementById('bookListPane');
const tabsEl = document.getElementById('tabs');
const groupFieldEl = document.getElementById('groupField');
const azEl = document.getElementById('azbar');
const groupListEl = document.getElementById('groupList');
const colHeaderEl = document.getElementById('colHeader');
const rowsEl = document.getElementById('rows');
const detailEl = document.getElementById('detail');
const countEl = document.getElementById('count');
const searchEl = document.getElementById('search');
const sortEl = document.getElementById('sort');
const viewListBtn = document.getElementById('viewList');
const viewGridBtn = document.getElementById('viewGrid');
const pageInfoEl = document.getElementById('pageinfo');
const prevBtn = document.getElementById('prev');
const nextBtn = document.getElementById('next');
const statsBtn = document.getElementById('statsBtn');
const statsPaneEl = document.getElementById('statsPane');
const addBtn = document.getElementById('addBtn');
const addPaneEl = document.getElementById('addPane');

const PAGE_SIZE = 60;
const collator = new Intl.Collator('fr', {sensitivity: 'base'});
const SORTERS = {
  title: (a, b) => collator.compare(a.title || '', b.title || ''),
  author: (a, b) => collator.compare((a.authors[0] || ''), (b.authors[0] || '')),
  year_desc: (a, b) => (parseInt(b.publication_year) || 0) - (parseInt(a.publication_year) || 0),
  publisher: (a, b) => collator.compare(a.publisher || '', b.publisher || ''),
};

// Each entry returns the list of group keys a book belongs to for that
// field (multi-valued for authors/genres, 0-or-1 for the plain lookups).
const GROUP_FIELDS = {
  authors: b => b.authors || [],
  genres: b => b.genres || [],
  publisher: b => b.publisher ? [b.publisher] : [],
  format: b => b.format ? [b.format] : [],
  language: b => b.language ? [b.language] : [],
  country: b => b.country ? [b.country] : [],
};

// `cover_url` is a raw `file://` URL (see HtmlBook::cover_url in
// src/main.rs) — fine when this same template is opened as a standalone
// file by bc_export's HTML export, but the BookNojo desktop app serves the page
// through Tauri's own protocol, not file://, so a WebView security policy
// blocks loading a raw file:// resource from it. `window.__TAURI__` only
// exists inside that app (tauri.conf.json: withGlobalTauri), so it's a
// reliable way to tell which context we're in and use Tauri's own asset
// protocol (convertFileSrc) only when actually needed.
function resolveCoverSrc(fileUrl) {
  if (!fileUrl) return null;
  if (window.__TAURI__) {
    const path = decodeURIComponent(fileUrl.replace(/^file:\/\//, ''));
    return window.__TAURI__.core.convertFileSrc(path);
  }
  return fileUrl;
}

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

// Every static (non-data-driven) user-facing string in this app, in both
// languages — see t()/setLang() near the top of the file. Rust-side
// messages returned from Tauri commands (success/error text from
// commit_add_book, delete_book, etc.) are a deliberate exception: they stay
// French-only for now, documented in CHANGELOG.md/MANUAL.md as a known,
// separate follow-up (would need a `lang` parameter threaded through every
// IPC call and bilingual message tables in the Rust crates).
const I18N = {
  fr: {
    nav_back_to_collection: '📚 Collection',
    nav_stats: '📊 Statistiques',
    nav_add: '➕ Ajouter un livre',

    group_all: 'Tous',
    group_sort_tooltip_active: 'Trié par nombre de livres — cliquer pour revenir au tri alphabétique',
    group_sort_tooltip_inactive: 'Cliquer pour trier par nombre de livres',
    azbar_all: 'Tout',

    opt_group_authors: 'Auteur', opt_group_genres: 'Genre', opt_group_publisher: 'Éditeur',
    opt_group_format: 'Format', opt_group_language: 'Langue', opt_group_country: 'Pays',
    opt_sort_title: 'Titre (A→Z)', opt_sort_author: 'Auteur (A→Z)',
    opt_sort_year_desc: 'Année (récent d\'abord)', opt_sort_publisher: 'Éditeur (A→Z)',
    list_search_placeholder: 'Rechercher titre ou auteur…',
    list_view_list: '☰ Liste', list_view_grid: '▦ Grille',
    col_title: 'Titre', col_author: 'Auteur',
    list_prev: '‹ Préc.', list_next: 'Suiv. ›',
    list_select_book: 'Sélectionnez un livre.',

    list_page_info: 'Page {page} / {total}',
    list_scope_books: '{count} livres',
    list_empty_view: 'Aucun livre dans cette vue.',
    list_book_not_found: 'Livre introuvable.',
    list_no_title: '(sans titre)',

    detail_current_value: 'Valeur actuelle : {v}',
    detail_purchase_price: 'Prix d\'achat : {v}',
    detail_group_product: 'Détails produit',
    detail_field_edition: 'Édition',
    detail_field_series: 'Série',
    detail_field_printed_by: 'Imprimé par',
    detail_field_paper_type: 'Type de papier',
    detail_field_dimensions: 'Dimensions',
    detail_field_dedication: 'Dédicace',
    detail_field_first_edition: 'Première édition',
    detail_yes: 'Oui',
    detail_field_printing: 'Tirage',
    detail_field_box_set: 'Coffret',
    detail_field_audio_length: 'Durée audio',
    detail_field_issue_number: 'N° de parution (Issue)',
    detail_group_original_edition: 'Édition originale',
    detail_field_original_title: 'Titre original',
    detail_field_original_subtitle: 'Sous-titre original',
    detail_field_original_publisher: 'Éditeur original',
    detail_field_original_country: 'Pays original',
    detail_field_original_language: 'Langue originale',
    detail_field_original_pub_date: 'Date de publication originale',
    detail_group_personal: 'Détails personnels',
    detail_field_owner: 'Propriétaire',
    detail_field_store: 'Magasin',
    detail_field_location_number: 'N° d\'emplacement',
    detail_field_cover_price: 'Prix de couverture',
    detail_field_purchase_date: 'Date d\'achat',
    detail_field_rare: 'Rare',
    detail_field_read_it: 'Lu',
    detail_field_read_times: 'Nombre de lectures',
    detail_field_read_date: 'Date de lecture',
    detail_field_custom1: 'Champ personnalisé 1',
    detail_field_custom2: 'Champ personnalisé 2',
    detail_field_date_added: 'Ajouté le',
    detail_group_credits: 'Crédits',
    detail_group_characters: 'Personnages',
    detail_group_contents: 'Contenus',
    detail_group_links: 'Liens',
    detail_alias: '(alias : {alias})',
    detail_starts_at_page: 'débute p. {n}',
    detail_group_genres: 'Genres',
    detail_group_subjects: 'Sujets',
    detail_group_plot: 'Résumé',
    detail_btn_edit: '✏️ Modifier ce livre',
    detail_btn_delete: '🗑️ Supprimer ce livre',
    detail_delete_confirm: 'Supprimer « {title} » ? Une sauvegarde est prise avant, mais cette action n\'est pas réversible depuis BookNojo.',
    detail_delete_failed: 'Échec de la suppression : {err}',
    detail_edit_load_failed: 'Impossible de charger ce livre pour modification : {err}',
    common_remove: 'Retirer',

    stats_title: 'Statistiques — {name}',
    stats_entries: '{n} fiches',
    stats_copies: '{n} exemplaires',
    stats_value_suffix: 'de valeur totale',
    stats_no_cover: '{n} sans couverture',
    stats_no_isbn: '{n} sans ISBN',
    stats_no_author: '{n} sans auteur',
    stats_formats: 'Formats',
    stats_languages: 'Langues',
    stats_publishers_top10: 'Éditeurs (top 10)',
    stats_by_decade: 'Par décennie de publication',
    stats_no_data: 'Aucune donnée.',

    add_heading_tauri: 'Ajouter un livre',
    add_heading_static: 'Ajouter un livre (préparation — étapes 1 à 5)',
    add_heading_edit: 'Modifier : {title}',
    add_hint_tauri: 'Ce formulaire écrit directement dans la collection ouverte, après confirmation. L\'onglet Couvertures n\'est pas repris ici (gestion de fichiers, hors périmètre).',
    add_hint_static: 'Ce formulaire ne modifie aucune donnée existante : il génère un fichier <code>new_book.json</code> à télécharger, à passer ensuite à <code>bc_add</code> pour vérifier ce qui existe déjà avant toute écriture réelle. L\'onglet Couvertures n\'est pas repris ici (gestion de fichiers, hors périmètre).',
    add_hint_edit: 'Les champs ci-dessous reflètent les valeurs actuelles de ce livre. Une confirmation sera demandée avant toute écriture.',
    add_submit_tauri: '💾 Enregistrer',
    add_submit_static: '⬇️ Générer new_book.json',
    add_submit_edit: '💾 Enregistrer les modifications',

    add_tab_main: 'Principal', add_tab_details: 'Détails', add_tab_personal: 'Personnel',
    add_tab_credits: 'Crédits & Personnages', add_tab_contents: 'Contenus',
    add_tab_plot: 'Intrigue & Notes', add_tab_links: 'Liens',

    add_field_title: 'Titre *', add_field_subtitle: 'Sous-titre',
    add_field_number_of_pages: 'Nombre de pages', add_field_year: 'Année',
    add_field_month: 'Mois', add_field_day: 'Jour', add_field_publisher: 'Éditeur',
    add_field_language: 'Langue', add_field_country: 'Pays', add_field_location: 'Emplacement',
    add_field_location_number: 'N° d\'emplacement', add_field_quantity: 'Quantité',
    add_field_collection: 'Sous-collection *',
    add_field_authors: 'Auteurs', add_placeholder_name_enter: 'Nom, puis Entrée',
    add_field_genres: 'Genres', add_placeholder_genre_enter: 'Genre, puis Entrée',
    add_field_subjects: 'Sujets', add_placeholder_subject_enter: 'Sujet, puis Entrée',

    add_field_issue_number: 'Issue N°', add_field_edition: 'Édition',
    add_field_printed_by: 'Imprimé par', add_field_series: 'Série', add_field_volume: 'Volume',
    add_field_paper_type: 'Type de papier',
    add_field_height: 'Hauteur', add_placeholder_height: 'ex. 21 cm',
    add_field_width: 'Largeur', add_placeholder_width: 'ex. 14 cm',
    add_field_printing: 'Tirage', add_field_audio_length: 'Durée audio (min)',
    add_field_dedication: 'Dédicace',
    add_check_first_edition: 'Première édition', add_check_abridged: 'Abrégé', add_check_box_set: 'Coffret',
    add_subhead_original_edition: 'Édition originale',
    add_field_original_title: 'Titre original', add_field_original_subtitle: 'Sous-titre original',
    add_field_original_publisher: 'Éditeur original', add_field_original_country: 'Pays original',
    add_field_original_language: 'Langue originale', add_field_original_year: 'Année orig.',
    add_field_original_month: 'Mois orig.', add_field_original_day: 'Jour orig.',

    add_field_condition: 'État', add_field_owner: 'Propriétaire', add_field_store: 'Magasin',
    add_field_current_value: 'Valeur actuelle', add_field_purchase_price: 'Prix d\'achat',
    add_field_cover_price: 'Prix de couverture', add_field_purchase_year: 'Année d\'achat',
    add_field_purchase_month: 'Mois d\'achat', add_field_purchase_day: 'Jour d\'achat',
    add_field_read_times: 'Nombre de lectures', add_field_read_year: 'Année de lecture',
    add_field_read_month: 'Mois de lecture', add_field_read_day: 'Jour de lecture',
    add_field_rating: 'Note (0–10)',
    add_check_rare: 'Rare', add_check_read_it: 'Lu',
    add_subhead_custom_fields: 'Champs personnalisés',
    add_field_custom_lookup1: 'Champ personnalisé 1 (liste)', add_field_custom_lookup2: 'Champ personnalisé 2 (liste)',
    add_field_custom_text1: 'Champ personnalisé 1 (texte)', add_field_custom_text2: 'Champ personnalisé 2 (texte)',

    add_field_credits_label: 'Crédits',
    add_placeholder_name: 'Nom',
    common_add: 'Ajouter',
    add_field_characters_label: 'Personnages',
    add_placeholder_character_name: 'Nom du personnage', add_placeholder_alias: 'Alias (optionnel)',

    add_contents_hint: 'Plusieurs histoires/nouvelles dans ce livre, chacune avec ses propres auteurs.',
    add_field_content_title: 'Titre', add_field_content_subtitle: 'Sous-titre',
    add_field_start_page: 'Page de début', add_field_content_authors: 'Auteurs de ce contenu',
    add_btn_add_content: '➕ Ajouter ce contenu',

    add_field_plot: 'Intrigue / résumé',
    add_placeholder_description: 'Description (optionnel)',

    add_confirm_title_heading: 'Confirmer l\'ajout',
    add_confirm_hint_new: '{n} nouvelle(s) valeur(s) seront créées (marquées "nouveau" ci-dessous) :',
    add_confirm_badge_new: 'nouveau',
    common_cancel: 'Annuler', common_save: 'Enregistrer', add_confirm_saving: 'Enregistrement…',

    add_err_title_required: 'Le titre est obligatoire.',
    add_err_collection_required: 'La sous-collection est obligatoire.',
    add_static_download_success: 'new_book.json téléchargé — passe-le à bc_add pour le vérifier.',

    boot_load_failed_heading: 'Impossible de charger la collection',
  },
  en: {
    nav_back_to_collection: '📚 Collection',
    nav_stats: '📊 Statistics',
    nav_add: '➕ Add a book',

    group_all: 'All',
    group_sort_tooltip_active: 'Sorted by book count — click to go back to alphabetical order',
    group_sort_tooltip_inactive: 'Click to sort by book count',
    azbar_all: 'All',

    opt_group_authors: 'Author', opt_group_genres: 'Genre', opt_group_publisher: 'Publisher',
    opt_group_format: 'Format', opt_group_language: 'Language', opt_group_country: 'Country',
    opt_sort_title: 'Title (A→Z)', opt_sort_author: 'Author (A→Z)',
    opt_sort_year_desc: 'Year (newest first)', opt_sort_publisher: 'Publisher (A→Z)',
    list_search_placeholder: 'Search title or author…',
    list_view_list: '☰ List', list_view_grid: '▦ Grid',
    col_title: 'Title', col_author: 'Author',
    list_prev: '‹ Prev.', list_next: 'Next ›',
    list_select_book: 'Select a book.',

    list_page_info: 'Page {page} of {total}',
    list_scope_books: '{count} books',
    list_empty_view: 'No books in this view.',
    list_book_not_found: 'Book not found.',
    list_no_title: '(untitled)',

    detail_current_value: 'Current value: {v}',
    detail_purchase_price: 'Purchase price: {v}',
    detail_group_product: 'Product details',
    detail_field_edition: 'Edition',
    detail_field_series: 'Series',
    detail_field_printed_by: 'Printed by',
    detail_field_paper_type: 'Paper type',
    detail_field_dimensions: 'Dimensions',
    detail_field_dedication: 'Dedication',
    detail_field_first_edition: 'First edition',
    detail_yes: 'Yes',
    detail_field_printing: 'Printing',
    detail_field_box_set: 'Box set',
    detail_field_audio_length: 'Audio length',
    detail_field_issue_number: 'Issue number',
    detail_group_original_edition: 'Original edition',
    detail_field_original_title: 'Original title',
    detail_field_original_subtitle: 'Original subtitle',
    detail_field_original_publisher: 'Original publisher',
    detail_field_original_country: 'Original country',
    detail_field_original_language: 'Original language',
    detail_field_original_pub_date: 'Original publication date',
    detail_group_personal: 'Personal details',
    detail_field_owner: 'Owner',
    detail_field_store: 'Store',
    detail_field_location_number: 'Location number',
    detail_field_cover_price: 'Cover price',
    detail_field_purchase_date: 'Purchase date',
    detail_field_rare: 'Rare',
    detail_field_read_it: 'Read',
    detail_field_read_times: 'Times read',
    detail_field_read_date: 'Date read',
    detail_field_custom1: 'Custom field 1',
    detail_field_custom2: 'Custom field 2',
    detail_field_date_added: 'Added on',
    detail_group_credits: 'Credits',
    detail_group_characters: 'Characters',
    detail_group_contents: 'Contents',
    detail_group_links: 'Links',
    detail_alias: '(alias: {alias})',
    detail_starts_at_page: 'starts at p. {n}',
    detail_group_genres: 'Genres',
    detail_group_subjects: 'Subjects',
    detail_group_plot: 'Summary',
    detail_btn_edit: '✏️ Edit this book',
    detail_btn_delete: '🗑️ Delete this book',
    detail_delete_confirm: 'Delete "{title}"? A backup is taken first, but this action cannot be undone from within BookNojo.',
    detail_delete_failed: 'Delete failed: {err}',
    detail_edit_load_failed: 'Could not load this book for editing: {err}',
    common_remove: 'Remove',

    stats_title: 'Statistics — {name}',
    stats_entries: '{n} entries',
    stats_copies: '{n} copies',
    stats_value_suffix: 'total value',
    stats_no_cover: '{n} with no cover',
    stats_no_isbn: '{n} with no ISBN',
    stats_no_author: '{n} with no author',
    stats_formats: 'Formats',
    stats_languages: 'Languages',
    stats_publishers_top10: 'Publishers (top 10)',
    stats_by_decade: 'By publication decade',
    stats_no_data: 'No data.',

    add_heading_tauri: 'Add a book',
    add_heading_static: 'Add a book (preview — steps 1 to 5)',
    add_heading_edit: 'Edit: {title}',
    add_hint_tauri: 'This form writes directly into the open collection, once confirmed. The Covers tab isn\'t included here (file management, out of scope).',
    add_hint_static: 'This form never modifies existing data: it generates a <code>new_book.json</code> file to download, to pass on to <code>bc_add</code> afterwards to check what already exists before any real write. The Covers tab isn\'t included here (file management, out of scope).',
    add_hint_edit: 'The fields below reflect this book\'s current values. A confirmation will be asked before anything is written.',
    add_submit_tauri: '💾 Save',
    add_submit_static: '⬇️ Generate new_book.json',
    add_submit_edit: '💾 Save changes',

    add_tab_main: 'Main', add_tab_details: 'Details', add_tab_personal: 'Personal',
    add_tab_credits: 'Credits & Characters', add_tab_contents: 'Contents',
    add_tab_plot: 'Plot & Notes', add_tab_links: 'Links',

    add_field_title: 'Title *', add_field_subtitle: 'Subtitle',
    add_field_number_of_pages: 'Number of pages', add_field_year: 'Year',
    add_field_month: 'Month', add_field_day: 'Day', add_field_publisher: 'Publisher',
    add_field_language: 'Language', add_field_country: 'Country', add_field_location: 'Location',
    add_field_location_number: 'Location number', add_field_quantity: 'Quantity',
    add_field_collection: 'Sub-collection *',
    add_field_authors: 'Authors', add_placeholder_name_enter: 'Name, then Enter',
    add_field_genres: 'Genres', add_placeholder_genre_enter: 'Genre, then Enter',
    add_field_subjects: 'Subjects', add_placeholder_subject_enter: 'Subject, then Enter',

    add_field_issue_number: 'Issue number', add_field_edition: 'Edition',
    add_field_printed_by: 'Printed by', add_field_series: 'Series', add_field_volume: 'Volume',
    add_field_paper_type: 'Paper type',
    add_field_height: 'Height', add_placeholder_height: 'e.g. 21 cm',
    add_field_width: 'Width', add_placeholder_width: 'e.g. 14 cm',
    add_field_printing: 'Printing', add_field_audio_length: 'Audio length (min)',
    add_field_dedication: 'Dedication',
    add_check_first_edition: 'First edition', add_check_abridged: 'Abridged', add_check_box_set: 'Box set',
    add_subhead_original_edition: 'Original edition',
    add_field_original_title: 'Original title', add_field_original_subtitle: 'Original subtitle',
    add_field_original_publisher: 'Original publisher', add_field_original_country: 'Original country',
    add_field_original_language: 'Original language', add_field_original_year: 'Orig. year',
    add_field_original_month: 'Orig. month', add_field_original_day: 'Orig. day',

    add_field_condition: 'Condition', add_field_owner: 'Owner', add_field_store: 'Store',
    add_field_current_value: 'Current value', add_field_purchase_price: 'Purchase price',
    add_field_cover_price: 'Cover price', add_field_purchase_year: 'Purchase year',
    add_field_purchase_month: 'Purchase month', add_field_purchase_day: 'Purchase day',
    add_field_read_times: 'Times read', add_field_read_year: 'Read year',
    add_field_read_month: 'Read month', add_field_read_day: 'Read day',
    add_field_rating: 'Rating (0–10)',
    add_check_rare: 'Rare', add_check_read_it: 'Read',
    add_subhead_custom_fields: 'Custom fields',
    add_field_custom_lookup1: 'Custom field 1 (lookup)', add_field_custom_lookup2: 'Custom field 2 (lookup)',
    add_field_custom_text1: 'Custom field 1 (text)', add_field_custom_text2: 'Custom field 2 (text)',

    add_field_credits_label: 'Credits',
    add_placeholder_name: 'Name',
    common_add: 'Add',
    add_field_characters_label: 'Characters',
    add_placeholder_character_name: 'Character name', add_placeholder_alias: 'Alias (optional)',

    add_contents_hint: 'Several stories in this book, each with its own authors.',
    add_field_content_title: 'Title', add_field_content_subtitle: 'Subtitle',
    add_field_start_page: 'Start page', add_field_content_authors: 'Authors of this content',
    add_btn_add_content: '➕ Add this content',

    add_field_plot: 'Plot / summary',
    add_placeholder_description: 'Description (optional)',

    add_confirm_title_heading: 'Confirm the addition',
    add_confirm_hint_new: '{n} new value(s) will be created (marked "new" below):',
    add_confirm_badge_new: 'new',
    common_cancel: 'Cancel', common_save: 'Save', add_confirm_saving: 'Saving…',

    add_err_title_required: 'Title is required.',
    add_err_collection_required: 'Sub-collection is required.',
    add_static_download_success: 'new_book.json downloaded — pass it to bc_add to check it.',

    boot_load_failed_heading: 'Unable to load the collection',
  },
};

// Flat key/value section for the detail card ("Product Details"/"Personal
// Details" etc., matching Book Collector's own section names) — falsy
// values (0, '', null) are dropped, same convention `current_value`/
// `purchase_price` already use just above in selectBook().
function kvGroup(label, pairs) {
  const rows = pairs
    .filter(([, v]) => v)
    .map(([k, v]) => `<div class="kv-row"><span class="kv-key">${escapeHtml(k)}</span><span class="kv-val">${escapeHtml(String(v))}</span></div>`)
    .join('');
  return rows ? `<div class="group"><div class="label">${escapeHtml(label)}</div><div class="kv-list">${rows}</div></div>` : '';
}

// Repeatable-entry section (Crédits, Personnages, Contenus, Liens) — each
// entry is a pre-built HTML string from the caller, since the fields
// differ per type.
function repGroup(label, itemsHtml) {
  return itemsHtml.length ? `<div class="group"><div class="label">${escapeHtml(label)}</div><div class="rep-list-view">${itemsHtml.join('')}</div></div>` : '';
}

const state = {
  collection: null, // set once allBooks has loaded — see boot()
  groupField: 'authors',
  groupValue: null, // null = "Tous"
  azLetter: null,
  groupSortByCount: false, // false = alphabétique, true = par nombre de livres (décroissant) — bascule en cliquant sur le nombre de la ligne "Tous"
  query: '',
  sort: 'title',
  mode: 'list',
  page: 1,
  items: [],
  activeId: null,
  panel: 'library', // 'library' | 'stats' | 'add'
};

// Multi-value chip/repeatable fields for the "add a book" form live outside
// `state` so they survive switching to another panel and back without being
// reset.
const addFormState = {
  authors: [], genres: [], subjects: [], credits: [],
  characters: [], // { name, alias }
  contents: [],   // { title, subtitle, number_of_pages, authors: [] }
  links: [],      // { url, description }
};

function resetAddFormState() {
  for (const key of Object.keys(addFormState)) addFormState[key].length = 0;
}

// Non-null while the Add-book panel is showing an existing book for
// modification instead of a blank form — set by editBook(), cleared on
// submit success or when the top "Ajouter un livre"/"Collection" button is
// clicked. editingBookData is what load_book_for_edit returned (current
// display-name values, same shape renderAddForm's inputs read).
let editingBookId = null;
let editingBookData = null;

function renderTabs() {
  tabsEl.innerHTML = '';
  for (const name of collections) {
    const tab = document.createElement('div');
    tab.className = 'tab' + (name === state.collection ? ' active' : '');
    const label = document.createElement('span');
    label.textContent = name;
    const n = document.createElement('span');
    n.className = 'n';
    n.textContent = collCounts.get(name);
    tab.appendChild(label);
    tab.appendChild(n);
    tab.addEventListener('click', () => switchCollection(name));
    tabsEl.appendChild(tab);
  }
}

function switchCollection(name) {
  if (name === state.collection) return;
  state.collection = name;
  state.groupValue = null;
  state.azLetter = null;
  state.query = '';
  searchEl.value = '';
  state.page = 1;
  renderTabs();
  refreshGroupPanel();
  recompute();
  if (state.panel === 'stats') renderStats();
}

function letterOf(value) {
  const ch = (value || '').trim().charAt(0).toUpperCase();
  return /[A-Z]/.test(ch) ? ch : '#';
}

function computeGroupCounts() {
  const getter = GROUP_FIELDS[state.groupField];
  const counts = new Map();
  for (const b of allBooks) {
    if (b.collection !== state.collection) continue;
    for (const v of getter(b)) {
      if (!v) continue;
      if (state.azLetter && letterOf(v) !== state.azLetter) continue;
      counts.set(v, (counts.get(v) || 0) + 1);
    }
  }
  const entries = [...counts.entries()];
  return state.groupSortByCount
    ? entries.sort((a, b) => b[1] - a[1] || collator.compare(a[0], b[0]))
    : entries.sort((a, b) => collator.compare(a[0], b[0]));
}

function computeAvailableLetters() {
  const getter = GROUP_FIELDS[state.groupField];
  const set = new Set();
  for (const b of allBooks) {
    if (b.collection !== state.collection) continue;
    for (const v of getter(b)) { if (v) set.add(letterOf(v)); }
  }
  return [...set].sort();
}

function renderAzBar() {
  azEl.innerHTML = '';
  const tout = document.createElement('button');
  tout.textContent = t('azbar_all');
  tout.className = state.azLetter === null ? 'active' : '';
  tout.addEventListener('click', () => { state.azLetter = null; refreshGroupPanel(); });
  azEl.appendChild(tout);
  for (const letter of computeAvailableLetters()) {
    const btn = document.createElement('button');
    btn.textContent = letter;
    btn.className = state.azLetter === letter ? 'active' : '';
    btn.addEventListener('click', () => { state.azLetter = letter; refreshGroupPanel(); });
    azEl.appendChild(btn);
  }
}

function renderGroupList() {
  groupListEl.innerHTML = '';
  const totalForCollection = collCounts.get(state.collection);
  groupListEl.appendChild(
    makeGroupRow(t('group_all'), totalForCollection, state.groupValue === null, () => selectGroup(null), true));
  for (const [key, count] of computeGroupCounts()) {
    groupListEl.appendChild(makeGroupRow(key, count, state.groupValue === key, () => selectGroup(key)));
  }
}

function makeGroupRow(label, count, active, onClick, countIsSortToggle) {
  const row = document.createElement('div');
  row.className = 'g' + (active ? ' active' : '');
  const l = document.createElement('span');
  l.textContent = label;
  const n = document.createElement('span');
  n.className = 'n';
  n.textContent = count;
  row.appendChild(l);
  row.appendChild(n);
  row.addEventListener('click', onClick);
  if (countIsSortToggle) {
    n.classList.add('sortable');
    n.title = state.groupSortByCount
      ? t('group_sort_tooltip_active')
      : t('group_sort_tooltip_inactive');
    if (state.groupSortByCount) n.classList.add('sort-active');
    n.addEventListener('click', (e) => {
      e.stopPropagation();
      state.groupSortByCount = !state.groupSortByCount;
      renderGroupList();
      saveState();
    });
  }
  return row;
}

// Rebuilds both the A–Z bar and the group list — call when the collection,
// grouping field, or active letter changes.
function refreshGroupPanel() {
  renderAzBar();
  renderGroupList();
}

function selectGroup(value) {
  state.groupValue = value;
  state.page = 1;
  renderGroupList();
  recompute();
}

// Lets a genre pill in the detail card double as a shortcut into the
// left-hand grouping panel.
function filterByGenre(g) {
  state.groupField = 'genres';
  groupFieldEl.value = 'genres';
  state.groupValue = g;
  state.azLetter = null;
  state.page = 1;
  refreshGroupPanel();
  recompute();
}

function recompute() {
  let items = allBooks.filter(b => b.collection === state.collection);
  if (state.groupValue !== null) {
    const getter = GROUP_FIELDS[state.groupField];
    items = items.filter(b => getter(b).includes(state.groupValue));
  }
  const q = state.query;
  if (q) {
    items = items.filter(b =>
      (b.title || '').toLowerCase().includes(q) ||
      (b.authors || []).join(' ').toLowerCase().includes(q));
  }
  items = items.slice().sort(SORTERS[state.sort]);
  state.items = items;
  const totalPages = Math.max(1, Math.ceil(items.length / PAGE_SIZE));
  state.page = Math.min(Math.max(1, state.page), totalPages);
  renderList();
}

function renderList() {
  const start = (state.page - 1) * PAGE_SIZE;
  const pageItems = state.items.slice(start, start + PAGE_SIZE);

  rowsEl.innerHTML = '';
  for (const b of pageItems) {
    const row = document.createElement('div');
    row.className = 'row' + (b.id === state.activeId ? ' active' : '');

    const rowThumb = b.thumb_url || resolveCoverSrc(b.cover_url);
    if (rowThumb) {
      const img = document.createElement('img');
      img.className = 'thumb';
      img.loading = 'lazy';
      img.src = rowThumb;
      img.alt = '';
      row.appendChild(img);
    } else {
      const ph = document.createElement('div');
      ph.className = 'thumb';
      row.appendChild(ph);
    }

    const txt = document.createElement('div');
    txt.className = 'txt';
    const titleEl = document.createElement('div');
    titleEl.className = 't';
    titleEl.textContent = b.title || t('list_no_title');
    const a = document.createElement('div');
    a.className = 'a';
    a.textContent = b.authors.join(', ');
    txt.appendChild(titleEl);
    txt.appendChild(a);
    row.appendChild(txt);

    row.addEventListener('click', () => selectBook(b.id));
    rowsEl.appendChild(row);
  }

  const totalPages = Math.max(1, Math.ceil(state.items.length / PAGE_SIZE));
  pageInfoEl.textContent = t('list_page_info', { page: state.page, total: totalPages });
  prevBtn.disabled = state.page <= 1;
  nextBtn.disabled = state.page >= totalPages;
  const scope = state.groupValue !== null
    ? `"${state.groupValue}"`
    : t('list_scope_books', { count: collCounts.get(state.collection) });
  countEl.textContent = `${state.items.length} / ${scope}`;

  // Show the already-selected book if it's on this page (incl. right after
  // a restore from a previous session), otherwise fall back to the first
  // item. selectBook() always (re)renders the detail card — cheap, and it
  // keeps a restored activeId from being silently skipped.
  const target = pageItems.find(b => b.id === state.activeId) || pageItems[0] || null;
  if (target) {
    selectBook(target.id);
  } else {
    state.activeId = null;
    detailEl.innerHTML = `<div id="empty">${t('list_empty_view')}</div>`;
    saveState();
  }
}

function stars(rating) {
  const r = Math.max(0, Math.min(10, rating || 0));
  let out = '';
  for (let i = 0; i < 10; i++) out += `<span class="${i < r ? '' : 'off'}">★</span>`;
  return out;
}

function selectBook(id, scrollIntoView) {
  state.activeId = id;
  for (const row of rowsEl.children) row.classList.remove('active');
  const idx = state.items.slice((state.page - 1) * PAGE_SIZE, state.page * PAGE_SIZE)
    .findIndex(b => b.id === id);
  if (idx >= 0) {
    const row = rowsEl.children[idx];
    row.classList.add('active');
    if (scrollIntoView) row.scrollIntoView({ block: 'nearest' });
  }

  const b = allBooks.find(x => x.id === id);
  if (!b) { detailEl.innerHTML = `<div id="empty">${t('list_book_not_found')}</div>`; return; }

  const coverSrc = resolveCoverSrc(b.cover_url);
  const cover = coverSrc
    ? `<img class="cover" src="${coverSrc}" alt="">`
    : `<div class="cover"></div>`;

  const props = [b.format, b.country, b.language].filter(Boolean)
    .map(x => `<span>${escapeHtml(x)}</span>`).join('');

  const props2 = [
    b.location ? escapeHtml(b.location) : '',
    b.condition ? escapeHtml(b.condition) : '',
    b.number_of_pages ? `${b.number_of_pages} pages` : '',
  ].filter(Boolean).map(x => `<span>${x}</span>`).join('');

  const moneyBits = [];
  if (b.current_value) moneyBits.push(`<span>${escapeHtml(t('detail_current_value', { v: b.current_value }))}</span>`);
  if (b.purchase_price) moneyBits.push(`<span>${escapeHtml(t('detail_purchase_price', { v: b.purchase_price }))}</span>`);

  const isGenreActive = g => state.groupField === 'genres' && state.groupValue === g;
  const genreTags = (b.genres || []).map(g =>
    `<span class="tag genre${isGenreActive(g) ? ' selected' : ''}" data-genre="${escapeHtml(g)}">${escapeHtml(g)}</span>`).join('');
  const subjectTags = (b.subjects || []).map(s => `<span class="tag subject">${escapeHtml(s)}</span>`).join('');
  const userTags = (b.tags || []).map(tag => `<span class="tag usertag">${escapeHtml(tag)}</span>`).join('');

  // "Product Details" / "Personal Details" — section names match Book
  // Collector's own detail view, which these were missing entirely against.
  const dimensions = [b.height, b.width].filter(Boolean).join(' × ');
  const yesLabel = t('detail_yes');
  const productDetails = kvGroup(t('detail_group_product'), [
    [t('detail_field_edition'), b.edition],
    [t('detail_field_series'), b.series ? (b.volume ? `${b.series}, vol. ${b.volume}` : b.series) : ''],
    [t('detail_field_printed_by'), b.printed_by],
    [t('detail_field_paper_type'), b.paper_type],
    [t('detail_field_dimensions'), dimensions],
    ['Dewey', b.dewey],
    ['LCCN', b.lccn],
    [t('detail_field_dedication'), b.dedication],
    [t('detail_field_first_edition'), b.first_edition ? yesLabel : ''],
    [t('detail_field_printing'), b.printing],
    [t('detail_field_box_set'), b.box_set ? yesLabel : ''],
    [t('detail_field_audio_length'), b.audio_length_minutes ? `${b.audio_length_minutes} min` : ''],
    [t('detail_field_issue_number'), b.issue_number],
  ]);
  const originalPubDate = [b.original_publication_day, b.original_publication_month, b.original_publication_year]
    .filter(Boolean).join('/');
  const originalEdition = kvGroup(t('detail_group_original_edition'), [
    [t('detail_field_original_title'), b.original_title],
    [t('detail_field_original_subtitle'), b.original_subtitle],
    [t('detail_field_original_publisher'), b.original_publisher],
    [t('detail_field_original_country'), b.original_country],
    [t('detail_field_original_language'), b.original_language],
    [t('detail_field_original_pub_date'), originalPubDate],
  ]);
  const purchaseDate = [b.purchase_day, b.purchase_month, b.purchase_year].filter(Boolean).join('/');
  const readDate = [b.read_day, b.read_month, b.read_year].filter(Boolean).join('/');
  const personalDetails = kvGroup(t('detail_group_personal'), [
    [t('detail_field_owner'), b.owner],
    [t('detail_field_store'), b.store],
    [t('detail_field_location_number'), b.location_number],
    [t('detail_field_cover_price'), b.cover_price],
    [t('detail_field_purchase_date'), purchaseDate],
    [t('detail_field_rare'), b.rare ? yesLabel : ''],
    [t('detail_field_read_it'), b.read_it ? yesLabel : ''],
    [t('detail_field_read_times'), b.read_times],
    [t('detail_field_read_date'), readDate],
    [t('detail_field_custom1'), b.user_lookup1 || b.user_text1],
    [t('detail_field_custom2'), b.user_lookup2 || b.user_text2],
    [t('detail_field_date_added'), b.date_added ? b.date_added.slice(0, 10) : ''],
  ]);

  const creditsSection = repGroup(t('detail_group_credits'), (b.credits || []).map(c =>
    `<div class="rep-item">${escapeHtml(c.name)}</div>`));
  const charactersSection = repGroup(t('detail_group_characters'), (b.characters || []).map(c =>
    `<div class="rep-item">${escapeHtml(c.name)}${c.alias ? ` <span class="rep-sub">${escapeHtml(t('detail_alias', { alias: c.alias }))}</span>` : ''}</div>`));
  const contentsSection = repGroup(t('detail_group_contents'), (b.contents || []).map(c => {
    const bits = [];
    if (c.number_of_pages) bits.push(`${c.number_of_pages} p.`);
    if (c.start_page) bits.push(t('detail_starts_at_page', { n: c.start_page }));
    if (c.authors && c.authors.length) bits.push(c.authors.join(', '));
    const sub = c.subtitle ? ` — ${escapeHtml(c.subtitle)}` : '';
    return `<div class="rep-item">${escapeHtml(c.title)}${sub}${bits.length ? ` <span class="rep-sub">(${escapeHtml(bits.join(' · '))})</span>` : ''}</div>`;
  }));
  const linksSection = repGroup(t('detail_group_links'), (b.links || []).map(l =>
    `<div class="rep-item"><a href="${escapeHtml(l.url)}" target="_blank" rel="noopener noreferrer">${escapeHtml(l.description || l.url)}</a></div>`));

  detailEl.innerHTML = `
    <div class="card">
      ${cover}
      <div class="meta">
        <div class="author">${(b.authors || []).join(', ')}</div>
        <div class="title"></div>
        ${b.subtitle ? '<div class="subtitle"></div>' : ''}
        <div class="pub"></div>
        ${b.isbn ? `<div class="isbn">${escapeHtml(b.isbn)}</div>` : ''}
        <div class="props">${props}</div>
        ${props2 ? `<div class="props">${props2}</div>` : ''}
        ${moneyBits.length ? `<div class="value-line">${moneyBits.join('')}</div>` : ''}
        ${b.my_rating ? `<div class="stars">${stars(b.my_rating)}</div>` : ''}
        ${genreTags ? `<div class="group"><div class="label">${escapeHtml(t('detail_group_genres'))}</div><div class="tags">${genreTags}</div></div>` : ''}
        ${subjectTags ? `<div class="group"><div class="label">${escapeHtml(t('detail_group_subjects'))}</div><div class="tags">${subjectTags}</div></div>` : ''}
        ${userTags ? `<div class="group"><div class="label">Tags</div><div class="tags">${userTags}</div></div>` : ''}
        ${productDetails}
        ${originalEdition}
        ${personalDetails}
        ${creditsSection}
        ${charactersSection}
        ${contentsSection}
        ${linksSection}
        ${b.plot ? `<div class="group"><div class="label">${escapeHtml(t('detail_group_plot'))}</div><div class="text-block plot"></div></div>` : ''}
        ${b.notes ? '<div class="group"><div class="label">Notes</div><div class="text-block notes"></div></div>' : ''}
        ${window.__TAURI__ ? `<div class="detail-actions"><button type="button" id="editBookBtn" class="secondary">${t('detail_btn_edit')}</button><button type="button" id="deleteBookBtn" class="danger">${t('detail_btn_delete')}</button></div>` : ''}
      </div>
    </div>`;

  // Free-text fields set via textContent (never innerHTML) to rule out any HTML injection.
  detailEl.querySelector('.title').textContent = b.title || t('list_no_title');
  if (b.subtitle) detailEl.querySelector('.subtitle').textContent = b.subtitle;
  const pubBits = [b.publisher, b.publication_year].filter(Boolean);
  detailEl.querySelector('.pub').textContent = pubBits.join(' — ');
  if (b.plot) detailEl.querySelector('.plot').textContent = b.plot;
  if (b.notes) detailEl.querySelector('.notes').textContent = b.notes;

  detailEl.querySelectorAll('.tag.genre').forEach(el => {
    el.addEventListener('click', () => filterByGenre(el.dataset.genre));
  });

  const deleteBtn = document.getElementById('deleteBookBtn');
  if (deleteBtn) {
    deleteBtn.addEventListener('click', () => deleteBookWithConfirm(b));
  }
  const editBtn = document.getElementById('editBookBtn');
  if (editBtn) {
    editBtn.addEventListener('click', () => editBook(b));
  }

  saveState();
}

// Tauri-only: fetch this book's current values (resolved display names,
// not raw foreign keys — see load_book_for_edit in lib.rs) and switch to
// the Add-book panel pre-filled with them, in "edit" mode.
async function editBook(b) {
  try {
    const data = await window.__TAURI__.core.invoke('load_book_for_edit', { path: currentCollectionPath, bookId: b.id });
    editingBookId = b.id;
    editingBookData = data;
    resetAddFormState();
    addFormState.authors.push(...data.authors);
    addFormState.genres.push(...data.genres);
    addFormState.subjects.push(...data.subjects);
    addFormState.credits.push(...data.credits.map(c => ({ ...c })));
    addFormState.characters.push(...data.characters.map(c => ({ ...c })));
    addFormState.contents.push(...data.contents.map(c => ({ ...c, authors: [...c.authors] })));
    addFormState.links.push(...data.links.map(l => ({ ...l })));
    setPanel('add');
  } catch (err) {
    window.alert(t('detail_edit_load_failed', { err }));
  }
}

// Tauri-only: delete the given book after a native confirm(), then reload
// the whole collection (simplest correct way to reflect the removal —
// matches the Add-book flow, which doesn't try to surgically patch
// allBooks/collCounts either). Never reachable from bc_export's static
// export (no window.__TAURI__, no button rendered in the first place).
async function deleteBookWithConfirm(b) {
  const label = b.title || t('list_no_title');
  try {
    // Tauri's dialog plugin replaces window.confirm with an async version
    // (resolves to a Promise, not a boolean) — a bare `if (!window.confirm(...))`
    // always sees a truthy Promise and never returns early, so the delete
    // used to go ahead no matter what the user clicked (found in v0.45.0).
    // Also inside this try now, not before it: a missing "dialog:default"
    // capability makes the underlying invoke reject outright, which used to
    // fail silently with no visible error at all (found right after, in
    // v0.45.1, testing the v0.45.0 fix) — any failure here should be as
    // visible as a failed delete_book call, not swallowed.
    if (!(await window.confirm(t('detail_delete_confirm', { title: label })))) {
      return;
    }
    await window.__TAURI__.core.invoke('delete_book', { path: currentCollectionPath, bookId: b.id });
    bootWithErrorHandling();
  } catch (err) {
    window.alert(t('detail_delete_failed', { err }));
  }
}

groupFieldEl.addEventListener('change', () => {
  state.groupField = groupFieldEl.value;
  state.groupValue = null;
  state.azLetter = null;
  state.page = 1;
  refreshGroupPanel();
  recompute();
});

searchEl.addEventListener('input', () => {
  state.query = searchEl.value.trim().toLowerCase();
  state.page = 1;
  recompute();
});

sortEl.addEventListener('change', () => {
  state.sort = sortEl.value;
  state.page = 1;
  recompute();
  updateColHeaderActive();
});

function updateColHeaderActive() {
  colHeaderEl.querySelectorAll('[data-sort]').forEach(el => {
    el.classList.toggle('active', el.dataset.sort === state.sort);
  });
}
colHeaderEl.querySelectorAll('[data-sort]').forEach(el => {
  el.addEventListener('click', () => {
    state.sort = el.dataset.sort;
    sortEl.value = state.sort;
    state.page = 1;
    recompute();
    updateColHeaderActive();
  });
});

viewListBtn.addEventListener('click', () => {
  state.mode = 'list';
  bookListPaneEl.classList.remove('grid');
  viewListBtn.classList.add('active');
  viewGridBtn.classList.remove('active');
  saveState();
});
viewGridBtn.addEventListener('click', () => {
  state.mode = 'grid';
  bookListPaneEl.classList.add('grid');
  viewGridBtn.classList.add('active');
  viewListBtn.classList.remove('active');
  saveState();
});

prevBtn.addEventListener('click', () => { state.page--; renderList(); });
nextBtn.addEventListener('click', () => { state.page++; renderList(); });

// Arrow-key navigation through the current filtered/sorted list, crossing
// page boundaries automatically. Ignored while typing in the search box.
function moveSelection(delta) {
  if (!state.items.length) return;
  const idx = state.items.findIndex(b => b.id === state.activeId);
  const next = Math.max(0, Math.min(state.items.length - 1, (idx === -1 ? 0 : idx) + delta));
  const newPage = Math.floor(next / PAGE_SIZE) + 1;
  state.activeId = state.items[next].id;
  if (newPage !== state.page) {
    state.page = newPage;
    renderList();
    selectBook(state.activeId, true);
  } else {
    selectBook(state.activeId, true);
  }
}
document.addEventListener('keydown', (e) => {
  if (e.target === searchEl || state.panel !== 'library') return;
  if (e.key === 'ArrowDown') { e.preventDefault(); moveSelection(1); }
  else if (e.key === 'ArrowUp') { e.preventDefault(); moveSelection(-1); }
});

// --- Statistics panel (toggled in place of <main>, scoped to the current
// sous-collection) ---
function computeStats() {
  const books = allBooks.filter(b => b.collection === state.collection);
  const tally = getter => {
    const m = new Map();
    for (const b of books) {
      for (const v of getter(b)) { if (v) m.set(v, (m.get(v) || 0) + 1); }
    }
    return [...m.entries()].sort((a, b) => b[1] - a[1]);
  };
  const decades = new Map();
  for (const b of books) {
    const y = parseInt(b.publication_year);
    if (!y) continue;
    const d = Math.floor(y / 10) * 10;
    decades.set(d, (decades.get(d) || 0) + 1);
  }
  return {
    totalFiches: books.length,
    totalQty: books.reduce((s, b) => s + (b.quantity || 1), 0),
    totalValue: books.reduce((s, b) => s + (b.current_value || 0), 0),
    noCover: books.filter(b => !b.cover_url).length,
    noIsbn: books.filter(b => !b.isbn).length,
    noAuthor: books.filter(b => !(b.authors && b.authors.length)).length,
    genres: tally(b => b.genres || []),
    formats: tally(b => b.format ? [b.format] : []),
    languages: tally(b => b.language ? [b.language] : []),
    publishers: tally(b => b.publisher ? [b.publisher] : []),
    decades: [...decades.entries()].sort((a, b) => a[0] - b[0]).map(([d, c]) => [`${d}s`, c]),
  };
}

function barList(title, entries, limit) {
  const top = limit ? entries.slice(0, limit) : entries;
  const max = top.reduce((m, [, c]) => Math.max(m, c), 1);
  const rows = top.map(([label, count]) => `
    <div class="stat-row">
      <span class="label">${escapeHtml(label)}</span>
      <span class="bar-wrap"><span class="bar" style="width:${(count / max * 100).toFixed(1)}%"></span></span>
      <span class="val">${count}</span>
    </div>`).join('');
  return `<div class="stat-block"><h3>${escapeHtml(title)}</h3>${rows || `<p class="stat-empty">${escapeHtml(t('stats_no_data'))}</p>`}</div>`;
}

function renderStats() {
  const s = computeStats();
  const moneyBit = s.totalValue ? `<span><strong>${s.totalValue.toFixed(2)}</strong> ${escapeHtml(t('stats_value_suffix'))}</span>` : '';
  statsPaneEl.innerHTML = `
    <div class="stats-wrap">
      <h2>${escapeHtml(t('stats_title', { name: state.collection }))}</h2>
      <div class="stat-summary">
        <span>${t('stats_entries', { n: `<strong>${s.totalFiches}</strong>` })}</span>
        <span>${t('stats_copies', { n: `<strong>${s.totalQty}</strong>` })}</span>
        ${moneyBit}
        <span>${t('stats_no_cover', { n: `<strong>${s.noCover}</strong>` })}</span>
        <span>${t('stats_no_isbn', { n: `<strong>${s.noIsbn}</strong>` })}</span>
        <span>${t('stats_no_author', { n: `<strong>${s.noAuthor}</strong>` })}</span>
      </div>
      <div class="stat-grid">
        ${barList(t('detail_group_genres'), s.genres, 15)}
        ${barList(t('stats_formats'), s.formats)}
        ${barList(t('stats_languages'), s.languages)}
        ${barList(t('stats_publishers_top10'), s.publishers, 10)}
        ${barList(t('stats_by_decade'), s.decades)}
      </div>
    </div>`;
}

// --- Panel switcher: exactly one of library / stats / add is visible ---
function setPanel(panel) {
  state.panel = panel;
  document.body.classList.toggle('panel-stats', panel === 'stats');
  document.body.classList.toggle('panel-add', panel === 'add');
  statsBtn.classList.toggle('active', panel === 'stats');
  statsBtn.textContent = panel === 'stats' ? t('nav_back_to_collection') : t('nav_stats');
  addBtn.classList.toggle('active', panel === 'add');
  addBtn.textContent = panel === 'add' ? t('nav_back_to_collection') : t('nav_add');
  if (panel === 'stats') renderStats();
  if (panel === 'add') renderAddForm();
}

statsBtn.addEventListener('click', () => {
  setPanel(state.panel === 'stats' ? 'library' : 'stats');
});
addBtn.addEventListener('click', () => {
  // The only way into "edit" mode is editBook() (the detail pane's
  // "Modifier" button) — this top button always means "start fresh" or
  // "go back", so any in-progress edit is abandoned here.
  if (editingBookId !== null) {
    editingBookId = null;
    editingBookData = null;
    resetAddFormState();
  }
  setPanel(state.panel === 'add' ? 'library' : 'add');
});

// --- "Add a book" form: pure client-side, step 1 of the add-a-book plan.
// It never talks to a server or the database — submitting it only
// downloads a new_book.json, which bc_add then reads (read-only) to
// resolve each lookup field against the existing collection. See
// MANUAL.md § 7. ---
function distinctFieldValues(getter) {
  const set = new Set();
  for (const b of allBooks) {
    for (const v of getter(b)) {
      if (v) set.add(v);
    }
  }
  return [...set].sort(collator.compare);
}

function setupChipField(inputId, chipsId, arr) {
  const input = document.getElementById(inputId);
  const chipsEl = document.getElementById(chipsId);
  function renderChips() {
    chipsEl.innerHTML = '';
    arr.forEach((val, i) => {
      const chip = document.createElement('span');
      chip.className = 'chip';
      const chipLabel = document.createElement('span');
      chipLabel.textContent = val;
      const x = document.createElement('span');
      x.className = 'chip-x';
      x.textContent = '✕';
      x.title = t('common_remove');
      x.addEventListener('click', () => {
        arr.splice(i, 1);
        renderChips();
      });
      chip.appendChild(chipLabel);
      chip.appendChild(x);
      chipsEl.appendChild(chip);
    });
  }
  input.addEventListener('keydown', (e) => {
    if (e.key !== 'Enter') return;
    e.preventDefault();
    const v = input.value.trim();
    if (v && !arr.includes(v)) {
      arr.push(v);
      input.value = '';
      renderChips();
    }
  });
  renderChips();
  return {
    render: renderChips,
    clear: () => { arr.length = 0; renderChips(); },
  };
}

/// Renders a list of free-form entries (characters, contents, links) with a
/// "✕" to remove each one — the repeatable-group equivalent of a chip field
/// for entries that carry more than one sub-field.
function setupRepeatableList(listId, arr, renderLabel) {
  const listEl = document.getElementById(listId);
  function render() {
    listEl.innerHTML = '';
    arr.forEach((entry, i) => {
      const row = document.createElement('div');
      row.className = 'rep-row';
      const label = document.createElement('span');
      label.textContent = renderLabel(entry);
      const x = document.createElement('span');
      x.className = 'chip-x';
      x.textContent = '✕';
      x.title = t('common_remove');
      x.addEventListener('click', () => {
        arr.splice(i, 1);
        render();
      });
      row.appendChild(label);
      row.appendChild(x);
      listEl.appendChild(row);
    });
  }
  render();
  return { render };
}

const ADD_TABS = [
  { id: 'main', key: 'add_tab_main' },
  { id: 'details', key: 'add_tab_details' },
  { id: 'personal', key: 'add_tab_personal' },
  { id: 'credits', key: 'add_tab_credits' },
  { id: 'contents', key: 'add_tab_contents' },
  { id: 'plot', key: 'add_tab_plot' },
  { id: 'links', key: 'add_tab_links' },
];

function renderAddForm() {
  const collOptions = collections
    .map(c => `<option value="${escapeHtml(c)}"${c === state.collection ? ' selected' : ''}>${escapeHtml(c)}</option>`)
    .join('');
  const tabButtons = ADD_TABS
    .map((tab, i) => `<button type="button" class="add-tab-btn${i === 0 ? ' active' : ''}" data-tab="${tab.id}">${t(tab.key)}</button>`)
    .join('');

  addPaneEl.innerHTML = `
    <div class="add-wrap">
      <h2>${window.__TAURI__ ? t('add_heading_tauri') : t('add_heading_static')}</h2>
      <p class="add-hint">${window.__TAURI__ ? t('add_hint_tauri') : t('add_hint_static')}</p>
      <div class="add-tabs">${tabButtons}</div>
      <form id="addForm" novalidate>
        <div id="addtab-main" class="add-tab-panel">
          <div class="add-grid">
            <label>${t('add_field_title')}<input name="title" required></label>
            <label>${t('add_field_subtitle')}<input name="subtitle"></label>
            <label>ISBN<input name="isbn"></label>
            <label>${t('add_field_number_of_pages')}<input name="number_of_pages" type="number" min="0"></label>
            <label>${t('add_field_year')}<input name="publication_year" placeholder="yyyy" maxlength="4"></label>
            <label>${t('add_field_month')}<input name="publication_month" placeholder="mm" maxlength="2"></label>
            <label>${t('add_field_day')}<input name="publication_day" placeholder="dd" maxlength="2"></label>
            <label>${t('add_field_publisher')}<input name="publisher" list="dl-publisher"></label>
            <label>Format<input name="format" list="dl-format"></label>
            <label>${t('add_field_language')}<input name="language" list="dl-language"></label>
            <label>${t('add_field_country')}<input name="country" list="dl-country"></label>
            <label>${t('add_field_location')}<input name="location" list="dl-location"></label>
            <label>${t('add_field_location_number')}<input name="location_number"></label>
            <label>${t('add_field_quantity')}<input name="quantity" type="number" min="1" value="1"></label>
            <label>${t('add_field_collection')}<select name="collection" required>${collOptions}</select></label>
          </div>
          <div class="add-list-field">
            <span class="label">${t('add_field_authors')}</span>
            <div id="authorsChips" class="chip-input"></div>
            <input id="authorInput" list="dl-author" placeholder="${t('add_placeholder_name_enter')}">
          </div>
          <div class="add-list-field">
            <span class="label">${t('add_field_genres')}</span>
            <div id="genresChips" class="chip-input"></div>
            <input id="genreInput" list="dl-genre" placeholder="${t('add_placeholder_genre_enter')}">
          </div>
          <div class="add-list-field">
            <span class="label">${t('add_field_subjects')}</span>
            <div id="subjectsChips" class="chip-input"></div>
            <input id="subjectInput" list="dl-subject" placeholder="${t('add_placeholder_subject_enter')}">
          </div>
        </div>

        <div id="addtab-details" class="add-tab-panel hidden">
          <div class="add-grid">
            <label>${t('add_field_issue_number')}<input name="issue_number" type="number" min="0"></label>
            <label>${t('add_field_edition')}<input name="edition" list="dl-edition"></label>
            <label>${t('add_field_printed_by')}<input name="printed_by" list="dl-printedby"></label>
            <label>${t('add_field_series')}<input name="series" list="dl-series"></label>
            <label>${t('add_field_volume')}<input name="volume" list="dl-volume"></label>
            <label>${t('add_field_paper_type')}<input name="paper_type" list="dl-papertype"></label>
            <label>${t('add_field_height')}<input name="height" placeholder="${t('add_placeholder_height')}"></label>
            <label>${t('add_field_width')}<input name="width" placeholder="${t('add_placeholder_width')}"></label>
            <label>Dewey<input name="dewey"></label>
            <label>LCCN<input name="lccn"></label>
            <label>${t('add_field_printing')}<input name="printing" type="number" min="1"></label>
            <label>${t('add_field_audio_length')}<input name="audio_length_minutes" type="number" min="0"></label>
          </div>
          <label class="add-full">${t('add_field_dedication')}<textarea name="dedication" rows="2"></textarea></label>
          <div class="add-checkrow">
            <label class="add-check"><input type="checkbox" name="first_edition"> ${t('add_check_first_edition')}</label>
            <label class="add-check"><input type="checkbox" name="abridged"> ${t('add_check_abridged')}</label>
            <label class="add-check"><input type="checkbox" name="box_set"> ${t('add_check_box_set')}</label>
          </div>
          <h3 class="add-subhead">${t('add_subhead_original_edition')}</h3>
          <div class="add-grid">
            <label>${t('add_field_original_title')}<input name="original_title"></label>
            <label>${t('add_field_original_subtitle')}<input name="original_subtitle"></label>
            <label>${t('add_field_original_publisher')}<input name="original_publisher" list="dl-publisher"></label>
            <label>${t('add_field_original_country')}<input name="original_country" list="dl-country"></label>
            <label>${t('add_field_original_language')}<input name="original_language" list="dl-language"></label>
            <label>${t('add_field_original_year')}<input name="original_publication_year" placeholder="yyyy" maxlength="4"></label>
            <label>${t('add_field_original_month')}<input name="original_publication_month" placeholder="mm" maxlength="2"></label>
            <label>${t('add_field_original_day')}<input name="original_publication_day" placeholder="dd" maxlength="2"></label>
          </div>
        </div>

        <div id="addtab-personal" class="add-tab-panel hidden">
          <div class="add-grid">
            <label>${t('add_field_condition')}<input name="condition" list="dl-condition"></label>
            <label>${t('add_field_owner')}<input name="owner" list="dl-owner"></label>
            <label>${t('add_field_store')}<input name="store" list="dl-store"></label>
            <label>${t('add_field_current_value')}<input name="current_value" type="number" step="0.01" min="0"></label>
            <label>${t('add_field_purchase_price')}<input name="purchase_price" type="number" step="0.01" min="0"></label>
            <label>${t('add_field_cover_price')}<input name="cover_price" type="number" step="0.01" min="0"></label>
            <label>${t('add_field_purchase_year')}<input name="purchase_year" placeholder="yyyy" maxlength="4"></label>
            <label>${t('add_field_purchase_month')}<input name="purchase_month" placeholder="mm" maxlength="2"></label>
            <label>${t('add_field_purchase_day')}<input name="purchase_day" placeholder="dd" maxlength="2"></label>
            <label>${t('add_field_read_times')}<input name="read_times" type="number" min="0"></label>
            <label>${t('add_field_read_year')}<input name="read_year" placeholder="yyyy" maxlength="4"></label>
            <label>${t('add_field_read_month')}<input name="read_month" placeholder="mm" maxlength="2"></label>
            <label>${t('add_field_read_day')}<input name="read_day" placeholder="dd" maxlength="2"></label>
            <label>${t('add_field_rating')}<input name="my_rating" type="number" min="0" max="10"></label>
          </div>
          <div class="add-checkrow">
            <label class="add-check"><input type="checkbox" name="rare"> ${t('add_check_rare')}</label>
            <label class="add-check"><input type="checkbox" name="read_it"> ${t('add_check_read_it')}</label>
          </div>
          <h3 class="add-subhead">${t('add_subhead_custom_fields')}</h3>
          <div class="add-grid">
            <label>${t('add_field_custom_lookup1')}<input name="user_lookup1" list="dl-userlookup1"></label>
            <label>${t('add_field_custom_lookup2')}<input name="user_lookup2" list="dl-userlookup2"></label>
            <label>${t('add_field_custom_text1')}<input name="user_text1"></label>
            <label>${t('add_field_custom_text2')}<input name="user_text2"></label>
          </div>
        </div>

        <div id="addtab-credits" class="add-tab-panel hidden">
          <div class="add-rep-field">
            <span class="label">${t('add_field_credits_label')}</span>
            <div id="creditsList" class="rep-list"></div>
            <div class="rep-add-row">
              <input id="creditNameInput" list="dl-credit" placeholder="${t('add_placeholder_name')}">
              <button type="button" id="creditAddBtn">${t('common_add')}</button>
            </div>
          </div>
          <div class="add-rep-field">
            <span class="label">${t('add_field_characters_label')}</span>
            <div id="charactersList" class="rep-list"></div>
            <div class="rep-add-row">
              <input id="characterNameInput" placeholder="${t('add_placeholder_character_name')}">
              <input id="characterAliasInput" placeholder="${t('add_placeholder_alias')}">
              <button type="button" id="characterAddBtn">${t('common_add')}</button>
            </div>
          </div>
        </div>

        <div id="addtab-contents" class="add-tab-panel hidden">
          <p class="add-hint">${t('add_contents_hint')}</p>
          <div id="contentsList" class="rep-list"></div>
          <div class="add-rep-field">
            <div class="add-grid">
              <label>${t('add_field_content_title')}<input id="contentTitleInput"></label>
              <label>${t('add_field_content_subtitle')}<input id="contentSubtitleInput"></label>
              <label>${t('add_field_number_of_pages')}<input id="contentPagesInput" type="number" min="0"></label>
              <label>${t('add_field_start_page')}<input id="contentStartPageInput" type="number" min="0"></label>
            </div>
            <div class="add-list-field">
              <span class="label">${t('add_field_content_authors')}</span>
              <div id="contentAuthorsChips" class="chip-input"></div>
              <input id="contentAuthorInput" list="dl-author" placeholder="${t('add_placeholder_name_enter')}">
            </div>
            <button type="button" id="contentAddBtn">${t('add_btn_add_content')}</button>
          </div>
        </div>

        <div id="addtab-plot" class="add-tab-panel hidden">
          <label class="add-full">${t('add_field_plot')}<textarea name="plot" rows="4"></textarea></label>
          <label class="add-full">Notes<textarea name="notes" rows="4"></textarea></label>
        </div>

        <div id="addtab-links" class="add-tab-panel hidden">
          <div id="linksList" class="rep-list"></div>
          <div class="rep-add-row">
            <input id="linkUrlInput" placeholder="https://…">
            <input id="linkDescInput" placeholder="${t('add_placeholder_description')}">
            <button type="button" id="linkAddBtn">${t('common_add')}</button>
          </div>
        </div>

        <datalist id="dl-publisher"></datalist>
        <datalist id="dl-format"></datalist>
        <datalist id="dl-language"></datalist>
        <datalist id="dl-country"></datalist>
        <datalist id="dl-location"></datalist>
        <datalist id="dl-author"></datalist>
        <datalist id="dl-genre"></datalist>
        <datalist id="dl-subject"></datalist>
        <datalist id="dl-series"></datalist>
        <datalist id="dl-condition"></datalist>
        <datalist id="dl-edition"></datalist>
        <datalist id="dl-printedby"></datalist>
        <datalist id="dl-volume"></datalist>
        <datalist id="dl-papertype"></datalist>
        <datalist id="dl-owner"></datalist>
        <datalist id="dl-store"></datalist>
        <datalist id="dl-credit"></datalist>
        <datalist id="dl-userlookup1"></datalist>
        <datalist id="dl-userlookup2"></datalist>

        <div class="add-actions">
          <button type="submit" class="primary">${window.__TAURI__ ? t('add_submit_tauri') : t('add_submit_static')}</button>
          <span id="addMsg"></span>
        </div>
      </form>
    </div>

    <div id="addConfirmOverlay" class="confirm-overlay hidden">
      <div class="confirm-card">
        <h3>${t('add_confirm_title_heading')}</h3>
        <p class="confirm-title" id="addConfirmTitle"></p>
        <p class="confirm-hint" id="addConfirmHint"></p>
        <div class="confirm-fields" id="addConfirmFields"></div>
        <div class="confirm-actions">
          <button type="button" id="addConfirmCancel">${t('common_cancel')}</button>
          <button type="button" id="addConfirmOk" class="primary">${t('common_save')}</button>
        </div>
      </div>
    </div>`;

  const fillDatalist = (id, values) => {
    document.getElementById(id).innerHTML = values.map(v => `<option value="${escapeHtml(v)}">`).join('');
  };
  // Suggestions drawn from values already used somewhere in the loaded
  // collection(s) — helps match existing spelling exactly, which is what
  // bc_add's case-insensitive resolution needs to "reuse" rather than
  // "create" a lookup value. credits/characters still have none — no
  // ZCREATOR/ZCHARACTER data exported yet (those tables were empty in the
  // test collection, see bc_add's step 5 "non couvert" notes).
  fillDatalist('dl-publisher', distinctFieldValues(b => [b.publisher]));
  fillDatalist('dl-format', distinctFieldValues(b => [b.format]));
  fillDatalist('dl-language', distinctFieldValues(b => [b.language]));
  fillDatalist('dl-country', distinctFieldValues(b => [b.country]));
  fillDatalist('dl-location', distinctFieldValues(b => [b.location]));
  fillDatalist('dl-author', distinctFieldValues(b => b.authors || []));
  fillDatalist('dl-genre', distinctFieldValues(b => b.genres || []));
  fillDatalist('dl-subject', distinctFieldValues(b => b.subjects || []));
  fillDatalist('dl-series', distinctFieldValues(b => [b.series]));
  fillDatalist('dl-condition', distinctFieldValues(b => [b.condition]));
  fillDatalist('dl-edition', distinctFieldValues(b => [b.edition]));
  fillDatalist('dl-printedby', distinctFieldValues(b => [b.printed_by]));
  fillDatalist('dl-volume', distinctFieldValues(b => [b.volume]));
  fillDatalist('dl-papertype', distinctFieldValues(b => [b.paper_type]));
  fillDatalist('dl-owner', distinctFieldValues(b => [b.owner]));
  fillDatalist('dl-store', distinctFieldValues(b => [b.store]));

  setupChipField('authorInput', 'authorsChips', addFormState.authors);
  setupChipField('genreInput', 'genresChips', addFormState.genres);
  setupChipField('subjectInput', 'subjectsChips', addFormState.subjects);
  // No dl-credit/dl-userlookup1/dl-userlookup2 autocomplete yet: bc_export
  // doesn't currently export ZCREATOR credit names or ZUSERLOOKUP1/2
  // values, so there's no client-side data to suggest from — the <datalist>
  // elements stay present but empty rather than offering nothing useful.

  document.querySelectorAll('.add-tab-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      document.querySelectorAll('.add-tab-btn').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      document.querySelectorAll('.add-tab-panel').forEach(p => p.classList.add('hidden'));
      document.getElementById('addtab-' + btn.dataset.tab).classList.remove('hidden');
    });
  });

  const creditsListHandle = setupRepeatableList(
    'creditsList', addFormState.credits,
    c => c.name,
  );
  document.getElementById('creditAddBtn').addEventListener('click', () => {
    const nameEl = document.getElementById('creditNameInput');
    const name = nameEl.value.trim();
    if (!name) return;
    addFormState.credits.push({ name });
    nameEl.value = '';
    creditsListHandle.render();
  });

  const charactersListHandle = setupRepeatableList(
    'charactersList', addFormState.characters,
    c => c.alias ? `${c.name} (alias : ${c.alias})` : c.name,
  );
  document.getElementById('characterAddBtn').addEventListener('click', () => {
    const nameEl = document.getElementById('characterNameInput');
    const aliasEl = document.getElementById('characterAliasInput');
    const name = nameEl.value.trim();
    if (!name) return;
    addFormState.characters.push({ name, alias: aliasEl.value.trim() || null });
    nameEl.value = '';
    aliasEl.value = '';
    charactersListHandle.render();
  });

  const linksListHandle = setupRepeatableList(
    'linksList', addFormState.links,
    l => l.description ? `${l.description} — ${l.url}` : l.url,
  );
  document.getElementById('linkAddBtn').addEventListener('click', () => {
    const urlEl = document.getElementById('linkUrlInput');
    const descEl = document.getElementById('linkDescInput');
    const url = urlEl.value.trim();
    if (!url) return;
    addFormState.links.push({ url, description: descEl.value.trim() || null });
    urlEl.value = '';
    descEl.value = '';
    linksListHandle.render();
  });

  const contentsListHandle = setupRepeatableList(
    'contentsList', addFormState.contents,
    c => `${c.title}${c.authors.length ? ' — ' + c.authors.join(', ') : ''}`,
  );
  let contentDraftAuthors = [];
  const contentAuthorsChips = setupChipField('contentAuthorInput', 'contentAuthorsChips', contentDraftAuthors);
  document.getElementById('contentAddBtn').addEventListener('click', () => {
    const titleEl = document.getElementById('contentTitleInput');
    const subtitleEl = document.getElementById('contentSubtitleInput');
    const pagesEl = document.getElementById('contentPagesInput');
    const startPageEl = document.getElementById('contentStartPageInput');
    const title = titleEl.value.trim();
    if (!title) return;
    addFormState.contents.push({
      title,
      subtitle: subtitleEl.value.trim() || null,
      number_of_pages: pagesEl.value ? parseInt(pagesEl.value, 10) : null,
      start_page: startPageEl.value ? parseInt(startPageEl.value, 10) : null,
      authors: [...contentDraftAuthors],
    });
    titleEl.value = '';
    subtitleEl.value = '';
    pagesEl.value = '';
    startPageEl.value = '';
    contentAuthorsChips.clear();
    contentsListHandle.render();
  });

  document.getElementById('addForm').addEventListener('submit', onAddFormSubmit);
  document.getElementById('addConfirmCancel').addEventListener('click', closeAddConfirm);
  document.getElementById('addConfirmOk').addEventListener('click', confirmAddBook);

  // Edit mode: pre-fill every plain input/select/textarea from
  // editingBookData (the chip/repeatable-list fields are already populated
  // via addFormState, set up before this render in editBook()) and swap
  // the heading/hint/submit label so it reads as "modifier", not "ajouter".
  if (editingBookData) {
    const form = document.getElementById('addForm');
    for (const el of form.querySelectorAll('[name]')) {
      const value = editingBookData[el.name];
      if (value === undefined) continue;
      if (el.type === 'checkbox') {
        el.checked = !!value;
      } else if (value !== null) {
        el.value = value;
      }
    }
    const wrap = document.querySelector('.add-wrap');
    wrap.querySelector('h2').textContent = t('add_heading_edit', { title: editingBookData.title });
    wrap.querySelector('.add-hint').textContent = t('add_hint_edit');
    wrap.querySelector('.add-actions button[type="submit"]').textContent = t('add_submit_edit');
  }
}

function onAddFormSubmit(e) {
  e.preventDefault();
  const fd = new FormData(e.target);
  const str = (name) => {
    const v = (fd.get(name) || '').toString().trim();
    return v ? v : null;
  };
  const num = (name) => {
    const v = str(name);
    return v ? parseInt(v, 10) : null;
  };
  const flt = (name) => {
    const v = str(name);
    return v ? parseFloat(v) : null;
  };
  const bool = (name) => fd.get(name) === 'on';
  const msgEl = document.getElementById('addMsg');

  const title = str('title');
  const collectionName = str('collection');
  if (!title) {
    msgEl.textContent = t('add_err_title_required');
    return;
  }
  if (!collectionName) {
    msgEl.textContent = t('add_err_collection_required');
    return;
  }

  // Field names match bc_add's NewBookInput exactly (src/bin/bc_add.rs).
  const book = {
    title,
    subtitle: str('subtitle'),
    isbn: str('isbn'),
    authors: [...addFormState.authors],
    publisher: str('publisher'),
    format: str('format'),
    language: str('language'),
    country: str('country'),
    number_of_pages: num('number_of_pages'),
    publication_year: str('publication_year'),
    publication_month: str('publication_month'),
    publication_day: str('publication_day'),
    genres: [...addFormState.genres],
    subjects: [...addFormState.subjects],
    location: str('location'),
    location_number: str('location_number'),
    quantity: num('quantity') || 1,
    collection: collectionName,

    issue_number: num('issue_number'),
    edition: str('edition'),
    printed_by: str('printed_by'),
    series: str('series'),
    volume: str('volume'),
    paper_type: str('paper_type'),
    height: str('height'),
    width: str('width'),
    dewey: str('dewey'),
    lccn: str('lccn'),
    dedication: str('dedication'),
    first_edition: bool('first_edition'),
    printing: num('printing'),
    abridged: bool('abridged'),
    box_set: bool('box_set'),
    audio_length_minutes: num('audio_length_minutes'),
    original_title: str('original_title'),
    original_subtitle: str('original_subtitle'),
    original_publisher: str('original_publisher'),
    original_country: str('original_country'),
    original_language: str('original_language'),
    original_publication_year: str('original_publication_year'),
    original_publication_month: str('original_publication_month'),
    original_publication_day: str('original_publication_day'),

    condition: str('condition'),
    owner: str('owner'),
    store: str('store'),
    current_value: flt('current_value'),
    purchase_price: flt('purchase_price'),
    cover_price: flt('cover_price'),
    purchase_year: str('purchase_year'),
    purchase_month: str('purchase_month'),
    purchase_day: str('purchase_day'),
    rare: bool('rare'),
    read_it: bool('read_it'),
    read_times: num('read_times'),
    read_year: str('read_year'),
    read_month: str('read_month'),
    read_day: str('read_day'),
    my_rating: num('my_rating'),
    user_lookup1: str('user_lookup1'),
    user_lookup2: str('user_lookup2'),
    user_text1: str('user_text1'),
    user_text2: str('user_text2'),

    plot: str('plot'),
    notes: str('notes'),

    credits: addFormState.credits.map(c => ({ ...c })),
    characters: addFormState.characters.map(c => ({ ...c })),
    contents: addFormState.contents.map(c => ({ ...c, authors: [...c.authors] })),
    links: addFormState.links.map(l => ({ ...l })),
  };

  if (window.__TAURI__) {
    openAddConfirm(book, msgEl);
    return;
  }

  const blob = new Blob([JSON.stringify(book, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = 'new_book.json';
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
  msgEl.textContent = t('add_static_download_success');
}

// --- Tauri-only confirmation step: resolve+plan the book against the
// currently-loaded collection via IPC, show a human-readable preview (any
// value that would create a brand-new Publisher/Genre/etc. entry flagged
// "nouveau"), and only call commit_add_book once the user explicitly
// confirms. Never reachable from bc_export's static HTML export. ---
let pendingAddBook = null;

async function openAddConfirm(book, msgEl) {
  msgEl.textContent = '';
  msgEl.className = '';
  try {
    // Same resolution logic whether adding or editing — the preview never
    // touches the specific existing row, only the field values being
    // resolved — so there's no "prepare_update_book", just this.
    const preview = await window.__TAURI__.core.invoke('prepare_add_book', { path: currentCollectionPath, input: book });
    pendingAddBook = book;
    document.getElementById('addConfirmTitle').textContent = preview.title;
    // Only say something here when there's actually something to flag —
    // a reassurance that "everything already exists" or a placeholder
    // explaining an empty list are both noise for an everyday user who
    // just wants to confirm the add; the one thing genuinely worth a
    // warning is a brand-new value about to be created (e.g. a typo'd
    // author name that would otherwise silently become a permanent new
    // entry in the collection) — found by the user looking at this dialog
    // on a simple book with nothing new on it (v0.48.0).
    const hintEl = document.getElementById('addConfirmHint');
    hintEl.classList.toggle('hidden', preview.new_count === 0);
    hintEl.textContent = preview.new_count > 0 ? t('add_confirm_hint_new', { n: preview.new_count }) : '';

    const fieldsEl = document.getElementById('addConfirmFields');
    fieldsEl.classList.toggle('hidden', preview.fields.length === 0);
    fieldsEl.innerHTML = preview.fields.map(f => `
          <div class="confirm-field">
            <span class="confirm-field-label">${escapeHtml(f.label)}</span>
            <span class="confirm-field-value">${escapeHtml(f.value)}</span>
            ${f.is_new ? `<span class="confirm-badge-new">${escapeHtml(t('add_confirm_badge_new'))}</span>` : ''}
          </div>`).join('');
    document.getElementById('addConfirmOverlay').classList.remove('hidden');
  } catch (err) {
    pendingAddBook = null;
    msgEl.textContent = String(err);
    msgEl.className = 'add-msg-error';
  }
}

function closeAddConfirm() {
  pendingAddBook = null;
  document.getElementById('addConfirmOverlay').classList.add('hidden');
}

async function confirmAddBook() {
  if (!pendingAddBook) return;
  const okBtn = document.getElementById('addConfirmOk');
  const msgEl = document.getElementById('addMsg');
  okBtn.disabled = true;
  okBtn.textContent = t('add_confirm_saving');
  const wasEditing = editingBookId !== null;
  try {
    const args = wasEditing
      ? { path: currentCollectionPath, bookId: editingBookId, input: pendingAddBook }
      : { path: currentCollectionPath, input: pendingAddBook };
    const result = await window.__TAURI__.core.invoke(wasEditing ? 'commit_update_book' : 'commit_add_book', args);
    closeAddConfirm();
    editingBookId = null;
    editingBookData = null;
    resetAddFormState();
    setPanel('library');
    bootWithErrorHandling();
    // After an edit, the updated book is already visible (same id, same
    // place in the list) — that's confirmation enough. After an add there
    // is nothing equivalent to look at without searching for the new
    // title, so without an explicit signal here, a user who doesn't
    // immediately search can't tell whether the save actually worked
    // (found and fixed in v0.44.0, after exactly that confusion led to a
    // book being added twice).
    if (!wasEditing) window.alert(result);
  } catch (err) {
    closeAddConfirm();
    msgEl.textContent = String(err);
    msgEl.className = 'add-msg-error';
  } finally {
    okBtn.disabled = false;
    okBtn.textContent = t('common_save');
  }
}

// --- Draggable splitters (left/right panes, and top/bottom right panes) ---
const MIN_GROUPS_W = 160, MAX_GROUPS_W = 480;
const MIN_LIST_H = 120, MIN_DETAIL_H = 160;

function makeDraggable(handle, onDrag) {
  let dragging = false;
  handle.addEventListener('mousedown', (e) => {
    dragging = true;
    handle.classList.add('dragging');
    document.body.style.userSelect = 'none';
    e.preventDefault();
  });
  window.addEventListener('mousemove', (e) => {
    if (dragging) onDrag(e);
  });
  window.addEventListener('mouseup', () => {
    if (!dragging) return;
    dragging = false;
    handle.classList.remove('dragging');
    document.body.style.userSelect = '';
    saveState();
  });
}

makeDraggable(splitterVEl, (e) => {
  const rect = mainEl.getBoundingClientRect();
  const w = Math.max(MIN_GROUPS_W, Math.min(MAX_GROUPS_W, e.clientX - rect.left));
  groupsEl.style.width = w + 'px';
});

makeDraggable(splitterHEl, (e) => {
  const rect = rightColEl.getBoundingClientRect();
  const maxH = rect.height - MIN_DETAIL_H - splitterHEl.offsetHeight;
  const h = Math.max(MIN_LIST_H, Math.min(maxH, e.clientY - rect.top));
  bookListPaneEl.style.flex = `0 0 ${h}px`;
});

// Per-browser convenience only (not synced, not read back by Claude) — if
// storage is unavailable (private window, blocked site data, ...) the page
// just keeps its defaults. Covers both pane sizes and "where was I"
// (collection/group/sort/search/page/selection) continuity across reloads.
function saveState() {
  try {
    localStorage.setItem('bc_state', JSON.stringify({
      groupsWidth: groupsEl.style.width,
      listHeight: bookListPaneEl.style.flex,
      collection: state.collection,
      groupField: state.groupField,
      groupValue: state.groupValue,
      azLetter: state.azLetter,
      groupSortByCount: state.groupSortByCount,
      query: state.query,
      sort: state.sort,
      mode: state.mode,
      page: state.page,
      activeId: state.activeId,
    }));
  } catch (e) { /* ignore */ }
}

function restoreState() {
  try {
    const raw = localStorage.getItem('bc_state');
    if (!raw) return;
    const s = JSON.parse(raw);
    if (s.groupsWidth) groupsEl.style.width = s.groupsWidth;
    if (s.listHeight) bookListPaneEl.style.flex = s.listHeight;
    if (s.collection && collections.includes(s.collection)) state.collection = s.collection;
    if (s.groupField && GROUP_FIELDS[s.groupField]) state.groupField = s.groupField;
    state.groupValue = (s.groupValue === undefined) ? null : s.groupValue;
    state.azLetter = s.azLetter || null;
    state.groupSortByCount = !!s.groupSortByCount;
    state.query = s.query || '';
    if (s.sort && SORTERS[s.sort]) state.sort = s.sort;
    state.page = s.page || 1;
    state.activeId = (s.activeId === undefined) ? null : s.activeId;
    if (s.mode === 'grid') {
      state.mode = 'grid';
      bookListPaneEl.classList.add('grid');
      viewGridBtn.classList.add('active');
      viewListBtn.classList.remove('active');
    }
    searchEl.value = state.query;
    sortEl.value = state.sort;
    groupFieldEl.value = state.groupField;
  } catch (e) { /* ignore */ }
}

// Shows which file is actually open — relevant once "File > Import a
// collection…" (step 4) lets the user switch collections at runtime, since
// otherwise there's no on-screen way to tell which one is loaded.
function updateFileBadge() {
  const el = document.getElementById('fileBadge');
  if (!el) return;
  const parts = currentCollectionPath.replace(/\/+$/, '').split('/').filter(Boolean);
  const file = parts[parts.length - 1] || currentCollectionPath;
  const dir = parts.length > 1 ? parts[parts.length - 2] : '';
  el.textContent = dir ? `📂 ${file} — ${dir}` : `📂 ${file}`;
  el.title = currentCollectionPath;
}

// bc_export's static export already burns its own version into #appVersion
// at generation time (see src/main.rs's __VERSION__ substitution) — nothing
// to do there. The BookNojo app has no such build-time step for its HTML shell
// (app/src/index.template.html is copied as-is), so it asks the Rust side
// for its own Cargo.toml version instead, over the same IPC mechanism as
// everything else.
async function updateVersionBadge() {
  if (!window.__TAURI__) return;
  try {
    const version = await window.__TAURI__.core.invoke('app_version');
    const el = document.getElementById('appVersion');
    if (el) el.textContent = `Ⓥ BookNojo v${version}`;
  } catch (e) {
    // Non-critical — leave the static placeholder label in place.
  }
}

// Every static (non-data-driven) label that lives in the HTML template
// rather than being generated by a render function above — <option> text,
// placeholders, pager/view-toggle buttons, column headers. Run once at
// module load (before boot(), so the loading phase itself isn't stuck in
// the wrong language) and again on every setLang().
function applyStaticLabels() {
  const setOptionText = (selectEl, value, key) => {
    const opt = selectEl && selectEl.querySelector(`option[value="${value}"]`);
    if (opt) opt.textContent = t(key);
  };
  setOptionText(groupFieldEl, 'authors', 'opt_group_authors');
  setOptionText(groupFieldEl, 'genres', 'opt_group_genres');
  setOptionText(groupFieldEl, 'publisher', 'opt_group_publisher');
  setOptionText(groupFieldEl, 'format', 'opt_group_format');
  setOptionText(groupFieldEl, 'language', 'opt_group_language');
  setOptionText(groupFieldEl, 'country', 'opt_group_country');

  setOptionText(sortEl, 'title', 'opt_sort_title');
  setOptionText(sortEl, 'author', 'opt_sort_author');
  setOptionText(sortEl, 'year_desc', 'opt_sort_year_desc');
  setOptionText(sortEl, 'publisher', 'opt_sort_publisher');

  searchEl.placeholder = t('list_search_placeholder');
  viewListBtn.textContent = t('list_view_list');
  viewGridBtn.textContent = t('list_view_grid');
  prevBtn.textContent = t('list_prev');
  nextBtn.textContent = t('list_next');

  const colTitle = colHeaderEl.querySelector('[data-sort="title"]');
  if (colTitle) colTitle.textContent = t('col_title');
  const colAuthor = colHeaderEl.querySelector('[data-sort="author"]');
  if (colAuthor) colAuthor.textContent = t('col_author');

  // Mirrors setPanel()'s own toggle logic for whichever panel isn't active
  // right now — setPanel() itself handles the label the moment either
  // button is actually clicked.
  statsBtn.textContent = state.panel === 'stats' ? t('nav_back_to_collection') : t('nav_stats');
  addBtn.textContent = state.panel === 'add' ? t('nav_back_to_collection') : t('nav_add');

  // Only matters for the brief moment before the first boot() render
  // replaces detailEl's contents entirely (or if setLang() runs during
  // that window) — harmless to set unconditionally either way.
  const emptyEl = document.getElementById('empty');
  if (emptyEl) emptyEl.textContent = t('list_select_book');
}

// Builds the FR/EN toggle once and inserts it right after the version badge
// — created in JS rather than added to the two HTML templates, so there's
// one source of truth for its label text (like everything else here) and
// nothing to keep in sync between templates/page.html and
// app/src/index.template.html.
function initLangToggle() {
  const toggle = document.createElement('div');
  toggle.id = 'langToggle';
  for (const lang of ['fr', 'en']) {
    const btn = document.createElement('button');
    btn.type = 'button';
    btn.dataset.lang = lang;
    btn.textContent = lang.toUpperCase();
    btn.addEventListener('click', () => setLang(lang));
    toggle.appendChild(btn);
  }
  const versionEl = document.getElementById('appVersion');
  if (versionEl) versionEl.insertAdjacentElement('afterend', toggle);
  updateLangToggleUI();
}

function updateLangToggleUI() {
  const toggle = document.getElementById('langToggle');
  if (!toggle) return;
  toggle.querySelectorAll('button').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.lang === currentLang);
  });
}

function setLang(lang) {
  if ((lang !== 'fr' && lang !== 'en') || lang === currentLang) return;
  currentLang = lang;
  try { localStorage.setItem('bc_lang', lang); } catch (e) { /* ignore */ }
  applyLanguageChange();
}

// Re-renders every visible part of the app in the newly selected language.
// Note: if the Add-book form is open, this rebuilds it from scratch via
// renderAddForm() — any typed-but-not-yet-submitted plain field value is
// lost (the authors/genres/credits/etc. lists in addFormState survive,
// since those live outside the form markup, not inside it). Switching
// language mid-fill is rare enough that this tradeoff isn't worth the
// extra complexity of preserving every field's live value across a full
// form rebuild.
function applyLanguageChange() {
  updateLangToggleUI();
  applyStaticLabels();
  if (allBooks) {
    renderTabs();
    refreshGroupPanel();
    recompute();
  }
  if (state.panel === 'stats') renderStats();
  if (state.panel === 'add') renderAddForm();
}

// One tab per sous-collection, largest first (ties broken alphabetically).
// Runs only after allBooks has actually loaded — synchronously already
// resolved for bc_export's static export, awaited for the BookNojo app's live
// Tauri command — so every function above that closes over
// allBooks/collCounts/collections only ever runs once they're populated.
async function boot() {
  updateFileBadge();
  updateVersionBadge();
  allBooks = await loadAllBooks();

  collCounts = new Map();
  for (const b of allBooks) collCounts.set(b.collection, (collCounts.get(b.collection) || 0) + 1);
  collections = [...collCounts.keys()].sort((a, b) =>
    collCounts.get(b) - collCounts.get(a) || collator.compare(a, b));
  state.collection = collections[0];

  restoreState();
  renderTabs();
  refreshGroupPanel();
  recompute();
  updateColHeaderActive();
}

function bootWithErrorHandling() {
  boot().catch(err => {
    console.error('BookNojo: échec du chargement de la collection', err);
    document.body.innerHTML =
      '<div style="padding:40px;font-family:sans-serif;color:#c0392b;">'
      + `<h2>${escapeHtml(t('boot_load_failed_heading'))}</h2><pre>` + escapeHtml(String(err)) + '</pre></div>';
  });
}

// "File > Import a file..." (step 4): the native picker runs on the Rust
// side (app/src-tauri/src/lib.rs) and sends the chosen path as this event,
// since picking a file is inherently async/callback-based there. Switches
// the whole page over to the new collection and remembers it for next time.
if (window.__TAURI__) {
  window.__TAURI__.event.listen('import-collection', (event) => {
    currentCollectionPath = event.payload;
    try { localStorage.setItem('bc_collection_path', currentCollectionPath); } catch (e) { /* ignore */ }
    bootWithErrorHandling();
  });
}

initLangToggle();
applyStaticLabels();
bootWithErrorHandling();
