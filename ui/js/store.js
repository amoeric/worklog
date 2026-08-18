/* 每日工作日誌 — 前端資料層
   資料全部來自 Rust 後端：它讀設定資料夾裡的 <民國7碼>.md，解析後一次送過來。
   這個 app 對日誌檔幾乎只讀不寫：唯一會動到 .md 的是 appendEntry()，
   而且只會往今天的檔案插一行，不改寫既有內容。TODO 存在 app 自己的設定目錄。

   因為要等後端，每一頁的畫面程式都要包在 Log.ready(function () { ... }) 裡。 */
(function () {
  var invoke = (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) || null;

  /* ---------- 民國日期 ---------- */
  var WEEKDAYS = ['日', '一', '二', '三', '四', '五', '六'];

  function pad(n, w) {
    var s = String(n);
    while (s.length < w) s = '0' + s;
    return s;
  }

  function toCode(date) {
    return String(date.getFullYear() - 1911) + pad(date.getMonth() + 1, 2) + pad(date.getDate(), 2);
  }

  function toDate(code) {
    var s = String(code);
    return new Date(Number(s.slice(0, 3)) + 1911, Number(s.slice(3, 5)) - 1, Number(s.slice(5, 7)));
  }

  function parts(code) {
    var s = String(code);
    return { roc: Number(s.slice(0, 3)), month: Number(s.slice(3, 5)), day: Number(s.slice(5, 7)) };
  }

  function longLabel(code) {
    var p = parts(code);
    return '民國' + p.roc + '年' + p.month + '月' + p.day + '日 星期' + WEEKDAYS[toDate(code).getDay()];
  }

  function shortLabel(code) {
    var p = parts(code);
    return p.month + '月' + p.day + '日（' + WEEKDAYS[toDate(code).getDay()] + '）';
  }

  function fileName(code) {
    return code + '.md';
  }

  function shift(code, days) {
    var d = toDate(code);
    d.setDate(d.getDate() + days);
    return toCode(d);
  }

  function daysBetween(from, to) {
    return Math.round((toDate(to) - toDate(from)) / 86400000);
  }

  /* ---------- 後端送過來的資料 ---------- */
  var WS = { folder: '', folder_exists: false, today: toCode(new Date()), days: [], items: [], projects: [], skipped: [], pending: [] };
  var STATUSES = [];
  var DAYS = {};          /* code -> entries */
  var TODOS = [];
  var readyQueue = [];
  var booted = false;

  function today() {
    return WS.today;
  }

  function stayLabel(code) {
    var n = daysBetween(code, today());
    if (n <= 0) return '今天進入';
    if (n === 1) return '停了 1 天';
    return '停了 ' + n + ' 天';
  }

  /* ---------- 狀態：介面顯示英文，日誌檔裡寫的是中文標籤 ---------- */
  function statusById(id) {
    for (var i = 0; i < STATUSES.length; i++) if (STATUSES[i].id === id) return STATUSES[i];
    return null;
  }

  function statusByZh(zh) {
    for (var i = 0; i < STATUSES.length; i++) if (STATUSES[i].zh === zh) return STATUSES[i];
    return null;
  }

  function statuses() {
    return STATUSES.slice();
  }

  function lifecycle() {
    return STATUSES.filter(function (s) { return s.lifecycle; });
  }

  /* 事件底色：淡色底、深色字；Archived 是終點，文字退成灰的 */
  function statusClass(id) {
    if (!statusById(id)) return 'border-transparent bg-accentsoft text-muted dark:bg-daccentsoft dark:text-dmuted';
    var text = id === 'archived' ? 'text-muted dark:text-dmuted' : 'text-ink dark:text-dink';
    return 'border-transparent bg-st-' + id + '-tint ' + text + ' dark:bg-st-' + id + '-dtint';
  }

  /* 一整組後面墊的底，比事件底再淡一階 */
  function groupClass(id) {
    if (!statusById(id)) return 'bg-accentsoft/50 dark:bg-daccentsoft/60';
    return 'bg-st-' + id + '-tint/50 dark:bg-st-' + id + '-dtint/60';
  }

  /* 行事曆事件左邊那顆實心圓點 */
  function statusDotClass(id) {
    if (!statusById(id)) return 'bg-muted dark:bg-dmuted';
    return 'bg-st-' + id + '-dot';
  }

  /* 行事曆裡的一列事件：圓點 + 名稱 + 數字 */
  function eventRow(id, label, count) {
    var dot = id ? 'bg-st-' + id + '-dot' : 'bg-muted dark:bg-dmuted';
    var box = id
      ? statusClass(id)
      : 'border-transparent bg-accentsoft text-muted dark:bg-daccentsoft dark:text-dmuted';
    return '<span class="flex items-center gap-1.5 rounded px-1.5 py-0.5 leading-tight ' + box + '">' +
      '<span class="h-2 w-2 shrink-0 rounded-full ' + dot + '"></span>' +
      '<span class="min-w-0 flex-1 truncate">' + label + '</span>' +
      '<span class="shrink-0 tabular-nums">' + count + '</span>' +
    '</span>';
  }

  function statusLabel(id) {
    var s = statusById(id);
    return s ? s.label : '';
  }

  /* ---------- 連結判斷 ---------- */
  function detectKind(url) {
    if (!url) return null;
    var u = String(url).trim();
    if (!u) return null;
    if (/merge_requests\/\d*/i.test(u)) return 'mr';
    if (/\/issues\/\d*/i.test(u) || /redmine/i.test(u)) return 'issue';
    return 'link';
  }

  /* 這個連結是哪個系統來的。
     GitLab 的網址帶 /-/（例如 /group/project_a/-/merge_requests/64），
     Redmine 的議題則是根目錄下的 /issues/123。 */
  function linkSource(url) {
    if (!url) return null;
    var u = String(url);
    if (/\/-\/(merge_requests|issues)\//i.test(u) || /merge_requests\/\d/i.test(u)) return 'gitlab';
    if (/redmine/i.test(u)) return 'redmine';
    if (/\/issues\/\d+/i.test(u)) return 'redmine';
    return null;
  }

  /* 標籤要看得出來源：GitLab MR / GitLab 議題 / Redmine 議題 */
  function linkLabel(url) {
    var kind = detectKind(url);
    var src = linkSource(url);
    if (kind === 'mr') return src === 'gitlab' ? 'GitLab MR' : 'MR';
    if (kind === 'issue') {
      if (src === 'redmine') return 'Redmine 議題';
      if (src === 'gitlab') return 'GitLab 議題';
      return '議題';
    }
    return kindLabel(kind);
  }

  function kindLabel(kind) {
    if (kind === 'mr') return 'MR';
    if (kind === 'issue') return '議題';
    if (kind === 'link') return '連結';
    return '';
  }

  /* 連結一律丟給系統瀏覽器開，不要在 app 視窗裡導航掉 */
  function openExternal(url) {
    if (!url) return;
    if (invoke) invoke('open_url', { url: url });
    else window.open(url, '_blank');
  }

  /* 抓 MR／議題的內容，讓連結可以在 app 裡看，不用跳去瀏覽器。
     token 存在後端，前端只拿得到整理好的內容。抓不到時 reject 的訊息就是要顯示的原因。 */
  function fetchLink(url) {
    if (!invoke) return Promise.reject(new Error('不在 app 裡，抓不到連結內容'));
    return invoke('fetch_link', { url: url });
  }

  /* 抓描述／留言裡的一張圖。附圖多半要帶 token，WebView 自己去 <img src> 多半 401，
     所以交給後端抓回來變成 data: URI。pageUrl 是那張圖所屬的議題／MR 連結，
     後端要靠它決定打哪一台、以及把相對路徑補成絕對網址。 */
  function fetchImage(pageUrl, src) {
    if (!invoke) return Promise.reject(new Error('不在 app 裡，抓不到圖片'));
    return invoke('fetch_image', { pageUrl: pageUrl, src: src });
  }

  function openLogFile(code) {
    if (invoke) return invoke('open_log_file', { code: code });
    return Promise.resolve();
  }

  /* ---------- 日誌 ---------- */
  function entriesOf(code) {
    return DAYS[code] ? DAYS[code].slice() : [];
  }

  function projects() {
    var list = WS.projects.slice();
    TODOS.forEach(function (t) {
      if (t.project && list.indexOf(t.project) === -1) list.push(t.project);
    });
    list = list.filter(function (p) { return p !== '其他'; });
    list.push('其他');
    return list;
  }

  /* 依專案分區 */
  function grouped(code) {
    var entries = entriesOf(code);
    var order = projects();
    var buckets = {};
    entries.forEach(function (e, i) {
      if (!buckets[e.project]) buckets[e.project] = [];
      buckets[e.project].push({ item: e, index: i });
    });
    var out = [];
    order.forEach(function (p) {
      if (buckets[p]) { out.push({ project: p, items: buckets[p] }); delete buckets[p]; }
    });
    Object.keys(buckets).forEach(function (p) { out.push({ project: p, items: buckets[p] }); });
    return out;
  }

  function countOf(code) {
    return entriesOf(code).length;
  }

  /* 這一天各狀態各幾筆，月曆格子用；沒標狀態的算成 none */
  function dayStatusCounts(code) {
    var counts = {};
    entriesOf(code).forEach(function (e) {
      var key = e.status || 'none';
      counts[key] = (counts[key] || 0) + 1;
    });
    return counts;
  }

  function hasDay(code) {
    return !!(DAYS[code] && DAYS[code].length);
  }

  function allCodes() {
    return Object.keys(DAYS).sort(function (a, b) { return Number(a) - Number(b); });
  }

  function recentCodes(limit) {
    var codes = allCodes().reverse();
    return limit ? codes.slice(0, limit) : codes;
  }

  /* 最靠近今天、而且有紀錄的一天；完全沒資料就回今天 */
  function latestCode() {
    var codes = allCodes();
    if (!codes.length) return today();
    var t = today();
    var best = codes[0];
    for (var i = 0; i < codes.length; i++) if (codes[i] <= t) best = codes[i];
    return codes.indexOf(t) !== -1 ? t : best;
  }

  /* ---------- 工作項目（後端已經推導好狀態與歷程） ---------- */
  function allItems() {
    return WS.items.slice();
  }

  function itemById(id) {
    for (var i = 0; i < WS.items.length; i++) if (WS.items[i].id === id) return WS.items[i];
    return null;
  }

  function itemsByStatus(statusId) {
    return WS.items.filter(function (it) { return it.status === statusId; });
  }

  function statusCounts() {
    var counts = {};
    STATUSES.forEach(function (s) { counts[s.id] = 0; });
    WS.items.forEach(function (it) { counts[it.status] = (counts[it.status] || 0) + 1; });
    return counts;
  }

  function historyOf(itemId) {
    var it = itemById(itemId);
    return it ? it.history.slice() : [];
  }

  /* ---------- 待寫回的變更 ----------
     看板上拖卡片不會動到 .md，只會記一筆變更，等使用者複製給 Claude 寫進當天的日誌。
     後端已經把這些變更併進當天條目，所以狀態立刻就是新的。 */
  function pendingEntries() {
    return (WS.pending || []).slice();
  }

  function pendingCount() {
    return (WS.pending || []).length;
  }

  function setWorkspace(ws) {
    WS = ws;
    DAYS = {};
    (ws.days || []).forEach(function (d) { DAYS[d.code] = d.entries; });
  }

  function moveItem(itemId, statusId) {
    if (!invoke) return Promise.resolve();
    return invoke('move_item', { itemId: itemId, statusId: statusId }).then(setWorkspace);
  }

  function clearPending() {
    if (!invoke) return Promise.resolve();
    return invoke('clear_pending').then(setWorkspace);
  }

  /* 一筆條目寫成日誌檔裡的那一行 */
  function entryMarkdown(entry) {
    var st = entry.status ? statusById(entry.status) : null;
    var tag = st ? '`' + st.zh + '` ' : '';
    var body = entry.url ? '[' + entry.title + '](' + entry.url + ')' : entry.title;
    return '- ' + tag + body;
  }

  /* ---------- TODO：這個 app 唯一可寫的東西 ---------- */
  function todos() {
    return TODOS.slice();
  }

  function persistTodos() {
    if (invoke) return invoke('save_todos', { todos: TODOS });
    return Promise.resolve();
  }

  function addTodo(todo) {
    TODOS.unshift({
      id: 't' + Date.now(),
      text: todo.text,
      project: todo.project || '',
      url: todo.url || '',
      status: todo.status || '',
      done: false,
    });
    return persistTodos();
  }

  /* 就地編輯：只覆蓋有傳進來的欄位，沒傳的維持原樣。
     資料流跟 addTodo 一樣：先改記憶體，再整包交給後端存檔。 */
  function updateTodo(id, fields) {
    var f = fields || {};
    TODOS.forEach(function (t) {
      if (t.id !== id) return;
      if (f.text !== undefined) t.text = f.text;
      if (f.project !== undefined) t.project = f.project || '';
      if (f.url !== undefined) t.url = f.url || '';
      if (f.status !== undefined) t.status = f.status || '';
      if (f.done !== undefined) t.done = !!f.done;
    });
    return persistTodos();
  }

  function toggleTodo(id) {
    TODOS.forEach(function (t) { if (t.id === id) t.done = !t.done; });
    return persistTodos();
  }

  /* 寫進日誌之後只標成完成，不刪掉，使用者還看得到自己做了什麼 */
  function markTodoDone(id) {
    TODOS.forEach(function (t) { if (t.id === id) t.done = true; });
    return persistTodos();
  }

  function removeTodo(id) {
    TODOS = TODOS.filter(function (t) { return t.id !== id; });
    return persistTodos();
  }

  /* 轉成日誌檔那一行的寫法，方便貼給 Claude */
  function todoMarkdown(t) {
    var st = t.status ? statusById(t.status) : null;
    var tag = st ? '`' + st.zh + '` ' : '';
    var body = t.url ? '[' + t.text + '](' + t.url + ')' : t.text;
    return '- ' + tag + body;
  }

  /* 把一筆 TODO 真的寫進今天的 <民國7碼>.md。
     後端只會在對應的 `## 專案` 區塊插一行；一模一樣的行不會重複寫，
     這種情況回傳的 duplicate 是 true。 */
  function appendEntry(todo) {
    if (!invoke) return Promise.reject(new Error('不在 app 裡，寫不了日誌檔'));
    return invoke('append_entry', {
      project: todo.project || '其他',
      status: todo.status || '',
      text: todo.text || '',
      url: todo.url || '',
    });
  }

  /* ---------- 設定 ---------- */
  function workspace() {
    return WS;
  }

  function loadSettings() {
    return invoke ? invoke('load_settings') : Promise.resolve({ folder: '', folder_exists: false, default_folder: '', config_dir: '' });
  }

  function saveSettings(folder) {
    return invoke ? invoke('save_settings', { folder: folder }) : Promise.resolve(null);
  }

  function pickFolder() {
    return invoke ? invoke('pick_folder') : Promise.resolve(null);
  }

  /* 外部服務的位址與 token。token 傳空字串代表「不要動原本那把」。 */
  function saveExternalSettings(v) {
    if (!invoke) return Promise.resolve(null);
    return invoke('save_external_settings', {
      gitlabBase: v.gitlab_base || '',
      gitlabToken: v.gitlab_token || '',
      redmineBase: v.redmine_base || '',
      redmineToken: v.redmine_token || '',
    });
  }

  function clearToken(service) {
    return invoke ? invoke('clear_token', { service: service }) : Promise.resolve(null);
  }

  /* 日誌規則：`~/.claude/CLAUDE.md` 裡「# 每日工作日誌」那一段。
     這份是給 Claude Code 看的，app 自己的解析規則在 parser.rs，不受它影響。
     saveRules 會真的去改使用者的全域設定檔（後端寫之前會先備份）。 */
  function loadRules() {
    return invoke ? invoke('load_rules') : Promise.resolve(null);
  }

  function saveRules(text) {
    return invoke ? invoke('save_rules', { text: text }) : Promise.resolve(null);
  }

  /* ---------- 開機 ---------- */
  function ingest(ws, table, todoList) {
    STATUSES = table;
    TODOS = todoList || [];
    setWorkspace(ws);
  }

  function boot() {
    if (!invoke) {
      /* 不在 Tauri 裡（例如直接用瀏覽器開）就顯示空狀態，不要整頁壞掉 */
      booted = true;
      flush();
      return;
    }
    Promise.all([
      invoke('load_workspace'),
      invoke('status_table'),
      invoke('load_todos'),
    ]).then(function (r) {
      ingest(r[0], r[1], r[2]);
      booted = true;
      flush();
    }).catch(function (e) {
      console.error('載入失敗', e);
      booted = true;
      flush();
    });
  }

  function flush() {
    var q = readyQueue.slice();
    readyQueue = [];
    q.forEach(function (fn) {
      try { fn(); } catch (e) { console.error(e); }
    });
  }

  function ready(fn) {
    if (booted) fn();
    else readyQueue.push(fn);
  }

  /* 改了設定之後重讀，不用重開 app */
  function reload() {
    if (!invoke) return Promise.resolve();
    return Promise.all([
      invoke('load_workspace'),
      invoke('status_table'),
      invoke('load_todos'),
    ]).then(function (r) { ingest(r[0], r[1], r[2]); });
  }

  window.Log = {
    WEEKDAYS: WEEKDAYS,
    statuses: statuses,
    lifecycle: lifecycle,
    pad: pad,
    toCode: toCode,
    toDate: toDate,
    parts: parts,
    longLabel: longLabel,
    shortLabel: shortLabel,
    fileName: fileName,
    shift: shift,
    today: today,
    daysBetween: daysBetween,
    stayLabel: stayLabel,
    statusById: statusById,
    statusByZh: statusByZh,
    statusLabel: statusLabel,
    statusClass: statusClass,
    groupClass: groupClass,
    eventRow: eventRow,
    statusDotClass: statusDotClass,
    detectKind: detectKind,
    kindLabel: kindLabel,
    linkSource: linkSource,
    linkLabel: linkLabel,
    openExternal: openExternal,
    fetchLink: fetchLink,
    fetchImage: fetchImage,
    openLogFile: openLogFile,
    projects: projects,
    entriesOf: entriesOf,
    itemsOf: entriesOf,
    grouped: grouped,
    countOf: countOf,
    dayStatusCounts: dayStatusCounts,
    pendingEntries: pendingEntries,
    pendingCount: pendingCount,
    clearPending: clearPending,
    moveItem: moveItem,
    entryMarkdown: entryMarkdown,
    hasDay: hasDay,
    allCodes: allCodes,
    recentCodes: recentCodes,
    latestCode: latestCode,
    allItems: allItems,
    itemById: itemById,
    itemsByStatus: itemsByStatus,
    statusCounts: statusCounts,
    historyOf: historyOf,
    todos: todos,
    addTodo: addTodo,
    updateTodo: updateTodo,
    toggleTodo: toggleTodo,
    markTodoDone: markTodoDone,
    removeTodo: removeTodo,
    todoMarkdown: todoMarkdown,
    appendEntry: appendEntry,
    workspace: workspace,
    loadSettings: loadSettings,
    saveSettings: saveSettings,
    pickFolder: pickFolder,
    saveExternalSettings: saveExternalSettings,
    clearToken: clearToken,
    loadRules: loadRules,
    saveRules: saveRules,
    reload: reload,
    ready: ready,
  };

  boot();
})();
