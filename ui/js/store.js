/* 每日工作日誌 — 前端資料層
   資料全部來自 Rust 後端：它讀設定資料夾裡的 <年>/<月>/<西元8碼>.md，解析後一次送過來。
   會動到 .md 的只有兩件事：TODO 的 appendEntry()（往今天的檔案插一行），
   以及看板改狀態的 moveItem()（改今天那一行的狀態標籤，沒有就插一行）。
   TODO 存在 app 自己的設定目錄。

   因為要等後端，每一頁的畫面程式都要包在 Log.ready(function () { ... }) 裡。 */
(function () {
  var invoke = (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) || null;
  var listen = (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen) || null;

  /* ---------- 西元日期 ---------- */
  var WEEKDAYS = ['日', '一', '二', '三', '四', '五', '六'];

  function pad(n, w) {
    var s = String(n);
    while (s.length < w) s = '0' + s;
    return s;
  }

  function toCode(date) {
    return String(date.getFullYear()) + pad(date.getMonth() + 1, 2) + pad(date.getDate(), 2);
  }

  function toDate(code) {
    var s = String(code);
    return new Date(Number(s.slice(0, 4)), Number(s.slice(4, 6)) - 1, Number(s.slice(6, 8)));
  }

  function parts(code) {
    var s = String(code);
    return { year: Number(s.slice(0, 4)), month: Number(s.slice(4, 6)), day: Number(s.slice(6, 8)) };
  }

  function longLabel(code) {
    var p = parts(code);
    return p.year + '年' + p.month + '月' + p.day + '日 星期' + WEEKDAYS[toDate(code).getDay()];
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
  var WS = { folder: '', folder_exists: false, today: toCode(new Date()), days: [], items: [], projects: [], skipped: [] };
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

  /* 事件底色：淡色底、深色字；Archived 是終點，文字退成灰的 */
  function statusClass(id) {
    var s = statusById(id);
    if (!s) return 'border-transparent bg-accentsoft text-muted dark:bg-daccentsoft dark:text-dmuted';
    var text = id === 'archived' ? 'text-muted dark:text-dmuted' : 'text-ink dark:text-dink';
    if (s.color) return 'border-transparent stx-tint-' + id + ' ' + text;
    return 'border-transparent bg-st-' + id + '-tint ' + text + ' dark:bg-st-' + id + '-dtint';
  }

  /* 一整組後面墊的底，比事件底再淡一階 */
  function groupClass(id) {
    var s = statusById(id);
    if (!s) return 'bg-accentsoft/50 dark:bg-daccentsoft/60';
    if (s.color) return 'stx-group-' + id;
    return 'bg-st-' + id + '-tint/50 dark:bg-st-' + id + '-dtint/60';
  }

  /* 行事曆事件左邊那顆實心圓點 */
  function statusDotClass(id) {
    var s = statusById(id);
    if (!s) return 'bg-muted dark:bg-dmuted';
    if (s.color) return 'stx-dot-' + id;
    return 'bg-st-' + id + '-dot';
  }

  /* 行事曆裡的一列事件：圓點 + 名稱 + 數字 */
  function eventRow(id, label, count) {
    var dot = id ? statusDotClass(id) : 'bg-muted dark:bg-dmuted';
    var box = id
      ? statusClass(id)
      : 'border-transparent bg-accentsoft text-muted dark:bg-daccentsoft dark:text-dmuted';
    return '<span class="flex items-center gap-1.5 rounded px-1.5 py-0.5 leading-tight ' + box + '">' +
      '<span class="h-2 w-2 shrink-0 rounded-full ' + dot + '"></span>' +
      '<span class="min-w-0 flex-1 truncate">' + label + '</span>' +
      '<span class="shrink-0 tabular-nums">' + count + '</span>' +
    '</span>';
  }

  /* 畫面上一律顯示中文——日誌檔裡寫的就是中文，兩邊對得起來。
     英文（Todo/Parked…）只留在 id 與程式碼裡。 */
  function statusText(id) {
    var s = statusById(id);
    return s ? s.zh : '';
  }

  function statusLabel(id) {
    return statusText(id);
  }

  /* ---------- 搜尋比對 ----------
     工具列的搜尋框只負責關鍵字，實際要濾什麼欄位由各頁自己決定，
     但比對規則三頁一致，所以放在這裡：
       - 一律不分大小寫
       - 關鍵字用空白切成多個詞，**每個詞都要命中**（AND）
       - 空字串或只有空白＝不過濾，全部留著
     中文沒有大小寫，toLowerCase() 對它是原樣返回，不影響比對。 */
  function searchTerms(q) {
    return String(q == null ? '' : q).toLowerCase().split(/\s+/).filter(function (t) { return t; });
  }

  /* fields 是這一筆可以被搜到的所有文字（標題、專案、狀態標籤、網址…），
     null／undefined／空字串會被忽略，不用在呼叫端先過濾一次。 */
  function matchesSearch(fields, q) {
    var terms = searchTerms(q);
    if (!terms.length) return true;
    var hay = (fields || []).filter(function (v) {
      return v !== null && v !== undefined && v !== false && v !== '';
    }).join(' ').toLowerCase();
    return terms.every(function (t) { return hay.indexOf(t) >= 0; });
  }

  /* 一筆日誌條目的可搜文字：標題、專案、狀態的中英文標籤、連結網址 */
  function entrySearchFields(e) {
    var st = e && e.status ? statusById(e.status) : null;
    return [e.title, e.project, st ? st.label : '', st ? st.zh : '', e.url];
  }

  /* 一支工作項目的可搜文字：標題、專案、狀態標籤、議題／MR 網址、項目 id */
  function itemSearchFields(it) {
    var st = it && it.status ? statusById(it.status) : null;
    return [it.title, it.project, st ? st.label : '', st ? st.zh : '', it.issue, it.mr, it.id];
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

  /* 圓形來源圖示：GitLab 狐狸、Redmine 的 R。
     圖示本身不帶字，完整名稱（GitLab MR／Redmine 議題）放 title，滑上去才看得到。
     認不出來源的連結退回文字標籤。 */
  var GITLAB_SVG = '<svg class="h-4 w-4" viewBox="0 0 1000 963.197"><g transform="matrix(5.2068817,0,0,5.2068817,-489.30756,-507.76085)"><path fill="#e24329" d="m 282.83,170.73 -0.27,-0.69 -26.14,-68.22 a 6.81,6.81 0 0 0 -2.69,-3.24 7,7 0 0 0 -8,0.43 7,7 0 0 0 -2.32,3.52 l -17.65,54 h -71.47 l -17.65,-54 a 6.86,6.86 0 0 0 -2.32,-3.53 7,7 0 0 0 -8,-0.43 6.87,6.87 0 0 0 -2.69,3.24 L 97.44,170 l -0.26,0.69 a 48.54,48.54 0 0 0 16.1,56.1 l 0.09,0.07 0.24,0.17 39.82,29.82 19.7,14.91 12,9.06 a 8.07,8.07 0 0 0 9.76,0 l 12,-9.06 19.7,-14.91 40.06,-30 0.1,-0.08 a 48.56,48.56 0 0 0 16.08,-56.04 z"/><path fill="#fc6d26" d="m 282.83,170.73 -0.27,-0.69 a 88.3,88.3 0 0 0 -35.15,15.8 L 190,229.25 c 19.55,14.79 36.57,27.64 36.57,27.64 l 40.06,-30 0.1,-0.08 a 48.56,48.56 0 0 0 16.1,-56.08 z"/><path fill="#fca326" d="m 153.43,256.89 19.7,14.91 12,9.06 a 8.07,8.07 0 0 0 9.76,0 l 12,-9.06 19.7,-14.91 c 0,0 -17.04,-12.89 -36.59,-27.64 -19.55,14.75 -36.57,27.64 -36.57,27.64 z"/><path fill="#fc6d26" d="M 132.58,185.84 A 88.19,88.19 0 0 0 97.44,170 l -0.26,0.69 a 48.54,48.54 0 0 0 16.1,56.1 l 0.09,0.07 0.24,0.17 39.82,29.82 c 0,0 17,-12.85 36.57,-27.64 z"/></g></svg>';
  var REDMINE_SVG = '<svg class="h-3.5 w-3.5" viewBox="58 306 121 70"><path fill="#4c4c4c" d="m 59.847143,374.6913 21,0 1.5,-18.00125 -19.50125,-4.49875 -2.99875,22.5 z"/><path fill="#4c4c4c" d="m 63.597143,349.19005 18.75,4.5 4.5,-15.75 -15.75,-8.24875 -7.5,19.49875 z"/><path fill="#4c4c4c" d="m 72.595893,326.69005 15,8.25 12.00125,-8.25 -11.25,-12.75 -15.75125,12.75 z"/><path fill="#4c4c4c" d="m 156.59589,374.6913 -20.99875,0 -1.50125,-18.00125 19.50125,-4.49875 2.99875,22.5 z"/><path fill="#4c4c4c" d="m 152.84589,349.19005 -18.75,4.5 -4.49875,-15.75 15.74875,-8.24875 7.5,19.49875 z"/><path fill="#4c4c4c" d="m 143.84589,326.69005 -15,8.25 -11.99875,-8.25 11.25,-12.75 15.74875,12.75 z"/><path fill="#4c4c4c" d="m 92.097143,311.69005 11.249997,13.5 10.49875,0 9.75125,-13.5 -9.75125,-3 -10.7625,0 -10.986247,3 z"/><path fill="#9c0000" d="m 59.847143,374.6913 21,0 1.5,-18.00125 -19.50125,-4.49875 -2.99875,22.5 z"/><path fill="#b50808" d="m 63.597143,349.19005 18.75,4.5 4.5,-15.75 -15.75,-8.24875 -7.5,19.49875 z"/><path fill="#c61818" d="m 72.595893,326.69005 15,8.25 12.00125,-8.25 -11.25,-12.75 -15.75125,12.75 z"/><path fill="#9c0000" d="m 156.59589,374.6913 -20.99875,0 -1.50125,-18.00125 19.50125,-4.49875 2.99875,22.5 z"/><path fill="#b50808" d="m 152.84589,349.19005 -18.75,4.5 -4.49875,-15.75 15.74875,-8.24875 7.5,19.49875 z"/><path fill="#c61818" d="m 143.84589,326.69005 -15,8.25 -11.99875,-8.25 11.25,-12.75 15.74875,12.75 z"/><path fill="#ce3129" d="m 92.097143,311.69005 11.249997,13.5 10.49875,0 9.75125,-13.5 -9.75125,-3 -10.7625,0 -10.986247,3 z"/></svg>';
  function linkBadge(url, extraClass) {
    var src = linkSource(url);
    var label = linkLabel(url);
    var cls = 'inline-flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-white ring-1 ring-line dark:ring-dline ' + (extraClass || '');
    var t = ' title="' + escapeHtml(label) + '" aria-label="' + escapeHtml(label) + '"';
    if (src === 'gitlab') return '<span class="' + cls + '"' + t + '>' + GITLAB_SVG + '</span>';
    if (src === 'redmine') return '<span class="' + cls + '"' + t + '>' + REDMINE_SVG + '</span>';
    return '<span class="shrink-0 rounded-full bg-accentsoft px-2 py-0.5 leading-tight text-muted dark:bg-daccentsoft dark:text-dmuted ' + (extraClass || '') + '"' + t + '>' + escapeHtml(label) + '</span>';
  }

  function escapeHtml(s) {
    return String(s == null ? '' : s).replace(/[&<>"']/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c];
    });
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

  function setWorkspace(ws) {
    WS = ws;
    DAYS = {};
    (ws.days || []).forEach(function (d) { DAYS[d.code] = d.entries; });
  }

  /* 看板改狀態：後端直接寫進今天的 <年>/<月>/<西元8碼>.md，寫完把重讀的結果一起送回來。
     回傳的物件除了 workspace 還有 code / file / status_zh / updated / created / unchanged，
     呼叫端拿去講「已寫進 1150820.md：測試中」。寫不了（沒設資料夾、沒權限…）會 reject。 */
  function moveItem(itemId, statusId) {
    if (!invoke) return Promise.reject(new Error('不在 app 裡，寫不了日誌檔'));
    return invoke('move_item', { itemId: itemId, statusId: statusId }).then(function (r) {
      if (r && r.workspace) setWorkspace(r.workspace);
      return r;
    });
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

  /* 把一筆 TODO 真的寫進今天的 <年>/<月>/<西元8碼>.md。
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

  /* ---------- 線上更新 ----------
     更新檔在 GitHub Releases，後端接的是 Tauri 官方的 updater。
     這個 app 是 ad-hoc 簽章、沒有 Apple 公證，所以更新有可能被 macOS 擋下來：
     裝不起來的時候後端會回一句中文原因，畫面要照樣給「開啟下載頁」讓使用者自己抓。 */
  var UPDATE_PROGRESS = 'worklog://update-progress';

  /* 目前跑的版本，來自 tauri.conf.json，前端不要自己寫死 */
  function appVersion() {
    return invoke ? invoke('app_version') : Promise.resolve('');
  }

  function checkUpdate() {
    if (!invoke) return Promise.reject(new Error('不在 app 裡，不能檢查更新'));
    return invoke('check_update');
  }

  function installUpdate() {
    if (!invoke) return Promise.reject(new Error('不在 app 裡，不能安裝更新'));
    return invoke('install_update');
  }

  function restartApp() {
    return invoke ? invoke('restart_app') : Promise.resolve();
  }

  /* 下載進度。回傳一個取消訂閱的 function（拿不到事件就回一個空的）。 */
  function onUpdateProgress(fn) {
    if (!listen) return Promise.resolve(function () {});
    return listen(UPDATE_PROGRESS, function (e) { fn(e.payload || {}); });
  }


  /* 自訂狀態的顏色。內建八個的色票寫在 tailwind-config.js，Tailwind 生得出
     bg-st-<id>-dot 那些 class；使用者自己加的狀態不在設定裡，所以改用
     stx-* 這組自備 class，開機與每次改狀態表時重畫一次。 */
  function rgba(hex, a) {
    var h = String(hex).replace('#', '');
    if (h.length !== 6) return hex;
    return 'rgba(' + parseInt(h.slice(0, 2), 16) + ',' + parseInt(h.slice(2, 4), 16) + ',' +
      parseInt(h.slice(4, 6), 16) + ',' + a + ')';
  }

  function applyStatusColors() {
    var css = '';
    STATUSES.forEach(function (s) {
      if (!s.color) return;
      var c = s.color;
      css += '.stx-dot-' + s.id + '{background-color:' + c.dot + '}';
      css += '.stx-tint-' + s.id + '{background-color:' + c.tint + '}';
      css += '.dark .stx-tint-' + s.id + '{background-color:' + c.dtint + '}';
      css += '.stx-group-' + s.id + '{background-color:' + rgba(c.tint, 0.5) + '}';
      css += '.dark .stx-group-' + s.id + '{background-color:' + rgba(c.dtint, 0.6) + '}';
    });
    var el = document.getElementById('stx-colors');
    if (!el) {
      el = document.createElement('style');
      el.id = 'stx-colors';
      document.head.appendChild(el);
    }
    el.textContent = css;
  }

  /* ---------- 自訂狀態 ----------
     新增／刪除都由後端存進 statuses.json，回來的是整張新表（順序＝看板欄序）。*/
  function setStatusTable(table) {
    STATUSES = table || [];
    applyStatusColors();
  }

  function addStatus(zh, hint, afterId) {
    if (!invoke) return Promise.reject('不在 app 裡，沒辦法新增狀態');
    return invoke('add_status', { zh: zh, hint: hint, afterId: afterId }).then(function (table) {
      setStatusTable(table);
      return table;
    });
  }

  function deleteStatus(id) {
    if (!invoke) return Promise.reject('不在 app 裡，沒辦法刪狀態');
    return invoke('delete_status', { id: id }).then(function (table) {
      setStatusTable(table);
      return table;
    });
  }

  /* 改名稱／說明。後端會順手把 ~/.claude/CLAUDE.md 的規則一起改（先備份）。 */
  function updateStatus(id, zh, hint) {
    if (!invoke) return Promise.reject('不在 app 裡，沒辦法改狀態');
    return invoke('update_status', { id: id, zh: zh, hint: hint }).then(function (r) {
      setStatusTable(r.table);
      return r;
    });
  }

  /* 把新狀態插進日誌規則後的完整規則本文。只是預覽，還沒寫檔。 */
  function rulesWithStatus(zh, hint, afterZh) {
    if (!invoke) return Promise.reject('不在 app 裡');
    return invoke('rules_with_status', { zh: zh, hint: hint, afterZh: afterZh });
  }

  /* 真的寫進 ~/.claude/CLAUDE.md（後端會先備份） */
  function saveRules(text) {
    if (!invoke) return Promise.reject('不在 app 裡');
    return invoke('save_rules', { text: text });
  }

  /* ---------- 開機 ---------- */
  function ingest(ws, table, todoList) {
    setStatusTable(table);
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
    statusText: statusText,
    statusClass: statusClass,
    groupClass: groupClass,
    eventRow: eventRow,
    statusDotClass: statusDotClass,
    addStatus: addStatus,
    deleteStatus: deleteStatus,
    updateStatus: updateStatus,
    rulesWithStatus: rulesWithStatus,
    saveRules: saveRules,
    searchTerms: searchTerms,
    matchesSearch: matchesSearch,
    entrySearchFields: entrySearchFields,
    itemSearchFields: itemSearchFields,
    detectKind: detectKind,
    kindLabel: kindLabel,
    linkSource: linkSource,
    linkLabel: linkLabel,
    linkBadge: linkBadge,
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
    moveItem: moveItem,
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
    appVersion: appVersion,
    checkUpdate: checkUpdate,
    installUpdate: installUpdate,
    restartApp: restartApp,
    onUpdateProgress: onUpdateProgress,
    reload: reload,
    ready: ready,
  };

  boot();
})();
