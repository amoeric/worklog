//! 把資料夾裡的 `<民國7碼>.md` 讀成結構化資料。
//!
//! 檔案格式（來自 CLAUDE.md 的每日工作日誌規則）：
//!
//! ```text
//! ## project_a
//!
//! - `已歸檔` [feat: 對話頁補上左側清單](http://.../merge_requests/64)
//! - `暫存` [search-message-history：對話紀錄搜尋](https://.../issues/32979)
//! - `完成` 議題對帳：兩邊未結案數對齊
//! ```
//!
//! 每行開頭的行內程式碼是狀態標籤；行內其他位置的反引號不算狀態。

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::model::{status_by_id, status_by_zh, Day, Entry, HistoryPoint, Item};

static RE_HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,6}\s+(.+?)\s*$").unwrap());
static RE_BULLET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*[-*]\s+(.*?)\s*$").unwrap());
static RE_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\[(.+?)\]\(([^)\s]+)\)\s*(.*)$").unwrap());
static RE_ARCHIVE_SLUG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^chore\s*[:：]\s*archive\s+([a-z][a-z0-9_-]{2,})\b").unwrap()
});
static RE_SLUG_PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([a-z][a-z0-9_-]{2,})：").unwrap());

/// `1150817.md`
fn is_log_file(name: &str) -> bool {
    let stem = match name.strip_suffix(".md") {
        Some(s) => s,
        None => return false,
    };
    stem.len() == 7 && stem.chars().all(|c| c.is_ascii_digit())
}

/// 掃資料夾，回傳依日期排好的每一天，以及看不懂而略過的行。
pub fn scan(folder: &Path) -> (Vec<Day>, Vec<String>) {
    let mut days: Vec<Day> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    let read = match std::fs::read_dir(folder) {
        Ok(r) => r,
        Err(e) => {
            skipped.push(format!("讀不到資料夾 {}：{}", folder.display(), e));
            return (days, skipped);
        }
    };

    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !is_log_file(&name) {
            continue;
        }
        let text = match std::fs::read_to_string(entry.path()) {
            Ok(t) => t,
            Err(e) => {
                skipped.push(format!("{}：讀檔失敗 {}", name, e));
                continue;
            }
        };
        let code = name.trim_end_matches(".md").to_string();
        let (entries, mut bad) = parse_file(&text);
        for b in bad.drain(..) {
            skipped.push(format!("{}：{}", name, b));
        }
        days.push(Day { code, file: name, entries });
    }

    days.sort_by(|a, b| a.code.cmp(&b.code));
    (days, skipped)
}

/// 解析單一檔案內容。回傳條目與看不懂的行。
pub fn parse_file(text: &str) -> (Vec<Entry>, Vec<String>) {
    let mut project = String::from("其他");
    let mut entries: Vec<Entry> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();

    for (no, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(c) = RE_HEADING.captures(line) {
            project = c[1].trim().to_string();
            continue;
        }
        if let Some(c) = RE_BULLET.captures(line) {
            let body = c[1].trim();
            if body.is_empty() {
                continue;
            }
            // 更深一層的縮排 bullet 是說明，不是獨立條目
            let indent = line.len() - line.trim_start().len();
            if indent >= 2 {
                continue;
            }
            entries.push(parse_entry(&project, body, line, no));
            continue;
        }
        // 不是標題也不是條目：留著回報，不要默默吞掉
        skipped.push(format!("看不懂的行：{}", trimmed));
    }

    (entries, skipped)
}

/// 解析一條 bullet 的內容。`no` 是這一行在檔案裡的行號（0 起算）。
fn parse_entry(project: &str, body: &str, raw: &str, no: usize) -> Entry {
    let (status, rest) = split_status(body);
    let (title, url, note) = split_link(rest);
    Entry {
        project: project.to_string(),
        status,
        title,
        url,
        note,
        item: None,
        raw: raw.trim().to_string(),
        line: no,
    }
}

/// 只有「行首的行內程式碼且內容是已知中文標籤」才算狀態。
/// 這樣 `` 建立規則（寫入 `~/.claude/CLAUDE.md`） `` 不會被誤判。
fn split_status(body: &str) -> (Option<String>, &str) {
    if !body.starts_with('`') {
        return (None, body);
    }
    let after = &body[1..];
    let end = match after.find('`') {
        Some(i) => i,
        None => return (None, body),
    };
    let label = &after[..end];
    match status_by_zh(label.trim()) {
        Some(s) => (Some(s.id.to_string()), after[end + 1..].trim_start()),
        None => (None, body),
    }
}

/// `[標題](url)（補充）` → (標題, url, 補充)；不是連結就整段當標題。
fn split_link(rest: &str) -> (String, Option<String>, Option<String>) {
    if let Some(c) = RE_LINK.captures(rest) {
        let title = c[1].trim().to_string();
        let url = c[2].trim().to_string();
        let tail = c[3].trim();
        let note = clean_note(tail);
        return (title, Some(url), note);
    }
    (rest.trim().to_string(), None, None)
}

/// 去掉補充外圍的全形／半形括號
fn clean_note(tail: &str) -> Option<String> {
    if tail.is_empty() {
        return None;
    }
    let t = tail.trim();
    let inner = t
        .strip_prefix('（')
        .and_then(|s| s.strip_suffix('）'))
        .or_else(|| t.strip_prefix('(').and_then(|s| s.strip_suffix(')')))
        .unwrap_or(t);
    let inner = inner.trim();
    if inner.is_empty() { None } else { Some(inner.to_string()) }
}

// ---------- 工作項目歸戶 ----------

/// 從標題直接抽 slug。兩種寫法：
/// 1. `search-message-history：對話紀錄搜尋` —— 全形冒號前是 slug
/// 2. `chore: archive obfuscate-record-ids` —— 封存 MR 的固定寫法
fn slug_of(title: &str) -> Option<String> {
    if let Some(c) = RE_ARCHIVE_SLUG.captures(title) {
        return Some(c[1].to_lowercase());
    }
    if let Some(c) = RE_SLUG_PREFIX.captures(title.trim()) {
        return Some(c[1].to_lowercase());
    }
    None
}

/// 判斷這一筆有沒有帶生命週期狀態（`完成` 不算）。
fn has_lifecycle_status(e: &Entry) -> bool {
    e.status
        .as_deref()
        .and_then(status_by_id)
        .map(|s| s.lifecycle)
        .unwrap_or(false)
}

/// 網址正規化：去掉 fragment、query 與結尾斜線。
/// 只做這三件事，主機與路徑的大小寫原樣保留（路徑本來就可能區分大小寫）。
fn normalize_url(url: &str) -> String {
    let u = url.trim();
    let u = u.split('#').next().unwrap_or(u);
    let u = u.split('?').next().unwrap_or(u);
    u.trim_end_matches('/').to_string()
}

/// FNV-1a 64bit。自己寫是為了「同一個 key 永遠得到同一個 id」，
/// 不依賴標準函式庫的 hasher（那個不保證跨版本穩定）。
fn fnv1a64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 歸不到 slug 時，用條目本身算一個穩定的 id。
///
/// key 的取法：
/// - 有連結 → `link:<正規化後的網址>`（同一支 MR／議題跨天寫幾次都是同一個 key）
/// - 沒連結 → `entry:<專案>\x01<標題>`
///
/// 真正的 id 是 key 的 FNV-1a 雜湊，寫成 `auto-<16 碼十六進位>`。
/// 用雜湊而不是直接把網址塞進 id，是因為 id 會原封不動接進網址
/// （`work-item.html?id=...`）與 HTML 屬性裡，網址裡的 `+`、`#`、`%`
/// 會在那條路上被改寫；`auto-` 加十六進位只有 ASCII 英數與短横線，怎麼傳都不會變形。
fn fallback_id(project: &str, title: &str, url: Option<&str>) -> String {
    let key = match url {
        Some(u) => format!("link:{}", normalize_url(u)),
        None => format!("entry:{}\u{1}{}", project.trim(), title.trim()),
    };
    format!("auto-{:016x}", fnv1a64(&key))
}

/// 去掉標題前面的 `slug：`，剩下的才是人看的描述。
fn strip_slug_prefix(title: &str, slug: &str) -> String {
    let p = format!("{}：", slug);
    match title.trim().strip_prefix(&p) {
        Some(s) => s.trim().to_string(),
        None => title.trim().to_string(),
    }
}

/// 把每個條目歸到工作項目，然後把工作項目本身組出來。
///
/// 歸戶順序（先命中先算）：
/// 1. 標題裡直接寫了 slug
/// 2. 條目的連結，在別處出現在某個 slug 名下（同一支 change 的 issue 連結會重複出現）
/// 3. 下一行是 `chore: archive <slug>`，且同一天同專案 —— 實作 MR 與封存 MR 成對出現
/// 4. 以上都對不上，但這一筆**帶生命週期狀態** —— 讓它自己成為一個工作項目，
///    id 由連結（或專案＋標題）算出來，所以同一件事跨天寫還是會併成同一支
///
/// `完成` 不是生命週期狀態，所以一次性雜項不會走第 4 條、也就不會變成工作項目。
/// 沒標狀態的條目一樣不歸戶，只會出現在當天的日誌裡。
pub fn derive_items(days: &mut [Day]) -> Vec<Item> {
    // 1. 標題直接帶 slug
    for day in days.iter_mut() {
        for e in day.entries.iter_mut() {
            if e.item.is_none() {
                e.item = slug_of(&e.title);
            }
        }
    }

    // 建 url → slug 對照；同一個 url 對到兩個 slug 就不採用
    let mut url_slug: HashMap<String, String> = HashMap::new();
    let mut ambiguous: HashSet<String> = HashSet::new();
    for day in days.iter() {
        for e in day.entries.iter() {
            if let (Some(slug), Some(url)) = (e.item.as_ref(), e.url.as_ref()) {
                match url_slug.get(url) {
                    Some(existing) if existing != slug => {
                        ambiguous.insert(url.clone());
                    }
                    _ => {
                        url_slug.insert(url.clone(), slug.clone());
                    }
                }
            }
        }
    }
    for u in &ambiguous {
        url_slug.remove(u);
    }

    // 2. 靠連結歸戶
    for day in days.iter_mut() {
        for e in day.entries.iter_mut() {
            if e.item.is_some() {
                continue;
            }
            if let Some(url) = e.url.as_ref() {
                if let Some(slug) = url_slug.get(url) {
                    e.item = Some(slug.clone());
                }
            }
        }
    }

    // 3. 實作 MR ＋ 緊接著的封存 MR
    for day in days.iter_mut() {
        let n = day.entries.len();
        for i in 0..n.saturating_sub(1) {
            if day.entries[i].item.is_some() {
                continue;
            }
            let next_item = day.entries[i + 1].item.clone();
            let same_project = day.entries[i].project == day.entries[i + 1].project;
            let next_is_archive = slug_of(&day.entries[i + 1].title).is_some()
                && day.entries[i + 1].title.to_lowercase().contains("archive");
            if same_project && next_is_archive {
                day.entries[i].item = next_item;
            }
        }
    }

    // 4. 前三條都歸不到，但帶生命週期狀態的，讓它自己成為一支工作項目。
    //    只放寬到「帶生命週期狀態」是刻意的：`完成` 那種一次性雜項若也變成工作項目，
    //    看板會被灌爆。順序放在最後，前面三條一律優先，既有行為完全不變。
    for day in days.iter_mut() {
        for e in day.entries.iter_mut() {
            if e.item.is_some() || !has_lifecycle_status(e) {
                continue;
            }
            e.item = Some(fallback_id(&e.project, &e.title, e.url.as_deref()));
        }
    }

    build_items(days)
}

fn build_items(days: &[Day]) -> Vec<Item> {
    let mut history: HashMap<String, Vec<HistoryPoint>> = HashMap::new();
    let mut named: HashMap<String, String> = HashMap::new();
    let mut projects: HashMap<String, HashMap<String, usize>> = HashMap::new();

    for day in days {
        for e in &day.entries {
            let slug = match e.item.as_ref() {
                Some(s) => s.clone(),
                None => continue,
            };
            history.entry(slug.clone()).or_default().push(HistoryPoint {
                code: day.code.clone(),
                status: e.status.clone(),
                title: e.title.clone(),
                url: e.url.clone(),
                project: e.project.clone(),
            });
            *projects
                .entry(slug.clone())
                .or_default()
                .entry(e.project.clone())
                .or_insert(0) += 1;

            // 標題以 `slug：` 開頭的那筆，才是這支 change 的正式名稱
            if !named.contains_key(&slug) {
                let stripped = strip_slug_prefix(&e.title, &slug);
                if stripped != e.title.trim() {
                    named.insert(slug.clone(), stripped);
                }
            }
        }
    }

    let order: HashMap<&str, usize> = crate::model::STATUSES
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id, i))
        .collect();

    let mut items: Vec<Item> = Vec::new();
    for (slug, mut points) in history {
        points.sort_by(|a, b| a.code.cmp(&b.code));

        // 目前狀態 = 最後一筆帶生命週期狀態的條目
        let mut status: Option<String> = None;
        let mut since = points.first().map(|p| p.code.clone()).unwrap_or_default();
        for p in points.iter().rev() {
            if let Some(id) = p.status.as_ref() {
                if status_by_id(id).map(|s| s.lifecycle).unwrap_or(false) {
                    status = Some(id.clone());
                    since = p.code.clone();
                    break;
                }
            }
        }
        // 一次都沒進過生命週期的，不算工作項目
        let status = match status {
            Some(s) => s,
            None => continue,
        };

        let issue = points
            .iter()
            .rev()
            .find_map(|p| p.url.clone().filter(|u| u.contains("/issues/")));
        let mr = points
            .iter()
            .rev()
            .find_map(|p| p.url.clone().filter(|u| u.contains("merge_requests")));

        let project = projects
            .get(&slug)
            .and_then(|m| m.iter().max_by_key(|(_, n)| **n).map(|(p, _)| p.clone()))
            .unwrap_or_else(|| "其他".to_string());

        let title = named.get(&slug).cloned().unwrap_or_else(|| {
            points
                .iter()
                .find(|p| p.status.is_some())
                .or_else(|| points.first())
                .map(|p| p.title.clone())
                .unwrap_or_else(|| slug.clone())
        });

        items.push(Item { id: slug, project, title, issue, mr, status, since, history: points });
    }

    items.sort_by(|a, b| {
        let oa = order.get(a.status.as_str()).copied().unwrap_or(99);
        let ob = order.get(b.status.as_str()).copied().unwrap_or(99);
        oa.cmp(&ob).then(a.since.cmp(&b.since)).then(a.id.cmp(&b.id))
    });
    items
}

/// 專案排序：出現次數多的在前，`其他` 永遠排最後。
pub fn project_list(days: &[Day]) -> Vec<String> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for day in days {
        for e in &day.entries {
            *counts.entry(e.project.clone()).or_insert(0) += 1;
        }
    }
    let mut list: Vec<(String, usize)> = counts.into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let mut names: Vec<String> = list.into_iter().map(|(p, _)| p).collect();
    if let Some(pos) = names.iter().position(|p| p == "其他") {
        let other = names.remove(pos);
        names.push(other);
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day_of(code: &str, text: &str) -> Day {
        let (entries, _) = parse_file(text);
        Day { code: code.into(), file: format!("{}.md", code), entries }
    }

    const SAMPLE: &str = r#"## project_a

- `已歸檔` [feat: 每個對話都是自己的頁面，對話頁補上左側清單](http://gitlab.example.com/group/project_a/-/merge_requests/64)
- `已歸檔` [chore: archive room-page-full-layout](http://gitlab.example.com/group/project_a/-/merge_requests/65)
- `暫存` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)
- `已歸檔` [archive-recalled-attachments：訊息收回後附件的封存與擋門](https://redmine.example.com/issues/32951)（補結案，MR !57 兩天前已合併）
- `完成` 議題對帳：補建三支提案的互連議題

## 其他

- `完成` 建立每日工作日誌自動維護規則（寫入 `~/.claude/CLAUDE.md`）
- 工作日誌 app 原型擴充：狀態總覽與單一項目歷程
"#;

    #[test]
    fn parses_projects_and_statuses() {
        let (entries, skipped) = parse_file(SAMPLE);
        assert!(skipped.is_empty(), "不該有看不懂的行：{:?}", skipped);
        assert_eq!(entries.len(), 7);
        assert_eq!(entries[0].project, "project_a");
        assert_eq!(entries[0].status.as_deref(), Some("archived"));
        assert_eq!(entries[2].status.as_deref(), Some("parked"));
        assert_eq!(entries[5].project, "其他");
    }

    #[test]
    fn backtick_not_at_start_is_not_a_status() {
        let (entries, _) = parse_file(SAMPLE);
        let e = &entries[5];
        assert_eq!(e.status.as_deref(), Some("done"));
        assert!(e.title.contains("CLAUDE.md"), "標題被吃掉了：{}", e.title);
    }

    #[test]
    fn entries_without_a_status_tag_are_kept() {
        let (entries, _) = parse_file(SAMPLE);
        assert_eq!(entries[6].status, None);
        assert!(entries[6].title.starts_with("工作日誌 app"));
    }

    #[test]
    fn link_and_trailing_note_are_split() {
        let (entries, _) = parse_file(SAMPLE);
        let e = &entries[3];
        assert_eq!(e.url.as_deref(), Some("https://redmine.example.com/issues/32951"));
        assert!(e.note.as_deref().unwrap().starts_with("補結案"));
    }

    #[test]
    fn feature_mr_inherits_item_from_adjacent_archive_mr() {
        let (entries, _) = parse_file(SAMPLE);
        let mut days = vec![Day { code: "1150817".into(), file: "1150817.md".into(), entries }];
        let items = derive_items(&mut days);
        assert_eq!(days[0].entries[0].item.as_deref(), Some("room-page-full-layout"));
        assert!(items.iter().any(|i| i.id == "room-page-full-layout" && i.status == "archived"));
        assert!(items.iter().any(|i| i.id == "search-message-history" && i.status == "parked"));
    }

    #[test]
    fn repeated_url_links_entries_across_days() {
        let day1 = "## project_a\n\n- `提案中` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)\n";
        let day2 = "## project_a\n\n- `實作中` [開分支動工](https://redmine.example.com/issues/32979)\n";
        let (e1, _) = parse_file(day1);
        let (e2, _) = parse_file(day2);
        let mut days = vec![
            Day { code: "1150815".into(), file: "1150815.md".into(), entries: e1 },
            Day { code: "1150816".into(), file: "1150816.md".into(), entries: e2 },
        ];
        let items = derive_items(&mut days);
        assert_eq!(days[1].entries[0].item.as_deref(), Some("search-message-history"));
        let it = items.iter().find(|i| i.id == "search-message-history").unwrap();
        assert_eq!(it.status, "building");
        assert_eq!(it.since, "1150816");
    }

    /// 看板改狀態會往當天的 md 補一行 `slug：描述`，
    /// 下次重讀時要靠 slug 歸到同一支，並且把狀態推過去
    #[test]
    fn a_line_written_by_the_board_moves_the_status() {
        let (entries, _) = parse_file(SAMPLE);
        let mut days = vec![Day { code: "1150817".into(), file: "1150817.md".into(), entries }];
        days.push(day_of(
            "1150818",
            "## project_a\n\n- `實作中` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)\n",
        ));

        let items = derive_items(&mut days);
        let it = items.iter().find(|i| i.id == "search-message-history").unwrap();
        assert_eq!(it.status, "building");
        assert_eq!(it.since, "1150818");
    }

    /// 行號要對得上原始檔案：看板改狀態就是靠它定位，不是字串比對
    #[test]
    fn entries_remember_which_line_they_came_from() {
        let md = "## project_a\n\n- `暫存` 甲\n\n## project_b\n\n- `暫存` 甲\n";
        let (entries, _) = parse_file(md);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].line, 2);
        assert_eq!(entries[1].line, 6);
        let lines: Vec<&str> = md.lines().collect();
        for e in &entries {
            assert_eq!(lines[e.line].trim(), e.raw, "行號指到別行去了");
        }
    }

    // ---- 第 4 條：歸不到 slug、但帶生命週期狀態的條目自己成為工作項目 ----

    #[test]
    fn unassigned_lifecycle_entry_becomes_its_own_item() {
        let md = "## project_b\n\n- `待合併` [feat: 上傳路徑改由後台設定](http://gitlab.example.com/group/project_b/-/merge_requests/392)\n";
        let mut days = vec![day_of("1150818", md)];
        let items = derive_items(&mut days);

        let id = days[0].entries[0].item.clone().expect("應該要歸戶到 fallback 項目");
        assert!(id.starts_with("auto-"), "fallback id 格式不對：{}", id);
        let it = items.iter().find(|i| i.id == id).unwrap();
        assert_eq!(it.status, "review");
        assert_eq!(it.project, "project_b");
        assert_eq!(it.title, "feat: 上傳路徑改由後台設定");
        assert_eq!(it.mr.as_deref(), Some("http://gitlab.example.com/group/project_b/-/merge_requests/392"));
    }

    #[test]
    fn fallback_id_is_stable_and_merges_the_same_link_across_days() {
        // 網址結尾斜線／query／fragment 不影響 id
        let d1 = day_of(
            "1150817",
            "## project_b\n\n- `實作中` [feat: 上傳路徑改由後台設定](http://gitlab.example.com/group/project_b/-/merge_requests/392/)\n",
        );
        let d2 = day_of(
            "1150818",
            "## project_b\n\n- `待合併` [開好 MR 等審](http://gitlab.example.com/group/project_b/-/merge_requests/392?tab=diffs#note-1)\n",
        );
        let mut days = vec![d1, d2];
        let items = derive_items(&mut days);

        let a = days[0].entries[0].item.clone().unwrap();
        let b = days[1].entries[0].item.clone().unwrap();
        assert_eq!(a, b, "同一個連結應該算出同一個 id");

        let it = items.iter().find(|i| i.id == a).unwrap();
        assert_eq!(it.status, "review", "狀態取最後一筆");
        assert_eq!(it.since, "1150818");
        assert_eq!(it.history.len(), 2);
    }

    #[test]
    fn fallback_id_without_a_link_uses_project_and_title() {
        let d1 = day_of("1150817", "## project_b\n\n- `提案中` 匯出報表的欄位規格\n");
        let d2 = day_of("1150818", "## project_b\n\n- `實作中` 匯出報表的欄位規格\n");
        let mut days = vec![d1, d2];
        let items = derive_items(&mut days);

        let id = days[0].entries[0].item.clone().unwrap();
        assert_eq!(days[1].entries[0].item.as_deref(), Some(id.as_str()));
        let it = items.iter().find(|i| i.id == id).unwrap();
        assert_eq!(it.status, "building");
        assert_eq!(it.title, "匯出報表的欄位規格");
    }

    #[test]
    fn done_entries_never_become_items() {
        let md = "## 其他\n\n- `完成` 議題對帳：兩邊未結案數對齊\n- 沒標狀態的一行\n";
        let mut days = vec![day_of("1150818", md)];
        let items = derive_items(&mut days);
        assert!(days[0].entries.iter().all(|e| e.item.is_none()), "`完成` 與沒標狀態的行都不該歸戶");
        assert!(items.is_empty());
    }

    /// 護欄：前三條規則能歸戶的，一律不走 fallback
    #[test]
    fn existing_rules_still_win_over_the_fallback() {
        let (entries, _) = parse_file(SAMPLE);
        let mut days = vec![Day { code: "1150817".into(), file: "1150817.md".into(), entries }];
        let items = derive_items(&mut days);

        // slug 前綴、chore: archive、相鄰封存 MR 三種歸戶結果都不變
        assert_eq!(days[0].entries[0].item.as_deref(), Some("room-page-full-layout"));
        assert_eq!(days[0].entries[1].item.as_deref(), Some("room-page-full-layout"));
        assert_eq!(days[0].entries[2].item.as_deref(), Some("search-message-history"));
        assert_eq!(days[0].entries[3].item.as_deref(), Some("archive-recalled-attachments"));
        assert!(days[0].entries[4].item.is_none(), "`完成` 不歸戶");
        assert!(!items.iter().any(|i| i.id.starts_with("auto-")), "這份範例不該產生 fallback 項目");
    }

    /// 靠重複連結歸戶的，也不能被 fallback 搶走
    #[test]
    fn repeated_url_rule_still_wins_over_the_fallback() {
        let d1 = day_of("1150815", "## project_a\n\n- `提案中` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)\n");
        let d2 = day_of("1150816", "## project_a\n\n- `實作中` [開分支動工](https://redmine.example.com/issues/32979)\n");
        let mut days = vec![d1, d2];
        let items = derive_items(&mut days);
        assert_eq!(days[1].entries[0].item.as_deref(), Some("search-message-history"));
        assert!(!items.iter().any(|i| i.id.starts_with("auto-")));
    }

    // ---- 測試中：推上 staging／測試環境之後的狀態 ----

    /// `測試中` 要解析得出 testing，而且要能把工作項目推到那一格
    #[test]
    fn testing_status_is_parsed_and_moves_the_item() {
        let d1 = day_of(
            "1150818",
            "## project_a\n\n- `實作中` [deploy-preview：預覽環境部署](https://redmine.example.com/issues/40001)\n",
        );
        let d2 = day_of(
            "1150819",
            "## project_a\n\n- `測試中` [deploy-preview：推上 staging 等驗證](https://redmine.example.com/issues/40001)\n",
        );
        let mut days = vec![d1, d2];
        let items = derive_items(&mut days);

        assert_eq!(days[1].entries[0].status.as_deref(), Some("testing"));
        assert_eq!(days[1].entries[0].item.as_deref(), Some("deploy-preview"));
        let it = items.iter().find(|i| i.id == "deploy-preview").unwrap();
        assert_eq!(it.status, "testing");
        assert_eq!(it.since, "1150819");
    }

    /// 沒有 slug、也歸不到別人的 `測試中` 條目，一樣自己成為一支工作項目
    #[test]
    fn testing_entry_without_a_slug_becomes_its_own_item() {
        let md = "## project_b\n\n- `測試中` [feat: 上傳路徑改由後台設定](http://gitlab.example.com/group/project_b/-/merge_requests/392)\n";
        let mut days = vec![day_of("1150819", md)];
        let items = derive_items(&mut days);

        let id = days[0].entries[0].item.clone().expect("帶生命週期狀態就該歸戶");
        assert!(id.starts_with("auto-"), "fallback id 格式不對：{}", id);
        assert_eq!(items.iter().find(|i| i.id == id).unwrap().status, "testing");
    }

    /// 流程順序＝看板欄位順序：測試中夾在實作中與待合併中間
    #[test]
    fn testing_sits_between_building_and_review() {
        let ids: Vec<&str> = crate::model::STATUSES.iter().map(|s| s.id).collect();
        let pos = |id: &str| ids.iter().position(|x| *x == id).unwrap();
        assert!(pos("building") < pos("testing"), "測試中要排在實作中後面");
        assert!(pos("testing") < pos("review"), "測試中要排在待合併前面");
        let st = crate::model::status_by_zh("測試中").expect("狀態表裡要有測試中");
        assert_eq!(st.id, "testing");
        assert!(st.lifecycle, "測試中要算生命週期狀態");
    }

    #[test]
    fn only_seven_digit_filenames_count() {
        assert!(is_log_file("1150817.md"));
        assert!(!is_log_file("115081.md"));
        assert!(!is_log_file("README.md"));
    }
}
