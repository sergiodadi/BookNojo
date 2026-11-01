const allBooks = JSON.parse(document.getElementById('data').textContent);
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

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

// One tab per sous-collection, largest first (ties broken alphabetically).
const collCounts = new Map();
for (const b of allBooks) collCounts.set(b.collection, (collCounts.get(b.collection) || 0) + 1);
const collections = [...collCounts.keys()].sort((a, b) =>
  collCounts.get(b) - collCounts.get(a) || collator.compare(a, b));

const state = {
  collection: collections[0],
  groupField: 'authors',
  groupValue: null, // null = "Tous"
  azLetter: null,
  query: '',
  sort: 'title',
  mode: 'list',
  page: 1,
  items: [],
  activeId: null,
};

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
  if (document.body.classList.contains('show-stats')) renderStats();
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

    const rowThumb = b.thumb_url || b.cover_url;
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

  const cover = b.cover_url
    ? `<img class="cover" src="${b.cover_url}" alt="">`
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
        ${b.plot ? '<div class="group"><div class="label">Résumé</div><div class="text-block plot"></div></div>' : ''}
        ${b.notes ? '<div class="group"><div class="label">Notes</div><div class="text-block notes"></div></div>' : ''}
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

  saveState();
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
  if (e.target === searchEl || document.body.classList.contains('show-stats')) return;
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

statsBtn.addEventListener('click', () => {
  const showing = document.body.classList.toggle('show-stats');
  statsBtn.classList.toggle('active', showing);
  statsBtn.textContent = showing ? '📚 Collection' : '📊 Statistiques';
  if (showing) renderStats();
});

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

restoreState();
renderTabs();
refreshGroupPanel();
recompute();
updateColHeaderActive();
