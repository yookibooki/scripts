// ==UserScript==
// @name         Uzum Marketplace Collector
// @namespace    https://uzum.uz/
// @version      2.6.0
// @description  Collects Uzum (uzum.uz) marketplace product catalog into IndexedDB. Exports JSONL. One-shot collection, resume on restart.
// @author       
// @match        https://uzum.uz/*
// @match        https://www.uzum.uz/*
// @grant        GM_registerMenuCommand
// @grant        GM_addStyle
// @run-at       document-idle
// ==/UserScript==

/* ===================================================================
   CONFIG
   =================================================================== */
const CFG = {
  DB_NAME: 'uzum_product_db',
  DB_VERSION: 3,
  PRODUCTS_STORE: 'products',
  STATE_STORE: 'state',
  BATCH_SIZE: 48,
  REQUEST_DELAY_MS: 400,
  SAVE_INTERVAL: 50,
  OFFSET_LIMIT: 9936, // max safe offset (offset + BATCH < 10000)
  GRAPHQL_URL: 'https://graphql.uzum.uz/',
  REST_BASE: 'https://api.uzum.uz/api',
};

/* ===================================================================
   LOGGER
   =================================================================== */
const Log = {
  _el: null,
  init(el) { this._el = el; },
  _ts() { return new Date().toLocaleTimeString(); },
  info(msg) {
    console.log('[UzumCollector]', msg);
    if (this._el) {
      const d = document.createElement('div');
      d.textContent = `[${this._ts()}] ${msg}`;
      this._el.appendChild(d);
      this._el.scrollTop = this._el.scrollHeight;
    }
  },
  warn(msg) { console.warn('[UzumCollector]', msg); if (this._el) this.info('⚠ ' + msg); },
  error(msg) { console.error('[UzumCollector]', msg); if (this._el) this.info('✗ ' + msg); },
};

/* ===================================================================
   FETCH WRAPPER
   =================================================================== */
async function apiFetch(url, opts = {}) {
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), opts.timeout || 20000);
  try {
    const res = await fetch(url, {
      method: opts.method || 'GET',
      headers: { Accept: 'application/json', 'Content-Type': 'application/json', ...(opts.headers || {}) },
      body: opts.data,
      signal: ctrl.signal,
      credentials: opts.credentials || 'omit',
    });
    clearTimeout(timer);
    if (!res.ok) {
      const txt = res.status === 429 ? 'RateLimited' : ((await res.text().catch(() => '')).slice(0, 200));
      throw new Error(`HTTP ${res.status}: ${txt}`);
    }
    return res;
  } catch (e) {
    clearTimeout(timer);
    if (e.name === 'AbortError') throw new Error('Timeout');
    throw e;
  }
}

async function apiFetchJSON(url, opts) { return (await apiFetch(url, opts)).json(); }

/* ===================================================================
   COOKIE HELPERS
   =================================================================== */
function getCookie(name) {
  const m = document.cookie.match(new RegExp('(?:^|;)\\s*' + name.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '=([^;]*)'));
  return m ? decodeURIComponent(m[1]) : null;
}

function authHeaders(gql = true) {
  const token = getCookie('access_token');
  const iid = getCookie('clickstream-client.installId');
  const h = {};
  if (gql) Object.assign(h, { 'apollographql-client-name': 'web-customers', 'apollographql-client-version': '1.34.6', 'city-id': '1' });
  if (token) h['Authorization'] = 'Bearer ' + token;
  if (iid && gql) h['X-Iid'] = iid.replace(/^"/, '').replace(/"$/, '');
  return h;
}

/* ===================================================================
   GRAPHQL
   =================================================================== */
async function graphql(query, variables, operationName) {
  const body = { query, variables };
  if (operationName) body.operationName = operationName;
  const data = await apiFetchJSON(CFG.GRAPHQL_URL, {
    method: 'POST',
    headers: authHeaders(true),
    data: JSON.stringify(body),
    credentials: 'omit',
  });
  if (data.errors) throw new Error('GQL: ' + (data.errors[0]?.message || ''));
  return data.data;
}

/* ===================================================================
   INDEXED DB
   =================================================================== */
class ProductDB {
  constructor() { this.db = null; }
  open() {
    return new Promise((resolve, reject) => {
      const req = indexedDB.open(CFG.DB_NAME, CFG.DB_VERSION);
      req.onupgradeneeded = e => {
        const db = e.target.result;
        if (!db.objectStoreNames.contains(CFG.PRODUCTS_STORE)) {
          const s = db.createObjectStore(CFG.PRODUCTS_STORE, { keyPath: 'id' });
          s.createIndex('categoryId', 'categoryId', { unique: false });
          s.createIndex('lastSeen', 'lastSeen', { unique: false });
        }
        if (!db.objectStoreNames.contains(CFG.STATE_STORE)) db.createObjectStore(CFG.STATE_STORE, { keyPath: 'key' });
      };
      req.onsuccess = e => { this.db = e.target.result; resolve(); };
      req.onerror = e => reject(e.target.error);
    });
  }
  _tx(n, m = 'readonly') { return this.db.transaction(n, m).objectStore(n); }
  putProducts(prods) {
    if (!prods.length) return Promise.resolve();
    return new Promise(resolve => {
      const store = this._tx(CFG.PRODUCTS_STORE, 'readwrite');
      let done = 0;
      for (const p of prods) {
        const req = store.put(p);
        req.onsuccess = () => { done++; if (done === prods.length) resolve(); };
        req.onerror = () => { done++; if (done === prods.length) resolve(); };
      }
    });
  }
  getAllProducts() {
    return new Promise(resolve => { const req = this._tx(CFG.PRODUCTS_STORE).getAll(); req.onsuccess = () => resolve(req.result || []); req.onerror = () => resolve([]); });
  }
  getProductCount() {
    return new Promise(resolve => { const req = this._tx(CFG.PRODUCTS_STORE).count(); req.onsuccess = () => resolve(req.result); req.onerror = () => resolve(0); });
  }
  setState(key, value) { return new Promise(resolve => { const req = this._tx(CFG.STATE_STORE, 'readwrite').put({ key, value }); req.onsuccess = () => resolve(); }); }
  getState(key, def = null) { return new Promise(resolve => { const req = this._tx(CFG.STATE_STORE).get(key); req.onsuccess = () => resolve(req.result ? req.result.value : def); req.onerror = () => resolve(def); }); }
  deleteState(key) { return new Promise(resolve => { const req = this._tx(CFG.STATE_STORE, 'readwrite').delete(key); req.onsuccess = () => resolve(); }); }
  getProductsByIds(ids) {
    return new Promise(resolve => {
      if (!ids.length) return resolve(new Map());
      const store = this._tx(CFG.PRODUCTS_STORE);
      const map = new Map(); let d = 0;
      for (const id of ids) {
        const req = store.get(id);
        req.onsuccess = () => { if (req.result) map.set(String(req.result.id), req.result); d++; if (d === ids.length) resolve(map); };
        req.onerror = () => { d++; if (d === ids.length) resolve(map); };
      }
    });
  }
  exportAll() {
    // JSONL export — one JSON object per line, no wrapper. Append-friendly.
    return this.getAllProducts().then(products => {
      const header = JSON.stringify({ exportedAt: new Date().toISOString(), totalProducts: products.length, version: '2.6.0', source: 'uzum.uz' });
      const lines = products.map(p => JSON.stringify(p));
      return header + '\n' + lines.join('\n');
    });
  }
  // Strip old fat fields from legacy records (description, url, images, brand, seller, etc.)
  _slimProduct(p) {
    if (!p.url && !p.images && !('priceHistory' in p)) return p; // already slim
    return {
      id: p.id, title: p.title,
      price: p.price, oldPrice: p.oldPrice,
      discountPercent: p.discountPercent,
      rating: p.rating, reviewCount: p.reviewCount,
      category: p.category, categoryId: p.categoryId,
      firstSeen: p.firstSeen, lastSeen: p.lastSeen,
      // priceHistory removed in v2.4.0
    };
  }
  slimAll() {
    return new Promise(resolve => {
      const store = this._tx(CFG.PRODUCTS_STORE, 'readwrite');
      const req = store.getAll();
      req.onsuccess = () => {
        const all = req.result || [];
        let done = 0;
        for (const p of all) {
          const slim = this._slimProduct(p);
          if (slim !== p) store.put(slim);
          done++;
        }
        Log.info(`Cleaned ${all.length} products`);
        resolve();
      };
      req.onerror = () => resolve();
    });
  }
}

/* ===================================================================
   UZUM API
   =================================================================== */
class UzumApi {
  async restGet(path) {
    return apiFetchJSON(CFG.REST_BASE + path, { headers: authHeaders(false), credentials: 'include' });
  }

  async getCategories() {
    try {
      const data = await this.restGet('/main/root-categories');
      return data?.payload || [];
    } catch (e) {
      Log.warn('REST categories failed: ' + e.message);
      return [];
    }
  }

  async searchProducts(opts = {}) {
    const { categoryId, offset = 0, limit = CFG.BATCH_SIZE } = opts;
    const query = `
      query MakeSearch_ItemsAndFilters($queryInput: MakeSearchQueryInput!) {
        makeSearch(query: $queryInput) {
          items {
            catalogCard {
              id productId title adult
              minFullPrice minSellPrice
              feedbackQuantity rating
              discount { discountPrice }
            }
          }
          total
        }
      }
    `;
    const vars = {
      queryInput: {
        ...(categoryId ? { categoryId: String(categoryId) } : {}),
        showAdultContent: 'TRUE',
        filters: [],
        sort: 'BY_ORDERS_NUMBER_DESC',
        pagination: { offset, limit },
        correctQuery: false,
        getFastCategories: false,
      },
    };
    try {
      const data = await graphql(query, vars, 'MakeSearch_ItemsAndFilters');
      return data?.makeSearch || null;
    } catch (e) {
      if (e.message.includes('too big query offset')) {
        Log.warn(`offset ${offset} exceeds limit — returning empty`);
        return { items: [], total: null, _offsetLimit: true };
      }
      Log.error('search error: ' + e.message); return null;
    }
  }
}

/* ===================================================================
   PRODUCT COLLECTOR
   =================================================================== */
class ProductCollector {
  constructor(db, api) {
    this.db = db; this.api = api;
    this.running = false; this.aborted = false; this.collectedCount = 0;
    this._se = null; this._ce = null;
  }
  setUI(s, c) { this._se = s; this._ce = c; }
  _s(v) { if (this._se) this._se.textContent = v; }
  _c(v) { if (this._ce) this._ce.textContent = String(v); }

  async start() {
    const done = await this.db.getState('status');
    if (done === 'collection_done') {
      Log.info('Collection already done. Use Export JSON to re-export.');
      this._s('Done ✓'); this._c(await this.db.getProductCount());
      this.running = false; return true;
    }
    this.running = true; this.aborted = false;
    Log.info('Starting collection...'); this._s('Collecting...');
    try {
      const cats = await this._getLeaves();
      if (cats.length) await this._collectByCat(cats);
      else { Log.warn('No cats, trying search'); await this._collectBySearch(); }
      if (this.aborted) { this._s('Stopped'); this.running = false; return false; }
      const cnt = await this.db.getProductCount();
      Log.info(`Done. ${cnt} products`); this._s('Complete ✓'); this._c(cnt);
      await this.db.setState('status', 'collection_done');
      this.running = false; return true;
    } catch (e) { Log.error('Failed: ' + e.message); this._s('Error'); this.running = false; return false; }
  }

  async refresh() {
    Log.info('Refresh: scanning for new products...'); this._s('Refreshing...');
    const oldCount = await this.db.getProductCount();
    try {
      await this.db.deleteState('status');
      this.running = true; this.aborted = false;
      const cats = await this._getLeaves();
      // Ensure we have category totals to work with
      let stored = await this.db.getState('cat_totals', null);
      if (!stored) {
        Log.info('Building category index first...');
        stored = await this._bootstrapCatTotals(cats);
      }
      if (this.aborted) { this._s('Stopped'); await this.db.setState('status', 'collection_done'); this.running = false; return; }
      const scannedCats = await this._refreshByCat(cats, stored);
      this.running = false;
      const newCount = await this.db.getProductCount();
      const added = newCount - oldCount;
      Log.info(`Refresh done. ${added} new, ${newCount} total (${scannedCats} categories changed).`);
      await this.db.setState('status', 'collection_done');
      this._s('Done ✓'); this._c(newCount);
    } catch (e) {
      Log.error('Refresh failed: ' + e.message);
      await this.db.setState('status', 'collection_done');
      this._s('Error'); this.running = false;
    }
  }

  stop() { this.aborted = true; this.running = false; this._s('Stopping...'); }

  // Bootstrap cat_totals from DB counts + API totals (one-time after upgrade)
  async _bootstrapCatTotals(cats) {
    Log.info('Building category index from DB...');
    // Count existing products per category directly from IndexedDB
    const allProds = await this.db.getAllProducts();
    const catCounts = {};
    for (const p of allProds) {
      const cid = p.categoryId;
      if (cid) catCounts[cid] = (catCounts[cid] || 0) + 1;
    }
    const totals = {};
    for (let ci = 0; ci < cats.length; ci++) {
      if (this.aborted) return totals;
      const cat = cats[ci];
      if (!cat.id) continue;
      if (ci > 0 && ci % 50 === 0) {
        Log.info(`Category index: ${ci}/${cats.length} (${Math.round(ci/cats.length*100)}%)`);
      }
      let r = await this.api.searchProducts({ categoryId: cat.id, offset: 0, limit: 1 });
      // Retry once on failure (transient timeouts)
      if (!r || !r.total) {
        await delay(2000);
        r = await this.api.searchProducts({ categoryId: cat.id, offset: 0, limit: 1 });
      }
      if (r && r.total) {
        const have = catCounts[cat.id] || 0;
        totals[cat.id] = { total: r.total, offset: Math.min(have, r.total) };
      }
      // Throttle to avoid hammering the server
      await delay(CFG.REQUEST_DELAY_MS);
    }
    await this.db.setState('cat_totals', totals);
    Log.info(`Category index built (${Object.keys(totals).length} cats)`);
    return totals;
  }

  // Smart refresh: quick page-0 pass, then scan only categories that grew
  async _refreshByCat(cats, stored) {
    const totals = { ...stored };
    const changed = [];

    Log.info('Checking category totals...');
    for (let ci = 0; ci < cats.length && !this.aborted; ci++) {
      const cat = cats[ci];
      if (!cat.id) continue;
      if (ci > 0 && ci % 50 === 0) {
        Log.info(`Checking category ${ci}/${cats.length} (${Math.round(ci/cats.length*100)}%)`);
      }
      let r = await this.api.searchProducts({ categoryId: cat.id, offset: 0, limit: CFG.BATCH_SIZE });
      // Retry once on transient timeout
      if (!r) {
        await delay(2000);
        r = await this.api.searchProducts({ categoryId: cat.id, offset: 0, limit: CFG.BATCH_SIZE });
      }
      if (!r) continue;
      const curTotal = r.total || 0;
      const prev = stored[cat.id];
      const needDeep = !prev || curTotal > prev.total || (prev.offset < Math.min(prev.total, CFG.OFFSET_LIMIT));
      if (needDeep) {
        // upsert page 0 immediately — catches hot new products
        const items = (r.items || []).map(i => i?.catalogCard).filter(Boolean);
        if (items.length) {
          await this._upsert(items, cat);
          this.collectedCount += items.length; this._c(this.collectedCount);
        }
        changed.push(cat);
        totals[cat.id] = { total: curTotal, offset: prev ? prev.offset : 0 };
      } else {
        // category unchanged — still record its total in case totals object is fresh
        totals[cat.id] = prev;
      }
    }
    if (this.aborted) { await this.db.setState('cat_totals', totals); return 0; }

    Log.info(`${changed.length} categories changed, scanning new pages...`);
    for (const cat of changed) {
      if (this.aborted) break;
      const cur = totals[cat.id];
      await this._scanCategoryForward(cat, cur.offset, cur.total, totals);
    }

    await this.db.setState('cat_totals', totals);
    return changed.length;
  }

  // Scan a single category from offset to limit-or-total, update the totals map in-place
  async _scanCategoryForward(cat, startOffset, apiTotal, totalsMap) {
    const limit = Math.min(apiTotal, CFG.OFFSET_LIMIT);
    let offset = startOffset;
    let empty = 0;
    while (!this.aborted && offset < limit) {
      const r = await this.api.searchProducts({ categoryId: cat.id, offset });
      if (!r) { empty++; if (empty >= 3) break; await delay(1000); continue; }
      if (r._offsetLimit) break;
      empty = 0;
      const items = (r.items || []).map(i => i?.catalogCard).filter(Boolean);
      if (!items.length) break;
      await this._upsert(items, cat);
      this.collectedCount += items.length; this._c(this.collectedCount);
      offset += CFG.BATCH_SIZE;
      if (this.collectedCount % CFG.SAVE_INTERVAL === 0) {
        totalsMap[cat.id].offset = offset;
        await this.db.setState('cat_totals', totalsMap);
      }
      if (this.aborted) return;
      await delay(CFG.REQUEST_DELAY_MS);
    }
    totalsMap[cat.id].offset = Math.min(offset, limit);
  }

  async _getLeaves() {
    const cats = await this.api.getCategories(); if (!cats.length) return [];
    const flat = []; const walk = ns => { for (const n of ns) { flat.push(n); if (n.children?.length) walk(n.children); } }; walk(cats);
    const leaves = flat.filter(c => !c.children || !c.children.length); return leaves.length ? leaves : flat;
  }

  async _collectByCat(cats) {
    let ri = 0, ro = 0;
    ri = await this.db.getState('resume_cat_idx', 0); ro = await this.db.getState('resume_offset', 0);
    if (ro && ro >= CFG.OFFSET_LIMIT) { ri++; ro = 0; }
    const totals = {};
    for (let ci = 0; ci < cats.length; ci++) {
      if (this.aborted) return;
      if (ci < ri) continue;
      const cat = cats[ci];
      if (!cat.id) continue;
      let offset = (ci === ri) ? ro : 0;
      let empty = 0;
      let firstTotal = null;
      while (!this.aborted) {
        if (offset >= CFG.OFFSET_LIMIT) {
          Log.warn(`Category ${cat.title} (${cat.id}) has more products beyond offset limit`);
          break;
        }
        const r = await this.api.searchProducts({ categoryId: cat.id, offset });
        if (!r) { empty++; if (empty >= 3) break; await delay(1000); continue; }
        if (r._offsetLimit) break;
        empty = 0;
        const items = (r.items || []).map(i => i?.catalogCard).filter(Boolean);
        if (!items.length) break;
        if (firstTotal === null && r.total != null) firstTotal = r.total;
        await this._upsert(items, cat);
        this.collectedCount += items.length; this._c(this.collectedCount);
        offset += CFG.BATCH_SIZE;
        if (this.collectedCount % CFG.SAVE_INTERVAL === 0) {
          await this.db.setState('resume_cat_idx', ci); await this.db.setState('resume_offset', offset); await this.db.setState('collected_count', this.collectedCount);
        }
        if (this.aborted) return;
        await delay(CFG.REQUEST_DELAY_MS);
      }
      if (firstTotal) totals[cat.id] = { total: firstTotal, offset: Math.min(offset, firstTotal) };
      // Save totals incrementally so partial runs still help
      await this.db.setState('cat_totals', totals);
    }
    await this.db.deleteState('resume_cat_idx'); await this.db.deleteState('resume_offset'); await this.db.setState('collected_count', this.collectedCount);
  }

  async _collectBySearch() {
    let offset = await this.db.getState('resume_offset', 0);
    // If saved resume offset is past the limit, reset
    if (offset >= CFG.OFFSET_LIMIT) offset = 0;
    let empty = 0;
    while (!this.aborted) {
      if (offset >= CFG.OFFSET_LIMIT) {
        Log.warn('Search: reached offset limit, stopping');
        break;
      }
      const r = await this.api.searchProducts({ offset });
      if (!r) { empty++; if (empty >= 3) break; await delay(2000); continue; }
      if (r._offsetLimit) break;
      empty = 0;
      const items = (r.items || []).map(i => i?.catalogCard).filter(Boolean);
      if (!items.length) break;
      await this._upsert(items); this.collectedCount += items.length; this._c(this.collectedCount);
      offset += CFG.BATCH_SIZE; await delay(CFG.REQUEST_DELAY_MS);
    }
    await this.db.deleteState('resume_offset');
  }

  async _upsert(cards, cat) {
    const now = new Date().toISOString(); const cn = cat?.title || null; const cid = cat?.id || null;
    const existing = await this.db.getProductsByIds(cards.map(c => c.productId));
    const batch = cards.map(card => {
      const old = existing.get(String(card.productId));
      if (old) {
        return {
          id: card.productId, title: card.title || old.title,
          price: card.minSellPrice, oldPrice: card.minFullPrice,
          discountPercent: card.minFullPrice && card.minSellPrice ? Math.round((1 - card.minSellPrice / card.minFullPrice) * 100) : 0,
          rating: card.rating || old.rating, reviewCount: card.feedbackQuantity || old.reviewCount,
          category: old.category || cn, categoryId: old.categoryId || cid,
          firstSeen: old.firstSeen, lastSeen: now,
        };
      }
      return {
        id: card.productId, title: card.title || '',
        price: card.minSellPrice, oldPrice: card.minFullPrice,
        discountPercent: card.minFullPrice && card.minSellPrice ? Math.round((1 - card.minSellPrice / card.minFullPrice) * 100) : 0,
        rating: card.rating || null, reviewCount: card.feedbackQuantity || 0,
        category: cn, categoryId: cid,
        firstSeen: now, lastSeen: now,
      };
    });
    await this.db.putProducts(batch);
  }
}

/* ===================================================================
   UI
   =================================================================== */
function createUI(db, collector) {
  GM_addStyle(`
    #uz-panel{position:fixed;top:80px;right:16px;z-index:999999;width:320px;background:#1a1a2e;border:1px solid #333;border-radius:10px;padding:14px 16px;font:13px -apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;color:#e0e0e0;box-shadow:0 4px 24px rgba(0,0,0,.5);user-select:none}
    #uz-panel h3{margin:0 0 10px 0;font-size:15px;font-weight:700;color:#fff;display:flex;align-items:center;gap:8px}
    #uz-panel h3 span{color:#6c63ff}
    #uz-panel .r{display:flex;align-items:center;justify-content:space-between;margin-bottom:8px}
    #uz-panel .l{color:#999}
    #uz-panel .v{font-weight:600;color:#fff}
    #uz-panel button{flex:1;padding:7px 0;border:none;border-radius:6px;font-size:13px;font-weight:600;cursor:pointer;transition:opacity .2s}
    #uz-panel button:disabled{opacity:.4;cursor:not-allowed}
    #uz-panel .bs{background:#2ecc71;color:#fff;margin-right:6px}
    #uz-panel .bs:hover:not(:disabled){background:#27ae60}
    #uz-panel .bp{background:#e74c3c;color:#fff;margin-left:6px}
    #uz-panel .bp:hover:not(:disabled){background:#c0392b}
    #uz-panel .be{background:#3498db;color:#fff}
    #uz-panel .be:hover:not(:disabled){background:#2980b9}
    #uz-panel .brf{background:#f39c12;color:#fff}
    #uz-panel .brf:hover:not(:disabled){background:#e67e22}
    #uz-panel .br{display:flex;gap:6px;margin-top:8px}
    #uz-panel .log{margin-top:8px;max-height:120px;overflow-y:auto;background:rgba(0,0,0,.3);border-radius:4px;padding:6px 8px;font:11px 'SF Mono',Monaco,'Cascadia Code',monospace;color:#aaa;line-height:1.5}
    #uz-panel .log::-webkit-scrollbar{width:4px}
    #uz-panel .log::-webkit-scrollbar-thumb{background:#444;border-radius:2px}
  `);
  const p = document.createElement('div'); p.id = 'uz-panel';
  p.innerHTML = `<h3>📦 <span>Uzum</span> Collector v2.6</h3><div class="r"><span class="l">Status</span><span class="v" id="uz-s">Idle</span></div><div class="r"><span class="l">Collected</span><span class="v" id="uz-c">0</span></div><div class="r"><button class="bs" id="uz-go">▶ Start</button><button class="bp" id="uz-st" disabled>■ Stop</button></div><div class="br"><button class="be" id="uz-ex">Export JSON</button><button class="brf" id="uz-rf">↻ Refresh</button></div><div class="log" id="uz-log"></div>`;
  document.body.appendChild(p);
  const s = p.querySelector('#uz-s'), c = p.querySelector('#uz-c'), go = p.querySelector('#uz-go'), st = p.querySelector('#uz-st'), ex = p.querySelector('#uz-ex'), rf = p.querySelector('#uz-rf'), lg = p.querySelector('#uz-log');
  Log.init(lg);
  let drag = false, ox, oy;
  p.querySelector('h3').style.cursor = 'grab';
  p.querySelector('h3').addEventListener('mousedown', e => { drag = true; ox = e.clientX - p.offsetLeft; oy = e.clientY - p.offsetTop; });
  document.addEventListener('mousemove', e => { if (!drag) return; p.style.left = (e.clientX - ox) + 'px'; p.style.right = 'auto'; p.style.top = (e.clientY - oy) + 'px'; });
  document.addEventListener('mouseup', () => { drag = false; });
  go.addEventListener('click', async () => {
    go.disabled = true; st.disabled = false; s.textContent = 'Starting...';
    collector.setUI(s, c);
    const ok = await collector.start();
    if (!ok && !collector.aborted) go.disabled = false;
    if (!collector.running) { st.disabled = true; }
  });
  st.addEventListener('click', () => { collector.stop(); go.disabled = false; st.disabled = true; s.textContent = 'Stopped'; db.setState('status', 'stopped'); });
  ex.addEventListener('click', async () => {
    ex.disabled = true;
    try {
      const data = await db.exportAll();
      const blob = new Blob([data], { type: 'application/jsonl' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a'); a.href = url; a.download = `uzum_${new Date().toISOString().slice(0,10)}.jsonl`;
      document.body.appendChild(a); a.click(); document.body.removeChild(a); URL.revokeObjectURL(url);
    } catch (e) { Log.error('Export: ' + e.message); }
    ex.disabled = false;
  });
  rf.addEventListener('click', async () => {
    if (collector.running) return;
    rf.disabled = true; go.disabled = true; st.disabled = false;
    collector.setUI(s, c);
    await collector.refresh();
    rf.disabled = false; st.disabled = true;
    if (!collector.running) go.disabled = false;
  });
  GM_registerMenuCommand('▶ Start', () => go.click()); GM_registerMenuCommand('■ Stop', () => st.click()); GM_registerMenuCommand('⬇ Export', () => ex.click());
  return { go, st, s, c };
}

function delay(ms) { return new Promise(r => setTimeout(r, ms)); }

/* ===================================================================
   INIT
   =================================================================== */
(async function () {
  'use strict';
  Log.info('Uzum Collector v2.6 initializing...');
  const db = new ProductDB();
  try { await db.open(); Log.info('IndexedDB ready'); } catch (e) { Log.error('DB: ' + e.message); return; }
  const api = new UzumApi();
  const collector = new ProductCollector(db, api);
  await db.slimAll();
  const ui = createUI(db, collector);
  const count = await db.getProductCount();
  ui.c.textContent = String(count);
  const saved = await db.getState('status', 'idle');
  if (saved === 'collection_done') ui.s.textContent = 'Collection done';
  else if (saved === 'stopped') ui.s.textContent = 'Stopped (data OK)';
  Log.info(`Ready. ${count} products.`);
})();
