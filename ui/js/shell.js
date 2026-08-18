/* 共用外框：macOS 行事曆樣式的工具列
   左邊是名稱與麵包屑，正中間是分段控制（日／週／月／年 的位置），右邊是動作。 */
(function () {
  var NAV = [
    { href: 'index.html', label: '月曆' },
    { href: 'board.html', label: '看板' },
    { href: 'daily-log.html', label: '日誌' },
    { href: 'todo-list.html', label: 'TODO' },
    { href: 'settings.html', label: '設定' },
  ];

  function currentPage() {
    var f = (location.pathname.split('/').pop() || 'index.html');
    if (!f) f = 'index.html';
    if (f === 'work-item.html') f = 'board.html';
    return f;
  }

  function themeButton() {
    return '' +
      '<button data-theme-toggle class="grid h-9 w-9 shrink-0 place-items-center rounded-full border border-line bg-surface text-muted shadow-sm hover:text-ink dark:border-dline dark:bg-dsurface dark:text-dmuted dark:hover:text-dink" aria-label="切換深淺色">' +
        '<svg data-icon="sun" class="hidden h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>' +
        '<svg data-icon="moon" class="h-5 w-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round"><path d="M20 14.5A8 8 0 019.5 4a8 8 0 1010.5 10.5z"/></svg>' +
      '</button>';
  }

  /* 未寫回 .md 的變更數。資料是後端非同步送來的，先留空位，開機完再填。 */
  function refreshBadge() {
    var slot = document.querySelector('[data-shell-badge]');
    if (!slot || !window.Log || !window.Log.pendingCount) return;
    var n = window.Log.pendingCount();
    slot.innerHTML = n
      ? '<a href="board.html" class="shrink-0 rounded-full px-2.5 py-1 leading-tight ' +
        window.Log.statusClass('parked') + '" title="這些變更還沒寫進 .md">未寫回 ' + n + '</a>'
      : '';
  }

  /* ---------- 連結面板 ----------
     MR 與議題先在 app 裡看內容（後端拿 token 打 API），面板下方才是「在瀏覽器開啟」。
     其餘連結維持原本行為，直接丟給系統瀏覽器。
     放在 shell.js 是因為每一頁都會出現這種連結。 */
  var panelEl = null;

  function closePanel() {
    if (!panelEl) return;
    panelEl.remove();
    panelEl = null;
    document.removeEventListener('keydown', onPanelKey);
  }

  function onPanelKey(e) {
    if (e.key === 'Escape') closePanel();
  }

  /* 一列 metadata，寫法跟設定頁的群組內嵌清單一致 */
  function metaRow(label, value) {
    var d = document.createElement('div');
    d.className = 'flex items-baseline gap-3 border-b border-line px-4 py-2.5 last:border-b-0 dark:border-dline';
    var a = document.createElement('span');
    a.className = 'w-24 shrink-0 text-muted dark:text-dmuted';
    a.textContent = label;
    var b = document.createElement('span');
    b.className = 'min-w-0 flex-1 break-words';
    b.textContent = value;
    d.appendChild(a);
    d.appendChild(b);
    return d;
  }

  /* ---------- 描述的 markdown ----------
     描述來自外部系統（Redmine／GitLab），一律當成不可信任的字串。
     流程固定是「先逐字轉義 & < > " '，再拿轉義後的字串去套 markdown 規則產生標籤」，
     所以原文不可能變成 <script>、<iframe>、on* 屬性；連結只放行 http/https/mailto，
     javascript: 那類一律不生成 <a>，維持純文字。
     只做面板看得到的那幾種語法，不追求完整 CommonMark；認不出來的寫法
     （例如某些 Redmine 專案用的 textile）會以接近純文字的樣子留著，不會被吃掉。 */
  var MD = (function () {
    /* 樣式一律用既有的 Tailwind token（ink / muted / line / accentsoft / surface 與深色版）。
       面板不是文章頁，所以標題刻意不放大。 */
    var C = {
      h: [
        'mt-4 mb-1.5 text-lg font-semibold tracking-tight first:mt-0',
        'mt-4 mb-1.5 text-base font-semibold tracking-tight first:mt-0',
        'mt-3 mb-1 font-semibold tracking-tight first:mt-0',
        'mt-3 mb-1 font-semibold tracking-tight first:mt-0',
        'mt-3 mb-1 font-semibold text-muted first:mt-0 dark:text-dmuted',
        'mt-3 mb-1 font-semibold text-muted first:mt-0 dark:text-dmuted',
      ],
      p: 'my-1.5 leading-relaxed first:mt-0 last:mb-0',
      ul: 'my-1.5 list-disc space-y-0.5 pl-5 first:mt-0 last:mb-0',
      ol: 'my-1.5 list-decimal space-y-0.5 pl-5 first:mt-0 last:mb-0',
      li: 'leading-relaxed',
      quote: 'my-2 border-l-2 border-line pl-3 text-muted first:mt-0 last:mb-0 dark:border-dline dark:text-dmuted',
      hr: 'my-3 border-0 border-t border-line dark:border-dline',
      pre: 'my-2 overflow-x-auto rounded-xl bg-accentsoft px-3 py-2.5 first:mt-0 last:mb-0 dark:bg-daccentsoft',
      precode: 'block whitespace-pre font-mono text-[0.92em] leading-relaxed',
      code: 'rounded bg-accentsoft px-1 py-0.5 font-mono text-[0.92em] dark:bg-daccentsoft',
      link: 'underline underline-offset-4 decoration-line hover:text-accent dark:decoration-dline dark:hover:text-daccent',
      imgtag: 'mr-1 rounded bg-accentsoft px-1 text-muted no-underline dark:bg-daccentsoft dark:text-dmuted',
      imgbox: 'my-1 inline-block max-w-full align-top',
      img: 'max-w-full h-auto rounded-lg border border-line dark:border-dline',
      imgnote: 'ml-1 text-muted dark:text-dmuted',
      del: 'text-muted dark:text-dmuted',
      twrap: 'my-2 overflow-x-auto rounded-xl border border-line first:mt-0 last:mb-0 dark:border-dline',
      table: 'w-full border-collapse text-left',
      th: 'border-b border-line px-2.5 py-1.5 font-semibold dark:border-dline',
      td: 'border-b border-line px-2.5 py-1.5 dark:border-dline',
      tdlast: 'px-2.5 py-1.5',
    };

    /* 這是整份轉換唯一的信任邊界：任何要進 innerHTML 的原文都得先走過這裡 */
    function esc(s) {
      return String(s)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
    }

    /* 只放行 http/https/mailto。傳進來的已經是轉義過的字串，所以不再轉一次。
       引號早就變成 &quot; / &#39;，本來就跳不出 href="…"，但正常網址不會帶引號或角括號，
       帶了就是有人在試，一律不做成連結。認不出來就回 null＝維持純文字。 */
    function safeUrl(u) {
      var t = String(u).trim();
      if (!/^(?:https?:\/\/|mailto:)[^\s]+$/i.test(t)) return null;
      if (/&quot;|&#39;|&lt;|&gt;|`/.test(t)) return null;
      return t;
    }

    /* 圖片來源。這裡**不會**直接變成 src：只是記在 data-img-src 上，
       面板畫完之後交給後端去抓（附圖要帶 token），後端回的 data: URI 才會進 <img>。
       所以這裡放行的範圍比 safeUrl 寬一點（相對路徑也收），但 scheme 一律只認 http/https：
       javascript:、data: 這種帶冒號的寫法直接回 null＝維持純文字。 */
    function imgUrl(u) {
      var t = String(u).trim();
      if (!t) return null;
      if (/&quot;|&#39;|&lt;|&gt;|`|\s/.test(t)) return null;
      if (/^https?:\/\//i.test(t)) return t;
      if (/^[a-z][a-z0-9+.-]*:/i.test(t)) return null;   /* 其他 scheme 一律不收 */
      if (/^\/\//.test(t)) return null;                  /* 沒有 scheme 的 //host/…，認不出主機 */
      return t;                                          /* 站內相對路徑，補全交給後端 */
    }

    /* 粗體、斜體、刪除線。只在「已轉義、且連結與行內程式碼都抽成佔位符」之後跑，
       免得網址裡的 _ 或 * 被當成語法。 */
    function emph(s) {
      return s
        .replace(/~~([\s\S]+?)~~/g, '<del class="' + C.del + '">$1</del>')
        .replace(/\*\*([\s\S]+?)\*\*/g, '<strong class="font-semibold">$1</strong>')
        .replace(/__([\s\S]+?)__/g, '<strong class="font-semibold">$1</strong>')
        .replace(/\*([^\s*][^*]*?)\*/g, '<em>$1</em>')
        .replace(/(^|[\s(（【「])_([^\s_][^_]*?)_(?=$|[\s.,:;!?)）】」])/g, '$1<em>$2</em>');
    }

    /* 行內語法。回傳的字串保證每個字元不是轉義過的原文，就是這裡自己組出來的標籤。 */
    function inline(raw) {
      var slots = [];
      function slot(html) { slots.push(html); return '\u0000' + (slots.length - 1) + '\u0000'; }

      var s = esc(raw);

      /* 行內程式碼先抽走，裡面的 * _ [ ] 才不會被當語法 */
      s = s.replace(/(`+)([\s\S]*?)\1/g, function (m, tick, body) {
        return slot('<code class="' + C.code + '">' + body + '</code>');
      });

      /* 圖片：這裡只放佔位，原始網址記在 data-img-src 上。
         Redmine／GitLab 的附圖多半要帶 token，也常是相對路徑，
         所以面板畫完之後才由 loadPanelImages() 交給後端抓，成功才換成 <img>。 */
      s = s.replace(/!\[([^\]]*)\]\(\s*([^\s)]+)(?:\s+&quot;[^)]*&quot;)?\s*\)/g, function (m, alt, url) {
        var u = imgUrl(url);
        if (!u) return m;
        return slot('<span class="' + C.imgbox + '" data-img-src="' + u + '" data-img-alt="' + alt + '">' +
          '<span class="' + C.imgtag + '">圖片</span>' +
          '<span class="' + C.imgnote + '">' + (alt ? emph(alt) : '載入中…') + '</span></span>');
      });

      /* 連結。點下去由面板的事件代理交給 Log.openExternal()，不會在 app 視窗裡導航 */
      s = s.replace(/\[([^\]]*)\]\(\s*([^\s)]+)(?:\s+&quot;[^)]*&quot;)?\s*\)/g, function (m, text, url) {
        var u = safeUrl(url);
        if (!u) return m;
        return slot('<a href="' + u + '" rel="noopener noreferrer" class="' + C.link + '">' +
          (text ? emph(text) : u) + '</a>');
      });

      s = emph(s);

      /* 把佔位符換回來。連結文字裡可能還包著行內程式碼的佔位符，所以要多跑幾輪。 */
      for (var k = 0; k < 5 && s.indexOf('\u0000') >= 0; k++) {
        s = s.replace(/\u0000(\d+)\u0000/g, function (m, n) { return slots[+n]; });
      }
      return s;
    }

    /* 清單項目：`- ` `* ` `+ ` `1. ` `1) `，縮排決定巢狀層級 */
    function item(line) {
      var m = /^(\s*)([-*+]|\d{1,9}[.)])\s+(\S[\s\S]*)?$/.exec(line);
      if (!m) return null;
      var ordered = /\d/.test(m[2]);
      return {
        indent: m[1].length,
        ordered: ordered,
        start: ordered ? parseInt(m[2], 10) : 1,
        text: m[3] || '',
      };
    }

    /* 一列表格切成欄；`\|` 是跳脫的直線，不算分隔 */
    function cells(line) {
      var s = line.trim().replace(/^\|/, '').replace(/\|\s*$/, '');
      var out = [], cur = '';
      for (var k = 0; k < s.length; k++) {
        if (s.charAt(k) === '\\' && s.charAt(k + 1) === '|') { cur += '|'; k++; continue; }
        if (s.charAt(k) === '|') { out.push(cur); cur = ''; continue; }
        cur += s.charAt(k);
      }
      out.push(cur);
      return out.map(function (c) { return c.trim(); });
    }

    /* GFM 表格的第二列（---|:--:|--:）。認不出來就整段當普通段落，不會壞掉。 */
    function isSep(line) {
      if (!line || line.indexOf('-') < 0 || line.indexOf('|') < 0) return false;
      var cs = cells(line);
      if (cs.length < 2) return false;
      return cs.every(function (c) { return /^:?-+:?$/.test(c); });
    }

    function isTableHead(lines, i) {
      return lines[i].indexOf('|') >= 0 && isSep(lines[i + 1]);
    }

    function table(lines, i) {
      var head = cells(lines[i]);
      var aligns = cells(lines[i + 1]).map(function (c) {
        if (/^:-+:$/.test(c)) return 'text-center';
        if (/:$/.test(c)) return 'text-right';
        return 'text-left';
      });
      i += 2;
      var rows = [];
      while (i < lines.length && lines[i].trim() && lines[i].indexOf('|') >= 0) { rows.push(cells(lines[i])); i++; }

      /* 欄數取最寬的一列，多出來的欄照樣畫出來，免得吃掉內容 */
      var cols = head.length;
      rows.forEach(function (r) { if (r.length > cols) cols = r.length; });

      var html = '<div class="' + C.twrap + '"><table class="' + C.table + '"><thead><tr>', k;
      for (k = 0; k < cols; k++) {
        html += '<th class="' + C.th + ' ' + (aligns[k] || 'text-left') + '">' + inline(head[k] || '') + '</th>';
      }
      html += '</tr></thead><tbody>';
      rows.forEach(function (r, ri) {
        html += '<tr>';
        for (var j = 0; j < cols; j++) {
          html += '<td class="' + (ri === rows.length - 1 ? C.tdlast : C.td) + ' ' +
            (aligns[j] || 'text-left') + '">' + inline(r[j] || '') + '</td>';
        }
        html += '</tr>';
      });
      return { html: html + '</tbody></table></div>', next: i };
    }

    /* 清單。用一個層級堆疊處理巢狀：縮排變深就開新的一層，變淺就收掉。 */
    function list(lines, i) {
      var stack = [], out = '';

      function close() {
        var l = stack.pop();
        var tag = l.ordered ? 'ol' : 'ul';
        var html = '<' + tag + ' class="' + (l.ordered ? C.ol : C.ul) + '"' +
          (l.start > 1 ? ' start="' + l.start + '"' : '') + '>' +
          l.items.map(function (it) { return '<li class="' + C.li + '">' + it.join('') + '</li>'; }).join('') +
          '</' + tag + '>';
        if (stack.length) {
          var p = stack[stack.length - 1];
          p.items[p.items.length - 1].push(html);   /* 巢狀清單塞回上一層的那一項裡 */
        } else {
          out += html;
        }
      }

      function open(m) { stack.push({ ordered: m.ordered, indent: m.indent, start: m.start, items: [] }); }

      while (i < lines.length) {
        var raw = lines[i];
        if (!raw.trim()) {
          /* 空行：後面還接著清單就當同一串，否則收工 */
          var nx = lines[i + 1];
          if (nx && nx.trim() && (item(nx) || /^\s{2,}\S/.test(nx))) { i++; continue; }
          break;
        }
        var m = item(raw);
        if (m) {
          while (stack.length && m.indent < stack[stack.length - 1].indent) close();
          if (!stack.length || m.indent > stack[stack.length - 1].indent) open(m);
          else if (m.ordered !== stack[stack.length - 1].ordered) { close(); open(m); }
          stack[stack.length - 1].items.push([inline(m.text)]);
          i++;
          continue;
        }
        /* 不是項目：只有「縮排夠深的續行」才併進上一項，其餘交還給外面重新判斷 */
        if (!stack.length) break;
        if (/^\s*(?:`{3,}|~{3,}|>|#{1,6}\s)/.test(raw)) break;
        if (/^\s{0,1}\S/.test(raw)) break;
        var top = stack[stack.length - 1];
        top.items[top.items.length - 1].push('<br>' + inline(raw.trim()));
        i++;
      }
      while (stack.length) close();
      return { html: out, next: i };
    }

    /* 會另起一個區塊的行；段落遇到就停 */
    function isBlockStart(line) {
      return /^ {0,3}(?:`{3,}|~{3,})/.test(line) ||
        /^ {0,3}(?:-{3,}|\*{3,}|_{3,})\s*$/.test(line) ||
        /^ {0,3}#{1,6}\s/.test(line) ||
        /^ {0,3}>/.test(line) ||
        !!item(line);
    }

    function blocks(lines) {
      var out = [], i = 0;
      while (i < lines.length) {
        var line = lines[i];
        if (!line.trim()) { i++; continue; }
        var m;

        /* 圍欄程式碼：內容原封不動（只轉義），換行與空白都留著，太寬就自己橫向捲。
           語言標記目前不顯示。 */
        if ((m = /^ {0,3}(`{3,}|~{3,})(.*)$/.exec(line))) {
          var fence = m[1].charAt(0), len = m[1].length, buf = [];
          var closer = new RegExp('^ {0,3}' + fence + '{' + len + ',}\\s*$');
          i++;
          while (i < lines.length && !closer.test(lines[i])) { buf.push(lines[i]); i++; }
          if (i < lines.length) i++;                 /* 吃掉收尾的圍欄；沒收尾就讀到檔尾 */
          out.push('<pre class="' + C.pre + '"><code class="' + C.precode + '">' +
            esc(buf.join('\n')) + '</code></pre>');
          continue;
        }

        /* 水平線 */
        if (/^ {0,3}(?:-{3,}|\*{3,}|_{3,})\s*$/.test(line)) {
          out.push('<hr class="' + C.hr + '">');
          i++;
          continue;
        }

        /* 標題 # ~ ###### */
        if ((m = /^ {0,3}(#{1,6})\s+(.*)$/.exec(line))) {
          var n = m[1].length;
          out.push('<h' + n + ' class="' + C.h[n - 1] + '">' +
            inline(m[2].replace(/\s+#+\s*$/, '')) + '</h' + n + '>');
          i++;
          continue;
        }

        /* 引言：把 > 拿掉之後整段再跑一次，所以引言裡照樣有清單與程式碼 */
        if (/^ {0,3}>/.test(line)) {
          var q = [];
          while (i < lines.length && /^ {0,3}>/.test(lines[i])) {
            q.push(lines[i].replace(/^ {0,3}>\s?/, ''));
            i++;
          }
          out.push('<blockquote class="' + C.quote + '">' + blocks(q) + '</blockquote>');
          continue;
        }

        /* 表格（GFM）。第二列對不上就走不到這裡，會被當成普通段落。 */
        if (isTableHead(lines, i)) {
          var t = table(lines, i);
          out.push(t.html);
          i = t.next;
          continue;
        }

        /* 清單 */
        if (item(line)) {
          var l = list(lines, i);
          out.push(l.html);
          i = l.next;
          continue;
        }

        /* 其餘都是段落。原文的換行照留（GitLab／Redmine 的描述都是這樣看的）。 */
        var p = [];
        while (i < lines.length && lines[i].trim() && !isBlockStart(lines[i]) && !isTableHead(lines, i)) {
          p.push(inline(lines[i].trim()));
          i++;
        }
        if (!p.length) { p.push(inline(lines[i].trim())); i++; }   /* 防呆：不讓任何一行被吞掉 */
        out.push('<p class="' + C.p + '">' + p.join('<br>') + '</p>');
      }
      return out.join('');
    }

    function render(src) {
      var text = String(src == null ? '' : src)
        .replace(/\u0000/g, '')      /* 佔位符用的字元，先清掉免得被冒充 */
        .replace(/\r\n?/g, '\n')
        .replace(/\t/g, '    ');
      return blocks(text.split('\n'));
    }

    /* css 開出來給圖片的非同步載入用：換掉佔位的時候要跟這裡長一樣 */
    return { render: render, escape: esc, css: C };
  })();

  /* 面板裡（描述、留言）的連結一律開系統瀏覽器：不在 app 視窗裡導航，也不再疊一層面板。
     stopPropagation 是為了不讓 interceptExternalLinks() 再處理一次。 */
  function openLinksExternally(el) {
    el.addEventListener('click', function (e) {
      var a = e.target && e.target.closest ? e.target.closest('a[href]') : null;
      if (!a || !el.contains(a)) return;
      e.preventDefault();
      e.stopPropagation();
      var href = a.getAttribute('href') || '';
      if (href && window.Log && window.Log.openExternal) window.Log.openExternal(href);
    });
  }

  /* ---------- 描述／留言裡的圖片 ----------
     MD 只留佔位（原始網址記在 data-img-src、alt 記在 data-img-alt），到這裡才真的去抓。
     Redmine／GitLab 的附圖多半要帶 token，也常寫成相對路徑，直接 <img src> 多半 401 或空白，
     所以由後端用同一套設定抓回來變成 data: URI。
     一則描述／留言裡可能有好幾張，各抓各的，失敗的那張不影響其他張。

     安全：src 只接受後端回傳、而且真的是 data:image/…;base64, 開頭的字串，其他一律不塞。 */
  var DATA_IMAGE = /^data:image\/[a-z0-9.+-]+;base64,[A-Za-z0-9+/]+={0,2}$/i;

  /* 抓不到就退回原本那個「可點的連結」樣子，旁邊用灰字寫原因 */
  function paintImageFallback(box, src, alt, why) {
    box.innerHTML = '';

    var tag = document.createElement('span');
    tag.className = MD.css.imgtag;
    tag.textContent = '圖片';
    box.appendChild(tag);

    /* 絕對網址才做得成連結；相對路徑點了也沒用，就照實把它寫出來 */
    if (/^https?:\/\//i.test(src)) {
      var a = document.createElement('a');
      a.className = MD.css.link;
      a.setAttribute('href', src);
      a.setAttribute('rel', 'noopener noreferrer');
      a.textContent = alt || src;
      box.appendChild(a);
    } else {
      var t = document.createElement('span');
      t.textContent = alt || src;
      box.appendChild(t);
    }

    var note = document.createElement('span');
    note.className = MD.css.imgnote;
    note.textContent = '（載入失敗：' + why + '）';
    box.appendChild(note);
  }

  function loadPanelImages(el, pageUrl) {
    var boxes = el.querySelectorAll('[data-img-src]');
    if (!boxes.length) return;
    if (!window.Log || !window.Log.fetchImage) return;

    Array.prototype.forEach.call(boxes, function (box) {
      var src = box.getAttribute('data-img-src') || '';
      var alt = box.getAttribute('data-img-alt') || '';
      box.removeAttribute('data-img-src');      /* 同一個佔位只抓一次 */
      if (!src) return;

      window.Log.fetchImage(pageUrl, src).then(function (r) {
        var uri = r && r.data_uri;
        if (!uri || !DATA_IMAGE.test(uri)) throw new Error('後端回的不是圖片資料');
        var img = document.createElement('img');
        img.className = MD.css.img;
        img.alt = alt;
        img.loading = 'lazy';
        img.src = uri;
        box.innerHTML = '';
        box.appendChild(img);
      }).catch(function (e) {
        paintImageFallback(box, src, alt, (e && e.message) ? e.message : String(e));
      });
    });
  }

  /* 留言區：標題列寫「留言」與則數，每一則是「作者・時間」小字加 markdown 內文。
     內文一律走 MD.render()（跟描述同一套：先逐字轉義再組標籤），不直接塞原文。 */
  function paintComments(body, c) {
    var list = (c && c.comments) || [];
    var err = c && c.comments_error;

    var head = document.createElement('h3');
    head.className = 'mt-7 flex items-baseline gap-2 px-1 font-semibold tracking-tight';
    var label = document.createElement('span');
    label.textContent = '留言';
    head.appendChild(label);
    if (list.length) {
      var n = document.createElement('span');
      n.className = 'font-normal text-muted dark:text-dmuted';
      n.textContent = list.length + ' 則';
      head.appendChild(n);
    }
    body.appendChild(head);

    /* 留言抓失敗：只有這一區說明原因，上面的內容照樣看得到 */
    if (err) {
      var bad = document.createElement('p');
      bad.className = 'mt-2 break-words px-1 text-muted dark:text-dmuted';
      bad.textContent = '留言讀取失敗：' + err;
      body.appendChild(bad);
      return;
    }

    if (!list.length) {
      var none = document.createElement('p');
      none.className = 'mt-2 px-1 text-muted dark:text-dmuted';
      none.textContent = '沒有留言';
      body.appendChild(none);
      return;
    }

    /* 群組內嵌清單：白面板、圓角 12px，每則之間一條髮絲線。
       這一區**不自己捲**：整個面板只有主體那一根捲軸，滑鼠滾到留言上不會卡在內層框裡。 */
    var box = document.createElement('div');
    box.className = 'mt-2 rounded-xl border border-line dark:border-dline';

    list.forEach(function (m, i) {
      var row = document.createElement('div');
      row.className = 'break-words px-4 py-3' +
        (i ? ' border-t border-line dark:border-dline' : '');

      var who = document.createElement('div');
      who.className = 'text-muted dark:text-dmuted';
      who.textContent = (m.author || '（不明）') + (m.time ? '・' + m.time : '');
      row.appendChild(who);

      var text = document.createElement('div');
      text.className = 'mt-1';
      var raw = (m.body || '').trim();
      if (raw) {
        /* 跟描述同一套轉換：原文在 MD 裡逐字轉義過才組標籤 */
        text.innerHTML = MD.render(raw);
      } else {
        text.className += ' text-muted dark:text-dmuted';
        text.textContent = '（空白留言）';
      }
      row.appendChild(text);

      box.appendChild(row);
    });

    openLinksExternally(box);
    body.appendChild(box);
    loadPanelImages(box, c && c.url);
  }

  /* 抓到內容：標題、狀態、metadata、描述、留言 */
  function paintContent(body, c) {
    body.innerHTML = '';

    var h = document.createElement('h2');
    h.className = 'font-display text-2xl font-semibold tracking-tightest';
    h.textContent = c.title || '（沒有標題）';
    body.appendChild(h);

    var pills = document.createElement('div');
    pills.className = 'mt-2 flex flex-wrap items-center gap-2';
    var st = document.createElement('span');
    st.className = 'rounded-full px-2.5 py-0.5 leading-tight ' + panelStatusClass(c);
    st.textContent = c.state_label || c.state || '';
    pills.appendChild(st);
    body.appendChild(pills);

    if (c.meta && c.meta.length) {
      var list = document.createElement('div');
      list.className = 'mt-4 overflow-hidden rounded-xl border border-line dark:border-dline';
      c.meta.forEach(function (m) { list.appendChild(metaRow(m.label, m.value)); });
      body.appendChild(list);
    }

    var head = document.createElement('h3');
    head.className = 'mt-7 px-1 font-semibold tracking-tight';
    head.textContent = '描述';
    body.appendChild(head);

    var desc = document.createElement('div');
    var text = (c.description || '').trim();
    if (text) {
      /* 這一區**不自己捲**：內容多長就多長，捲動統一交給面板主體那一層，
         滑鼠滾到描述上不會卡在內層框裡。程式碼區塊與表格的橫向捲動另外由 MD 處理。 */
      desc.className = 'mt-2 break-words rounded-xl border border-line px-4 py-3 dark:border-dline';
      /* 描述是 markdown，自己轉成 HTML。原文在 MD 裡逐字轉義過才組標籤，
         所以這裡的 innerHTML 不會塞進任何來自外部系統的可執行內容。 */
      desc.innerHTML = MD.render(text);
      openLinksExternally(desc);
    } else {
      desc.className = 'mt-2 px-1 text-muted dark:text-dmuted';
      desc.textContent = '（沒有描述）';
    }
    body.appendChild(desc);
    if (text) loadPanelImages(desc, c.url);

    paintComments(body, c);
  }

  /* 沒設 token、認不出網址、連不上……原因照實寫，按鈕照樣給 */
  function paintError(body, msg) {
    body.innerHTML = '';
    var pill = document.createElement('span');
    pill.className = 'rounded-full px-2.5 py-0.5 leading-tight ' +
      (window.Log ? window.Log.statusClass('parked') : '');
    pill.textContent = '抓不到內容';
    body.appendChild(pill);

    var p = document.createElement('p');
    p.className = 'mt-2.5 break-words';
    p.textContent = msg;
    body.appendChild(p);

    var hint = document.createElement('p');
    hint.className = 'mt-2 text-muted dark:text-dmuted';
    hint.textContent = '還是可以用下面的按鈕在瀏覽器開啟。';
    body.appendChild(hint);
  }

  /* MR 已合併就借「已歸檔」的灰，開啟中借「待合併」的黃，其餘一律灰膠囊 */
  function panelStatusClass(c) {
    if (!window.Log || !window.Log.statusClass) return '';
    if (c.state === 'merged') return window.Log.statusClass('archived');
    if (c.state === 'opened') return window.Log.statusClass('review');
    if (c.state === 'closed') return window.Log.statusClass('done');
    return window.Log.statusClass('todo');
  }

  function openLinkPanel(url) {
    closePanel();
    var kind = window.Log && window.Log.detectKind ? window.Log.detectKind(url) : 'link';

    panelEl = document.createElement('div');
    panelEl.setAttribute('data-link-panel', '');
    panelEl.className = 'fixed inset-0 z-50 flex items-start justify-center bg-ink/25 p-4 pt-10 dark:bg-black/60';
    panelEl.innerHTML = '' +
      /* 面板只有一根捲軸：主體那一層。描述與留言都不自己捲，
         所以滑鼠滾到哪裡都是在捲整個面板。主體底下多留一段空白，
         配上連結列本來就有的髮絲線，捲到底看得出來下面還有東西（捲軸是全站隱藏的）。 */
      '<div data-panel-card class="flex max-h-full w-full max-w-5xl flex-col overflow-hidden rounded-xl border border-line bg-surface shadow-pop dark:border-dline dark:bg-dsurface">' +
        '<div class="flex shrink-0 items-center gap-2.5 border-b border-line px-6 py-3 dark:border-dline">' +
          '<span data-panel-kind class="shrink-0 rounded-full px-2.5 py-0.5 leading-tight"></span>' +
          '<span data-panel-ref class="min-w-0 flex-1 truncate font-mono text-muted dark:text-dmuted"></span>' +
          '<button data-panel-close class="shrink-0 rounded-full border border-line bg-surface px-3 py-1 leading-tight shadow-sm hover:bg-accentsoft dark:border-dline dark:bg-dsurface dark:hover:bg-daccentsoft">關閉</button>' +
        '</div>' +
        '<div data-panel-body class="min-h-0 flex-1 overflow-y-auto px-6 pb-8 pt-5"></div>' +
        '<div class="flex shrink-0 flex-wrap items-center gap-2 border-t border-line px-6 py-3 dark:border-dline">' +
          '<span data-panel-url class="min-w-0 flex-1 truncate font-mono text-muted dark:text-dmuted"></span>' +
          '<button data-panel-open class="shrink-0 rounded-full bg-accent px-4 py-2 font-medium leading-tight text-white shadow-sm hover:opacity-90 dark:bg-daccent">在瀏覽器開啟</button>' +
        '</div>' +
      '</div>';

    var body = panelEl.querySelector('[data-panel-body]');
    var pill = panelEl.querySelector('[data-panel-kind]');
    pill.className = 'shrink-0 rounded-full px-2.5 py-0.5 leading-tight ' +
      (window.Log ? window.Log.statusClass(kind === 'mr' ? 'review' : 'building') : '');
    pill.textContent = window.Log ? window.Log.linkLabel(url) : '';
    panelEl.querySelector('[data-panel-url]').textContent = url;

    var loading = document.createElement('p');
    loading.className = 'text-muted dark:text-dmuted';
    loading.textContent = '讀取中…';
    body.appendChild(loading);

    panelEl.querySelector('[data-panel-close]').addEventListener('click', closePanel);
    panelEl.querySelector('[data-panel-open]').addEventListener('click', function () {
      closePanel();
      if (window.Log && window.Log.openExternal) window.Log.openExternal(url);
    });
    /* 點面板外面關掉 */
    panelEl.addEventListener('mousedown', function (e) {
      if (e.target === panelEl) closePanel();
    });
    document.addEventListener('keydown', onPanelKey);
    document.body.appendChild(panelEl);

    var mine = panelEl;
    window.Log.fetchLink(url).then(function (c) {
      if (panelEl !== mine) return;
      mine.querySelector('[data-panel-ref]').textContent = c.reference || '';
      paintContent(body, c);
    }).catch(function (e) {
      if (panelEl !== mine) return;
      paintError(body, (e && e.message) ? e.message : String(e));
    });
  }

  /* app 視窗不該被外部網站佔走：
     MR 與議題先開面板看內容，其餘連結一律丟給系統瀏覽器 */
  function interceptExternalLinks() {
    document.addEventListener('click', function (e) {
      var a = e.target && e.target.closest ? e.target.closest('a[href]') : null;
      if (!a) return;
      var href = a.getAttribute('href') || '';
      if (!/^https?:\/\//i.test(href)) return;
      e.preventDefault();
      if (!window.Log) return;
      var kind = window.Log.detectKind ? window.Log.detectKind(href) : 'link';
      if ((kind === 'mr' || kind === 'issue') && window.Log.fetchLink) openLinkPanel(href);
      else if (window.Log.openExternal) window.Log.openExternal(href);
    });
  }

  /* 資料夾沒設好或沒讀到檔案時，每一頁都看得到 */
  function banner() {
    if (currentPage() === 'settings.html') return;
    if (!window.Log || !window.Log.workspace) return;
    var ws = window.Log.workspace();
    var msg = null;
    if (!ws.folder) msg = '還沒設定日誌資料夾。';
    else if (!ws.folder_exists) msg = '找不到資料夾 ' + ws.folder;
    else if (!ws.days.length) msg = ws.folder + ' 裡沒有 <民國7碼>.md，例如 1150817.md。';
    if (!msg) return;

    var bar = document.createElement('div');
    bar.className = 'shrink-0 border-b border-line dark:border-dline ' + window.Log.statusClass('parked');
    bar.innerHTML = '<div class="flex items-center gap-3 px-4 py-2 md:px-6">' +
      '<span class="min-w-0 flex-1 truncate"></span>' +
      '<a href="settings.html" class="shrink-0 rounded-full border border-line bg-surface px-3 py-1 leading-none dark:border-dline dark:bg-dsurface">去設定</a>' +
      '</div>';
    bar.querySelector('span').textContent = msg;
    var header = document.querySelector('[data-od-id="topbar"]');
    if (header && header.parentNode) header.parentNode.insertBefore(bar, header.nextSibling);
    else document.body.insertBefore(bar, document.body.firstChild);
  }

  /* 分段控制：選中的那格是白色膠囊 */
  function segmented(here) {
    var segs = NAV.map(function (n) {
      var active = n.href === here;
      return '<a href="' + n.href + '"' + (active ? ' aria-current="page"' : '') +
        ' class="shrink-0 rounded-full px-4 py-1.5 leading-tight ' +
        (active
          ? 'bg-surface font-semibold text-ink shadow-sm dark:bg-dline dark:text-dink'
          : 'text-muted hover:text-ink dark:text-dmuted dark:hover:text-dink') +
        '">' + n.label + '</a>';
    }).join('');
    return '<nav class="flex items-center gap-0.5 rounded-full bg-accentsoft p-1 dark:bg-daccentsoft" data-od-id="topnav">' + segs + '</nav>';
  }

  function topbar(crumb, tab) {
    var here = tab || currentPage();

    return '' +
      '<header class="shrink-0 border-b border-line bg-paper/95 backdrop-blur dark:border-dline dark:bg-dpaper/95" data-od-id="topbar">' +
        '<div class="flex flex-col gap-2 px-4 py-2.5 md:grid md:grid-cols-[1fr_auto_1fr] md:items-center md:gap-3 md:px-6">' +
          '<div class="flex min-w-0 items-center gap-2">' +
            '<span class="h-2.5 w-2.5 shrink-0 rounded-full bg-accent dark:bg-daccent"></span>' +
            '<a href="index.html" class="shrink-0 font-display font-semibold tracking-tight">工作日誌</a>' +
            (crumb ? '<span class="shrink-0 text-muted dark:text-dmuted">/</span>' +
                     '<span class="min-w-0 truncate text-muted dark:text-dmuted" data-od-id="crumb">' + crumb + '</span>' : '') +
          '</div>' +
          '<div class="order-last flex justify-center overflow-x-auto md:order-none">' + segmented(here) + '</div>' +
          '<div class="flex items-center justify-end gap-2">' +
            '<span data-shell-badge></span>' +
            themeButton() +
          '</div>' +
        '</div>' +
      '</header>';
  }

  /* ---------- 通知 ----------
     訊息掛在右上角（工具列下面），不然放在頁尾很容易沒看到。 */
  var toastTimer = null;

  function toast(msg, kind) {
    if (!msg) return;
    var box = document.querySelector('[data-toast]');
    if (!box) {
      box = document.createElement('div');
      box.setAttribute('data-toast', '');
      box.className = 'pointer-events-none fixed right-4 top-20 z-50 flex flex-col items-end gap-2';
      document.body.appendChild(box);
    }
    box.innerHTML = '';

    var tone = 'bg-surface text-ink dark:bg-dsurface dark:text-dink';
    if (kind === 'error') tone = 'bg-surface text-accent dark:bg-dsurface dark:text-daccent';

    var item = document.createElement('div');
    item.className = 'pointer-events-auto max-w-sm rounded-xl border border-line px-4 py-2.5 shadow-pop ' +
      'opacity-0 transition-opacity duration-150 dark:border-dline ' + tone;
    item.textContent = msg;
    box.appendChild(item);
    requestAnimationFrame(function () { item.style.opacity = '1'; });

    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(function () {
      item.style.opacity = '0';
      setTimeout(function () { if (item.parentNode) item.parentNode.removeChild(item); }, 200);
    }, kind === 'error' ? 6000 : 3500);
  }

  function mount() {
    document.querySelectorAll('[data-shell="topbar"]').forEach(function (n) {
      n.outerHTML = topbar(n.getAttribute('data-crumb') || '', n.getAttribute('data-tab') || '');
    });
    if (window.Theme && window.Theme.mount) window.Theme.mount();
    interceptExternalLinks();
    if (window.Log && window.Log.ready) {
      window.Log.ready(function () { refreshBadge(); banner(); });
    }
  }

  window.Shell = {
    mount: mount,
    topbar: topbar,
    themeButton: themeButton,
    refreshBadge: refreshBadge,
    openLinkPanel: openLinkPanel,
    renderMarkdown: MD.render,
  };
  document.addEventListener('DOMContentLoaded', mount);
})();
