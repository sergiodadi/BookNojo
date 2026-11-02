// In the NBC desktop app, there's no embedded `<script id="data">` block —
// the collection is loaded live through a Tauri command instead (see
// `boot()` at the bottom of this file). `window.__TAURI__` only exists
// inside that app (tauri.conf.json: withGlobalTauri), so it reliably tells
// apart that case from bc_export's static HTML export, which keeps working
// exactly as before (synchronous, embedded JSON, opened in any browser).
//
// Fallback when nothing better is known yet (first launch, nothing in
// localStorage): this dev machine's sample collection (see
// testdata-sample/, build-sample-testdata.sh). "File > Import a file..."
// (step 4) picks a real one and overrides it from then on, remembered
// across relaunches the same way the rest of the UI state already is.
const DEV_COLLECTION_PATH = '/Users/serge/Developer/book_collector_export/testdata-sample/MyCollection.bkcp';
let currentCollectionPath = DEV_COLLECTION_PATH;
try {
  const saved = localStorage.getItem('bc_collection_path');
  if (saved) currentCollectionPath = saved;
} catch (e) { /* ignore */ }

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
// file by bc_export's HTML export, but the NBC desktop app serves the page
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

// Canonical role key (what's stored/round-tripped, see role_id_for_role in
// lib.rs) -> French display label. Shared by the Crédits sub-tab's
// repeatable list and the read-only detail-card view.
const ROLE_LABELS = {
  Translator: 'Traducteur', CoverArtist: 'Artiste de couverture', Editor: 'Éditeur (rôle)',
  Illustrator: 'Illustrateur', Ghostwriter: 'Nègre littéraire', ForewordAuthor: 'Auteur de la préface',
  Photographer: 'Photographe', Narrator: 'Narrateur',
  UserRole1: 'Rôle personnalisé 1', UserRole2: 'Rôle personnalisé 2', UserRole3: 'Rôle personnalisé 3',
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
  return [...counts.entries()].sort((a, b) => collator.compare(a[0], b[0]));
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
  tout.textContent = 'Tout';
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
    makeGroupRow('Tous', totalForCollection, state.groupValue === null, () => selectGroup(null)));
  for (const [key, count] of computeGroupCounts()) {
    groupListEl.appendChild(makeGroupRow(key, count, state.groupValue === key, () => selectGroup(key)));
  }
}

function makeGroupRow(label, count, active, onClick) {
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
    const t = document.createElement('div');
    t.className = 't';
    t.textContent = b.title || '(sans titre)';
    const a = document.createElement('div');
    a.className = 'a';
    a.textContent = b.authors.join(', ');
    txt.appendChild(t);
    txt.appendChild(a);
    row.appendChild(txt);

    row.addEventListener('click', () => selectBook(b.id));
    rowsEl.appendChild(row);
  }

  const totalPages = Math.max(1, Math.ceil(state.items.length / PAGE_SIZE));
  pageInfoEl.textContent = `Page ${state.page} / ${totalPages}`;
  prevBtn.disabled = state.page <= 1;
  nextBtn.disabled = state.page >= totalPages;
  const scope = state.groupValue !== null
    ? `"${state.groupValue}"`
    : collCounts.get(state.collection) + ' livres';
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
    detailEl.innerHTML = '<div id="empty">Aucun livre dans cette vue.</div>';
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
  if (!b) { detailEl.innerHTML = '<div id="empty">Livre introuvable.</div>'; return; }

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
  if (b.current_value) moneyBits.push(`<span>Valeur actuelle : ${b.current_value}</span>`);
  if (b.purchase_price) moneyBits.push(`<span>Prix d'achat : ${b.purchase_price}</span>`);

  const isGenreActive = g => state.groupField === 'genres' && state.groupValue === g;
  const genreTags = (b.genres || []).map(g =>
    `<span class="tag genre${isGenreActive(g) ? ' selected' : ''}" data-genre="${escapeHtml(g)}">${escapeHtml(g)}</span>`).join('');
  const subjectTags = (b.subjects || []).map(s => `<span class="tag subject">${escapeHtml(s)}</span>`).join('');
  const userTags = (b.tags || []).map(t => `<span class="tag usertag">${escapeHtml(t)}</span>`).join('');

  // "Product Details" / "Personal Details" — section names match Book
  // Collector's own detail view, which these were missing entirely against.
  const dimensions = [b.height, b.width].filter(Boolean).join(' × ');
  const productDetails = kvGroup('Détails produit', [
    ['Édition', b.edition],
    ['Série', b.series ? (b.volume ? `${b.series}, vol. ${b.volume}` : b.series) : ''],
    ['Imprimé par', b.printed_by],
    ['Type de papier', b.paper_type],
    ['Dimensions', dimensions],
    ['Dewey', b.dewey],
    ['LCCN', b.lccn],
    ['Dédicace', b.dedication],
    ['Première édition', b.first_edition ? 'Oui' : ''],
    ['Tirage', b.printing],
    ['Coffret', b.box_set ? 'Oui' : ''],
    ['Durée audio', b.audio_length_minutes ? `${b.audio_length_minutes} min` : ''],
    ['N° de parution (Issue)', b.issue_number],
  ]);
  const originalPubDate = [b.original_publication_day, b.original_publication_month, b.original_publication_year]
    .filter(Boolean).join('/');
  const originalEdition = kvGroup('Édition originale', [
    ['Titre original', b.original_title],
    ['Sous-titre original', b.original_subtitle],
    ['Éditeur original', b.original_publisher],
    ['Pays original', b.original_country],
    ['Langue originale', b.original_language],
    ['Date de publication originale', originalPubDate],
  ]);
  const purchaseDate = [b.purchase_day, b.purchase_month, b.purchase_year].filter(Boolean).join('/');
  const readDate = [b.read_day, b.read_month, b.read_year].filter(Boolean).join('/');
  const personalDetails = kvGroup('Détails personnels', [
    ['Propriétaire', b.owner],
    ['Magasin', b.store],
    ["N° d'emplacement", b.location_number],
    ['Prix de couverture', b.cover_price],
    ["Date d'achat", purchaseDate],
    ['Rare', b.rare ? 'Oui' : ''],
    ['Lu', b.read_it ? 'Oui' : ''],
    ['Nombre de lectures', b.read_times],
    ['Date de lecture', readDate],
    ['Champ personnalisé 1', b.user_lookup1 || b.user_text1],
    ['Champ personnalisé 2', b.user_lookup2 || b.user_text2],
    ['Ajouté le', b.date_added ? b.date_added.slice(0, 10) : ''],
  ]);

  const creditsSection = repGroup('Crédits', (b.credits || []).map(c => {
    const role = c.role ? (ROLE_LABELS[c.role] || c.role) : '';
    return `<div class="rep-item">${escapeHtml(c.name)}${role ? ` <span class="rep-sub">(${escapeHtml(role)})</span>` : ''}</div>`;
  }));
  const charactersSection = repGroup('Personnages', (b.characters || []).map(c =>
    `<div class="rep-item">${escapeHtml(c.name)}${c.alias ? ` <span class="rep-sub">(alias : ${escapeHtml(c.alias)})</span>` : ''}</div>`));
  const contentsSection = repGroup('Contenus', (b.contents || []).map(c => {
    const bits = [];
    if (c.number_of_pages) bits.push(`${c.number_of_pages} p.`);
    if (c.start_page) bits.push(`débute p. ${c.start_page}`);
    if (c.authors && c.authors.length) bits.push(c.authors.join(', '));
    const sub = c.subtitle ? ` — ${escapeHtml(c.subtitle)}` : '';
    return `<div class="rep-item">${escapeHtml(c.title)}${sub}${bits.length ? ` <span class="rep-sub">(${escapeHtml(bits.join(' · '))})</span>` : ''}</div>`;
  }));
  const linksSection = repGroup('Liens', (b.links || []).map(l =>
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
        ${genreTags ? `<div class="group"><div class="label">Genres</div><div class="tags">${genreTags}</div></div>` : ''}
        ${subjectTags ? `<div class="group"><div class="label">Sujets</div><div class="tags">${subjectTags}</div></div>` : ''}
        ${userTags ? `<div class="group"><div class="label">Tags</div><div class="tags">${userTags}</div></div>` : ''}
        ${productDetails}
        ${originalEdition}
        ${personalDetails}
        ${creditsSection}
        ${charactersSection}
        ${contentsSection}
        ${linksSection}
        ${b.plot ? '<div class="group"><div class="label">Résumé</div><div class="text-block plot"></div></div>' : ''}
        ${b.notes ? '<div class="group"><div class="label">Notes</div><div class="text-block notes"></div></div>' : ''}
        ${window.__TAURI__ ? '<div class="detail-actions"><button type="button" id="editBookBtn" class="secondary">✏️ Modifier ce livre</button><button type="button" id="deleteBookBtn" class="danger">🗑️ Supprimer ce livre</button></div>' : ''}
      </div>
    </div>`;

  // Free-text fields set via textContent (never innerHTML) to rule out any HTML injection.
  detailEl.querySelector('.title').textContent = b.title || '(sans titre)';
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
    window.alert(`Impossible de charger ce livre pour modification : ${err}`);
  }
}

// Tauri-only: delete the given book after a native confirm(), then reload
// the whole collection (simplest correct way to reflect the removal —
// matches the Add-book flow, which doesn't try to surgically patch
// allBooks/collCounts either). Never reachable from bc_export's static
// export (no window.__TAURI__, no button rendered in the first place).
async function deleteBookWithConfirm(b) {
  const label = b.title || '(sans titre)';
  if (!window.confirm(`Supprimer « ${label} » ? Une sauvegarde est prise avant, mais cette action n'est pas réversible depuis NBC.`)) {
    return;
  }
  try {
    await window.__TAURI__.core.invoke('delete_book', { path: currentCollectionPath, bookId: b.id });
    bootWithErrorHandling();
  } catch (err) {
    window.alert(`Échec de la suppression : ${err}`);
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
  return `<div class="stat-block"><h3>${escapeHtml(title)}</h3>${rows || '<p class="stat-empty">Aucune donnée.</p>'}</div>`;
}

function renderStats() {
  const s = computeStats();
  const moneyBit = s.totalValue ? `<span><strong>${s.totalValue.toFixed(2)}</strong> de valeur totale</span>` : '';
  statsPaneEl.innerHTML = `
    <div class="stats-wrap">
      <h2>Statistiques — ${escapeHtml(state.collection)}</h2>
      <div class="stat-summary">
        <span><strong>${s.totalFiches}</strong> fiches</span>
        <span><strong>${s.totalQty}</strong> exemplaires</span>
        ${moneyBit}
        <span><strong>${s.noCover}</strong> sans couverture</span>
        <span><strong>${s.noIsbn}</strong> sans ISBN</span>
        <span><strong>${s.noAuthor}</strong> sans auteur</span>
      </div>
      <div class="stat-grid">
        ${barList('Genres', s.genres, 15)}
        ${barList('Formats', s.formats)}
        ${barList('Langues', s.languages)}
        ${barList('Éditeurs (top 10)', s.publishers, 10)}
        ${barList('Par décennie de publication', s.decades)}
      </div>
    </div>`;
}

// --- Panel switcher: exactly one of library / stats / add is visible ---
function setPanel(panel) {
  state.panel = panel;
  document.body.classList.toggle('panel-stats', panel === 'stats');
  document.body.classList.toggle('panel-add', panel === 'add');
  statsBtn.classList.toggle('active', panel === 'stats');
  statsBtn.textContent = panel === 'stats' ? '📚 Collection' : '📊 Statistiques';
  addBtn.classList.toggle('active', panel === 'add');
  addBtn.textContent = panel === 'add' ? '📚 Collection' : '➕ Ajouter un livre';
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
      const t = document.createElement('span');
      t.textContent = val;
      const x = document.createElement('span');
      x.className = 'chip-x';
      x.textContent = '✕';
      x.title = 'Retirer';
      x.addEventListener('click', () => {
        arr.splice(i, 1);
        renderChips();
      });
      chip.appendChild(t);
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
      x.title = 'Retirer';
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
  { id: 'main', label: 'Principal' },
  { id: 'details', label: 'Détails' },
  { id: 'personal', label: 'Personnel' },
  { id: 'credits', label: 'Crédits & Personnages' },
  { id: 'contents', label: 'Contenus' },
  { id: 'plot', label: 'Intrigue & Notes' },
  { id: 'links', label: 'Liens' },
];

function renderAddForm() {
  const collOptions = collections
    .map(c => `<option value="${escapeHtml(c)}"${c === state.collection ? ' selected' : ''}>${escapeHtml(c)}</option>`)
    .join('');
  const tabButtons = ADD_TABS
    .map((t, i) => `<button type="button" class="add-tab-btn${i === 0 ? ' active' : ''}" data-tab="${t.id}">${t.label}</button>`)
    .join('');

  addPaneEl.innerHTML = `
    <div class="add-wrap">
      <h2>${window.__TAURI__ ? 'Ajouter un livre' : 'Ajouter un livre (préparation — étapes 1 à 5)'}</h2>
      <p class="add-hint">${window.__TAURI__
        ? `Ce formulaire écrit directement dans la collection ouverte, après confirmation.
           L'onglet Couvertures n'est pas repris ici (gestion de fichiers, hors périmètre).`
        : `Ce formulaire ne modifie aucune donnée existante : il génère un
           fichier <code>new_book.json</code> à télécharger, à passer ensuite à
           <code>bc_add</code> pour vérifier ce qui existe déjà avant toute écriture réelle.
           L'onglet Couvertures n'est pas repris ici (gestion de fichiers, hors périmètre).`}</p>
      <div class="add-tabs">${tabButtons}</div>
      <form id="addForm" novalidate>
        <div id="addtab-main" class="add-tab-panel">
          <div class="add-grid">
            <label>Titre *<input name="title" required></label>
            <label>Sous-titre<input name="subtitle"></label>
            <label>ISBN<input name="isbn"></label>
            <label>Nombre de pages<input name="number_of_pages" type="number" min="0"></label>
            <label>Année<input name="publication_year" placeholder="yyyy" maxlength="4"></label>
            <label>Mois<input name="publication_month" placeholder="mm" maxlength="2"></label>
            <label>Jour<input name="publication_day" placeholder="dd" maxlength="2"></label>
            <label>Éditeur<input name="publisher" list="dl-publisher"></label>
            <label>Format<input name="format" list="dl-format"></label>
            <label>Langue<input name="language" list="dl-language"></label>
            <label>Pays<input name="country" list="dl-country"></label>
            <label>Emplacement<input name="location" list="dl-location"></label>
            <label>N° d'emplacement<input name="location_number"></label>
            <label>Quantité<input name="quantity" type="number" min="1" value="1"></label>
            <label>Sous-collection *<select name="collection" required>${collOptions}</select></label>
          </div>
          <div class="add-list-field">
            <span class="label">Auteurs</span>
            <div id="authorsChips" class="chip-input"></div>
            <input id="authorInput" list="dl-author" placeholder="Nom, puis Entrée">
          </div>
          <div class="add-list-field">
            <span class="label">Genres</span>
            <div id="genresChips" class="chip-input"></div>
            <input id="genreInput" list="dl-genre" placeholder="Genre, puis Entrée">
          </div>
          <div class="add-list-field">
            <span class="label">Sujets</span>
            <div id="subjectsChips" class="chip-input"></div>
            <input id="subjectInput" list="dl-subject" placeholder="Sujet, puis Entrée">
          </div>
        </div>

        <div id="addtab-details" class="add-tab-panel hidden">
          <div class="add-grid">
            <label>Issue N°<input name="issue_number" type="number" min="0"></label>
            <label>Édition<input name="edition" list="dl-edition"></label>
            <label>Imprimé par<input name="printed_by" list="dl-printedby"></label>
            <label>Série<input name="series" list="dl-series"></label>
            <label>Volume<input name="volume" list="dl-volume"></label>
            <label>Type de papier<input name="paper_type" list="dl-papertype"></label>
            <label>Hauteur<input name="height" placeholder="ex. 21 cm"></label>
            <label>Largeur<input name="width" placeholder="ex. 14 cm"></label>
            <label>Dewey<input name="dewey"></label>
            <label>LCCN<input name="lccn"></label>
            <label>Tirage<input name="printing" type="number" min="1"></label>
            <label>Durée audio (min)<input name="audio_length_minutes" type="number" min="0"></label>
          </div>
          <label class="add-full">Dédicace<textarea name="dedication" rows="2"></textarea></label>
          <div class="add-checkrow">
            <label class="add-check"><input type="checkbox" name="first_edition"> Première édition</label>
            <label class="add-check"><input type="checkbox" name="abridged"> Abrégé</label>
            <label class="add-check"><input type="checkbox" name="box_set"> Coffret</label>
          </div>
          <h3 class="add-subhead">Édition originale</h3>
          <div class="add-grid">
            <label>Titre original<input name="original_title"></label>
            <label>Sous-titre original<input name="original_subtitle"></label>
            <label>Éditeur original<input name="original_publisher" list="dl-publisher"></label>
            <label>Pays original<input name="original_country" list="dl-country"></label>
            <label>Langue originale<input name="original_language" list="dl-language"></label>
            <label>Année orig.<input name="original_publication_year" placeholder="yyyy" maxlength="4"></label>
            <label>Mois orig.<input name="original_publication_month" placeholder="mm" maxlength="2"></label>
            <label>Jour orig.<input name="original_publication_day" placeholder="dd" maxlength="2"></label>
          </div>
        </div>

        <div id="addtab-personal" class="add-tab-panel hidden">
          <div class="add-grid">
            <label>État<input name="condition" list="dl-condition"></label>
            <label>Propriétaire<input name="owner" list="dl-owner"></label>
            <label>Magasin<input name="store" list="dl-store"></label>
            <label>Valeur actuelle<input name="current_value" type="number" step="0.01" min="0"></label>
            <label>Prix d'achat<input name="purchase_price" type="number" step="0.01" min="0"></label>
            <label>Prix de couverture<input name="cover_price" type="number" step="0.01" min="0"></label>
            <label>Année d'achat<input name="purchase_year" placeholder="yyyy" maxlength="4"></label>
            <label>Mois d'achat<input name="purchase_month" placeholder="mm" maxlength="2"></label>
            <label>Jour d'achat<input name="purchase_day" placeholder="dd" maxlength="2"></label>
            <label>Nombre de lectures<input name="read_times" type="number" min="0"></label>
            <label>Année de lecture<input name="read_year" placeholder="yyyy" maxlength="4"></label>
            <label>Mois de lecture<input name="read_month" placeholder="mm" maxlength="2"></label>
            <label>Jour de lecture<input name="read_day" placeholder="dd" maxlength="2"></label>
            <label>Note (0–10)<input name="my_rating" type="number" min="0" max="10"></label>
          </div>
          <div class="add-checkrow">
            <label class="add-check"><input type="checkbox" name="rare"> Rare</label>
            <label class="add-check"><input type="checkbox" name="read_it"> Lu</label>
          </div>
          <h3 class="add-subhead">Champs personnalisés</h3>
          <div class="add-grid">
            <label>Champ personnalisé 1 (liste)<input name="user_lookup1" list="dl-userlookup1"></label>
            <label>Champ personnalisé 2 (liste)<input name="user_lookup2" list="dl-userlookup2"></label>
            <label>Champ personnalisé 1 (texte)<input name="user_text1"></label>
            <label>Champ personnalisé 2 (texte)<input name="user_text2"></label>
          </div>
        </div>

        <div id="addtab-credits" class="add-tab-panel hidden">
          <div class="add-rep-field">
            <span class="label">Crédits (traducteur, illustrateur...)</span>
            <div id="creditsList" class="rep-list"></div>
            <div class="rep-add-row">
              <input id="creditNameInput" list="dl-credit" placeholder="Nom">
              <select id="creditRoleInput">
                <option value="">— Rôle (optionnel) —</option>
                <option value="Translator">Traducteur</option>
                <option value="CoverArtist">Artiste de couverture</option>
                <option value="Editor">Éditeur (rôle)</option>
                <option value="Illustrator">Illustrateur</option>
                <option value="Ghostwriter">Nègre littéraire</option>
                <option value="ForewordAuthor">Auteur de la préface</option>
                <option value="Photographer">Photographe</option>
                <option value="Narrator">Narrateur</option>
                <option value="UserRole1">Rôle personnalisé 1</option>
                <option value="UserRole2">Rôle personnalisé 2</option>
                <option value="UserRole3">Rôle personnalisé 3</option>
              </select>
              <button type="button" id="creditAddBtn">Ajouter</button>
            </div>
          </div>
          <div class="add-rep-field">
            <span class="label">Personnages</span>
            <div id="charactersList" class="rep-list"></div>
            <div class="rep-add-row">
              <input id="characterNameInput" placeholder="Nom du personnage">
              <input id="characterAliasInput" placeholder="Alias (optionnel)">
              <button type="button" id="characterAddBtn">Ajouter</button>
            </div>
          </div>
        </div>

        <div id="addtab-contents" class="add-tab-panel hidden">
          <p class="add-hint">Plusieurs histoires/nouvelles dans ce livre, chacune avec ses propres auteurs.</p>
          <div id="contentsList" class="rep-list"></div>
          <div class="add-rep-field">
            <div class="add-grid">
              <label>Titre<input id="contentTitleInput"></label>
              <label>Sous-titre<input id="contentSubtitleInput"></label>
              <label>Nombre de pages<input id="contentPagesInput" type="number" min="0"></label>
              <label>Page de début<input id="contentStartPageInput" type="number" min="0"></label>
            </div>
            <div class="add-list-field">
              <span class="label">Auteurs de ce contenu</span>
              <div id="contentAuthorsChips" class="chip-input"></div>
              <input id="contentAuthorInput" list="dl-author" placeholder="Nom, puis Entrée">
            </div>
            <button type="button" id="contentAddBtn">➕ Ajouter ce contenu</button>
          </div>
        </div>

        <div id="addtab-plot" class="add-tab-panel hidden">
          <label class="add-full">Intrigue / résumé<textarea name="plot" rows="4"></textarea></label>
          <label class="add-full">Notes<textarea name="notes" rows="4"></textarea></label>
        </div>

        <div id="addtab-links" class="add-tab-panel hidden">
          <div id="linksList" class="rep-list"></div>
          <div class="rep-add-row">
            <input id="linkUrlInput" placeholder="https://…">
            <input id="linkDescInput" placeholder="Description (optionnel)">
            <button type="button" id="linkAddBtn">Ajouter</button>
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
          <button type="submit" class="primary">${window.__TAURI__ ? '💾 Enregistrer' : '⬇️ Générer new_book.json'}</button>
          <span id="addMsg"></span>
        </div>
      </form>
    </div>

    <div id="addConfirmOverlay" class="confirm-overlay hidden">
      <div class="confirm-card">
        <h3>Confirmer l'ajout</h3>
        <p class="confirm-title" id="addConfirmTitle"></p>
        <p class="confirm-hint" id="addConfirmHint"></p>
        <div class="confirm-fields" id="addConfirmFields"></div>
        <div class="confirm-actions">
          <button type="button" id="addConfirmCancel">Annuler</button>
          <button type="button" id="addConfirmOk" class="primary">Enregistrer</button>
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
    c => c.role ? `${c.name} (${ROLE_LABELS[c.role] || c.role})` : c.name,
  );
  document.getElementById('creditAddBtn').addEventListener('click', () => {
    const nameEl = document.getElementById('creditNameInput');
    const roleEl = document.getElementById('creditRoleInput');
    const name = nameEl.value.trim();
    if (!name) return;
    addFormState.credits.push({ name, role: roleEl.value || null });
    nameEl.value = '';
    roleEl.value = '';
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
    wrap.querySelector('h2').textContent = `Modifier : ${editingBookData.title}`;
    wrap.querySelector('.add-hint').textContent =
      'Les champs ci-dessous reflètent les valeurs actuelles de ce livre. Une confirmation sera demandée avant toute écriture.';
    wrap.querySelector('.add-actions button[type="submit"]').textContent = '💾 Enregistrer les modifications';
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
    msgEl.textContent = 'Le titre est obligatoire.';
    return;
  }
  if (!collectionName) {
    msgEl.textContent = 'La sous-collection est obligatoire.';
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
  msgEl.textContent = 'new_book.json téléchargé — passe-le à bc_add pour le vérifier.';
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
    const hintEl = document.getElementById('addConfirmHint');
    if (preview.new_count > 0) {
      hintEl.textContent = `${preview.new_count} nouvelle(s) valeur(s) seront créées (marquées "nouveau" ci-dessous) :`;
    } else {
      hintEl.textContent = 'Toutes les valeurs ci-dessous existent déjà dans la collection.';
    }
    const fieldsEl = document.getElementById('addConfirmFields');
    fieldsEl.innerHTML = preview.fields.length
      ? preview.fields.map(f => `
          <div class="confirm-field">
            <span class="confirm-field-label">${escapeHtml(f.label)}</span>
            <span class="confirm-field-value">${escapeHtml(f.value)}</span>
            ${f.is_new ? '<span class="confirm-badge-new">nouveau</span>' : ''}
          </div>`).join('')
      : '<p class="confirm-empty">(aucune valeur de type éditeur/genre/auteur/etc. sur cette fiche)</p>';
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
  okBtn.textContent = 'Enregistrement…';
  const wasEditing = editingBookId !== null;
  try {
    const args = wasEditing
      ? { path: currentCollectionPath, bookId: editingBookId, input: pendingAddBook }
      : { path: currentCollectionPath, input: pendingAddBook };
    const result = await window.__TAURI__.core.invoke(wasEditing ? 'commit_update_book' : 'commit_add_book', args);
    closeAddConfirm();
    if (wasEditing) {
      editingBookId = null;
      editingBookData = null;
      resetAddFormState();
      setPanel('library');
      bootWithErrorHandling();
    } else {
      msgEl.textContent = result;
      msgEl.className = 'add-msg-success';
    }
  } catch (err) {
    closeAddConfirm();
    msgEl.textContent = String(err);
    msgEl.className = 'add-msg-error';
  } finally {
    okBtn.disabled = false;
    okBtn.textContent = 'Enregistrer';
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
// to do there. The NBC app has no such build-time step for its HTML shell
// (app/src/index.template.html is copied as-is), so it asks the Rust side
// for its own Cargo.toml version instead, over the same IPC mechanism as
// everything else.
async function updateVersionBadge() {
  if (!window.__TAURI__) return;
  try {
    const version = await window.__TAURI__.core.invoke('app_version');
    const el = document.getElementById('appVersion');
    if (el) el.textContent = `Ⓥ NBC v${version}`;
  } catch (e) {
    // Non-critical — leave the static placeholder label in place.
  }
}

// One tab per sous-collection, largest first (ties broken alphabetically).
// Runs only after allBooks has actually loaded — synchronously already
// resolved for bc_export's static export, awaited for the NBC app's live
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
    console.error('NBC: échec du chargement de la collection', err);
    document.body.innerHTML =
      '<div style="padding:40px;font-family:sans-serif;color:#c0392b;">'
      + '<h2>Impossible de charger la collection</h2><pre>' + escapeHtml(String(err)) + '</pre></div>';
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

bootWithErrorHandling();
