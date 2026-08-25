//! 前端會呼叫的指令。

use std::path::{Path, PathBuf};

use chrono::{Datelike, Local};
use serde::Serialize;
use tauri_plugin_dialog::DialogExt;

use crate::index;
use crate::model::{Day, StatusDto, Workspace};
use crate::parser;
use crate::store::{self, Settings, Todo};

/// 今天的西元 8 碼，例如 20260821
pub fn today_code() -> String {
    let d = Local::now().date_naive();
    format!("{}{:02}{:02}", d.year(), d.month(), d.day())
}

#[derive(Serialize)]
pub struct SettingsDto {
    pub folder: String,
    pub folder_exists: bool,
    pub default_folder: String,
    pub config_dir: String,
    pub gitlab_base: String,
    pub redmine_base: String,
    /// token 本身不送去前端，只說有沒有設好
    pub gitlab_token_set: bool,
    pub redmine_token_set: bool,
}

fn to_dto(s: &Settings) -> SettingsDto {
    SettingsDto {
        folder: s.folder.clone(),
        folder_exists: Path::new(&s.folder).is_dir(),
        default_folder: store::default_folder(),
        config_dir: store::config_dir_display(),
        gitlab_base: s.gitlab_base.clone(),
        redmine_base: s.redmine_base.clone(),
        gitlab_token_set: !s.gitlab_token.trim().is_empty(),
        redmine_token_set: !s.redmine_token.trim().is_empty(),
    }
}

#[tauri::command]
pub fn load_settings() -> SettingsDto {
    to_dto(&store::load_settings())
}

#[tauri::command]
pub fn save_settings(folder: String) -> Result<SettingsDto, String> {
    let mut s = store::load_settings();
    s.folder = folder;
    store::save_settings(&s).map_err(|e| e.to_string())?;
    Ok(to_dto(&s))
}

/// 存外部服務的位址與 token。
///
/// token 留空代表「不要動原本那把」——因為畫面上本來就不會把已存的 token 顯示出來，
/// 使用者只改位址時不該把 token 洗掉。要清掉請用 [`clear_token`]。
#[tauri::command]
pub fn save_external_settings(
    gitlab_base: String,
    gitlab_token: String,
    redmine_base: String,
    redmine_token: String,
) -> Result<SettingsDto, String> {
    let mut s = store::load_settings();
    s.gitlab_base = gitlab_base.trim().trim_end_matches('/').to_string();
    s.redmine_base = redmine_base.trim().trim_end_matches('/').to_string();
    if !gitlab_token.trim().is_empty() {
        s.gitlab_token = gitlab_token.trim().to_string();
    }
    if !redmine_token.trim().is_empty() {
        s.redmine_token = redmine_token.trim().to_string();
    }
    store::save_settings(&s).map_err(|e| e.to_string())?;
    Ok(to_dto(&s))
}

/// 清掉某個服務的 token（`gitlab` 或 `redmine`）
#[tauri::command]
pub fn clear_token(service: String) -> Result<SettingsDto, String> {
    let mut s = store::load_settings();
    match service.as_str() {
        "gitlab" => s.gitlab_token.clear(),
        "redmine" => s.redmine_token.clear(),
        other => return Err(format!("沒有這個服務：{}", other)),
    }
    store::save_settings(&s).map_err(|e| e.to_string())?;
    Ok(to_dto(&s))
}

/* ---------- 日誌規則 ----------
   日誌檔是 Claude Code 照 `~/.claude/CLAUDE.md` 裡的規則寫的，所以這裡編輯的就是那個檔的
   `# 每日工作日誌` 那一段。只有使用者按下按鈕才會寫，而且寫之前一定先備份。 */

#[derive(Serialize)]
pub struct RulesDto {
    /// 目前生效的規則本文
    pub text: String,
    /// 內建的預設值（來自 docs/worklog-rules.md，編譯期嵌進 binary）
    pub default_text: String,
    /// `~/.claude/CLAUDE.md` 裡現在有沒有這一段
    pub present: bool,
    /// 那個檔在哪
    pub path: String,
    /// 這次寫入前備份到哪；只有存檔才有值
    pub backup: Option<String>,
}

fn rules_dto(backup: Option<String>) -> RulesDto {
    let (text, present) = store::load_rules();
    RulesDto {
        text,
        default_text: store::default_rules(),
        present,
        path: store::claude_md_display(),
        backup,
    }
}

#[tauri::command]
pub fn load_rules() -> RulesDto {
    rules_dto(None)
}

/// 把規則寫進 `~/.claude/CLAUDE.md`。
///
/// 只換 `# 每日工作日誌` 那一段，其他內容不動；寫之前先備份成
/// `CLAUDE.md.bak-<西元8碼>-<時分秒>`。
#[tauri::command]
pub fn save_rules(text: String) -> Result<RulesDto, String> {
    let stamp = format!("{}-{}", today_code(), Local::now().format("%H%M%S"));
    let backup = store::save_rules(&text, &stamp).map_err(|e| e.to_string())?;
    Ok(rules_dto(backup))
}

/// 抓一個連結的內容，讓前端不用把使用者丟去瀏覽器。
///
/// 打 API 是會擋住的動作，丟到背景執行緒跑，畫面才不會卡住。
#[tauri::command]
pub async fn fetch_link(url: String) -> Result<crate::link::LinkContent, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = store::load_settings();
        crate::link::fetch(&url, &settings)
    })
    .await
    .map_err(|e| format!("背景工作沒跑完：{}", e))?
}

/// 抓描述／留言裡的一張圖，回傳 `data:` URI。
///
/// 附圖多半要帶 token 才拿得到，WebView 自己去要多半 401 或空白，所以由後端抓。
/// `page_url` 是那張圖所屬的議題／MR 連結，用來決定打哪一台、以及怎麼把相對路徑補成絕對網址。
/// 一則描述裡可能有好幾張，前端會各自呼叫，失敗的那張不影響其他張。
#[tauri::command]
pub async fn fetch_image(page_url: String, src: String) -> Result<crate::link::ImageData, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let settings = store::load_settings();
        crate::link::fetch_image(&page_url, &src, &settings)
    })
    .await
    .map_err(|e| format!("背景工作沒跑完：{}", e))?
}

/// 開系統的選資料夾視窗
#[tauri::command]
pub fn pick_folder(app: tauri::AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p: PathBuf| p.to_string_lossy().to_string())
}

/// 讀整個資料夾，一次把所有頁面要的資料算好。
#[tauri::command]
pub fn load_workspace() -> Workspace {
    let settings = store::load_settings();
    let folder = PathBuf::from(&settings.folder);
    let folder_exists = folder.is_dir();

    let (mut days, skipped) = if folder_exists {
        parser::scan(&folder)
    } else {
        (Vec::new(), vec![format!("資料夾不存在：{}", settings.folder)])
    };

    let items = parser::derive_items(&mut days);
    let projects = parser::project_list(&days);

    // 順手更新給 Claude Code 讀的 slug 索引。寫不出來不影響讀日誌，
    // 但要讓使用者看得到原因，所以塞進 skipped 一起回報。
    let mut skipped = skipped;
    if folder_exists {
        if let Err(e) = index::write(&folder, &items, &today_code()) {
            skipped.push(e);
        }
    }

    Workspace {
        folder: settings.folder,
        folder_exists,
        today: today_code(),
        days,
        items,
        projects,
        skipped,
    }
}

/// 在背景更新 `_items.md`。
///
/// `load_workspace()` 本身就會順手寫，但那要等前端把畫面載起來才會被呼叫；
/// 索引是給 Claude Code 讀的，只要 app 跑起來就該是最新的，不該跟畫面綁在一起。
///
/// **一定要在背景執行緒跑，而且要等 tauri runtime 起來之後。**
/// 掃資料夾會 `opendir()` 使用者的文件夾，macOS 可能要跳權限對話框——
/// 在 `main()` 裡直接呼叫的話，視窗系統還沒起來，那個對話框顯示不出來，
/// app 就永遠卡在啟動階段（這個坑踩過：process 活著但一個視窗都開不出來）。
pub fn refresh_index_in_background() {
    std::thread::spawn(|| {
        let _ = load_workspace();
    });
}

/// 看板改狀態的結果，讓前端知道寫到哪個檔、是改了既有那一行還是新增一行。
#[derive(Serialize)]
pub struct MoveResult {
    /// 寫進哪一天（西元 8 碼）
    pub code: String,
    pub file: String,
    /// 新狀態的中文標籤，直接拿去顯示
    pub status_zh: String,
    /// true = 改了今天既有那一行的狀態標籤；false = 新增了一行
    pub updated: bool,
    /// 這次順手把檔案建出來
    pub created: bool,
    /// 本來就已經是這個狀態，什麼都沒動
    pub unchanged: bool,
    /// 寫完重讀的結果，前端不用再問一次
    pub workspace: Workspace,
}

/// 看板把卡片拖到別欄（或用卡片上的移動選單）：**直接改今天的 md**。
///
/// - 今天的檔案裡已經有這支工作項目的行 → 只把行首的狀態標籤換掉，
///   標題、連結、括號補充一個字都不動
/// - 今天還沒有 → 在對應的 `## 專案` 區塊新增一行，寫法跟日誌檔既有的一致
#[tauri::command]
pub fn move_item(item_id: String, status_id: String) -> Result<MoveResult, String> {
    let settings = store::load_settings();
    let folder = PathBuf::from(&settings.folder);
    if settings.folder.trim().is_empty() {
        return Err("還沒設定日誌資料夾，去設定頁選一個".into());
    }
    if !folder.is_dir() {
        return Err(format!("日誌資料夾不存在：{}", settings.folder));
    }

    let st = crate::model::status_by_id(&status_id)
        .ok_or_else(|| format!("沒有這個狀態：{}", status_id))?;

    let ws = load_workspace();
    let item = ws
        .items
        .iter()
        .find(|i| i.id == item_id)
        .ok_or_else(|| format!("找不到工作項目：{}", item_id))?;

    let code = today_code();
    let file = parser::log_path(Path::new(""), &code)
        .to_string_lossy()
        .to_string();

    if item.status == status_id {
        return Ok(MoveResult {
            code,
            file,
            status_zh: st.zh.to_string(),
            updated: false,
            created: false,
            unchanged: true,
            workspace: ws,
        });
    }

    let written = move_item_in(&folder, &code, &ws.days, item, &status_id)?;

    Ok(MoveResult {
        code,
        file,
        status_zh: st.zh.to_string(),
        updated: written.updated,
        created: written.created,
        unchanged: false,
        workspace: load_workspace(),
    })
}

/// 定位＋寫檔，`move_item` 的核心。抽出來讓測試可以指到臨時資料夾。
fn move_item_in(
    folder: &Path,
    code: &str,
    days: &[Day],
    item: &crate::model::Item,
    status_id: &str,
) -> Result<Written, String> {
    // 已合併、待審查看 MR，其餘看議題
    let url = match status_id {
        "review" | "archived" => item.mr.clone().or_else(|| item.issue.clone()),
        _ => item.issue.clone(),
    };

    // 新增的那一行寫成 `slug：描述`，下次重讀才會歸到同一支工作項目。
    // `auto-` 開頭的 id 是雜湊算出來的，不是人取的名字，寫進 md 只是噪音；
    // 那種項目本來就是靠連結（或專案＋標題）歸戶的，不加前綴一樣認得出來。
    let title = if item.id.starts_with("auto-") || item.title.starts_with(&format!("{}：", item.id))
    {
        item.title.clone()
    } else {
        format!("{}：{}", item.id, item.title)
    };

    // 今天已經有這支項目的行就改那一行，只動狀態標籤
    let at = days
        .iter()
        .find(|d| d.code == code)
        .and_then(|d| line_of_item(d, &item.id));

    write_status_in(
        folder,
        code,
        at,
        &item.project,
        status_id,
        &title,
        url.as_deref().unwrap_or(""),
    )
}

/// 今天的檔案裡，屬於這支工作項目的是第幾行。
///
/// 優先取**最後一筆帶生命週期狀態**的行——工作項目目前的狀態就是那一行決定的，
/// 改它才對得上。都沒有的話取最後一筆屬於它的行（那行還沒標狀態，改的時候補上）。
fn line_of_item(day: &Day, item_id: &str) -> Option<usize> {
    let mine: Vec<&crate::model::Entry> = day
        .entries
        .iter()
        .filter(|e| e.item.as_deref() == Some(item_id))
        .collect();

    mine.iter()
        .rev()
        .find(|e| {
            e.status
                .as_deref()
                .and_then(crate::model::status_by_id)
                .map(|s| s.lifecycle)
                .unwrap_or(false)
        })
        .or_else(|| mine.last())
        .map(|e| e.line)
}

#[tauri::command]
pub fn status_table() -> Vec<StatusDto> {
    crate::model::status_table()
}

/* ---------- 自訂狀態 ----------
   內建八個寫死在 model.rs，使用者可以自己加。加出來的狀態跟內建的一樣：
   看板有它的欄、卡片可以拖進去、日誌檔裡寫的就是它的中文標籤。 */

/// 自動配色用的候選盤，跟內建八色錯開。用完就從頭再來。
const PALETTE: &[(&str, &str, &str)] = &[
    ("#ff2d55", "#ffe0e6", "#3d1420"), // 粉紅
    ("#5856d6", "#e5e5fa", "#1e1d40"), // 靛藍
    ("#00c7be", "#d5f5f3", "#0f3330"), // 薄荷
    ("#a2845e", "#f0e9e0", "#33291f"), // 棕
    ("#d946ef", "#f9e2fd", "#3a1240"), // 洋紅
    ("#64748b", "#e7eaef", "#262b33"), // 石板
];

/// 挑一個還沒用過的顏色；全用過了就照數量輪一輪。
fn next_color(table: &[StatusDto]) -> crate::model::StatusColor {
    let used: Vec<&str> = table
        .iter()
        .filter_map(|s| s.color.as_ref().map(|c| c.dot.as_str()))
        .collect();
    let pick = PALETTE
        .iter()
        .find(|(dot, _, _)| !used.contains(dot))
        .unwrap_or(&PALETTE[used.len() % PALETTE.len()]);
    crate::model::StatusColor {
        dot: pick.0.to_string(),
        tint: pick.1.to_string(),
        dtint: pick.2.to_string(),
    }
}

/// 新狀態的 id：程式內部用，日誌檔裡寫的是中文標籤，所以取個不會撞的就好。
fn next_id(table: &[StatusDto]) -> String {
    for n in 1..1000 {
        let id = format!("custom{}", n);
        if !table.iter().any(|s| s.id == id) {
            return id;
        }
    }
    "custom".to_string()
}

/// 新增一個狀態，插在 `after_id` 那一欄後面（空字串＝排到最後）。
///
/// 回傳更新後的整張表；順序就是看板由左到右的欄序。
#[tauri::command]
pub fn add_status(zh: String, hint: String, after_id: String) -> Result<Vec<StatusDto>, String> {
    let zh = zh.trim().to_string();
    if zh.is_empty() {
        return Err("狀態名稱不能空白".into());
    }
    if zh.contains('`') {
        return Err("狀態名稱不能有反引號（`），那是日誌檔裡的標籤符號".into());
    }
    let mut table = crate::model::status_table();
    if table.iter().any(|s| s.zh == zh) {
        return Err(format!("已經有「{}」這個狀態了", zh));
    }

    let st = StatusDto {
        id: next_id(&table),
        label: zh.clone(),
        zh,
        hint: hint.trim().to_string(),
        lifecycle: true,
        branch: false,
        color: Some(next_color(&table)),
        builtin: false,
    };

    let at = match table.iter().position(|s| s.id == after_id) {
        Some(i) => i + 1,
        None => table.len(),
    };
    table.insert(at, st);

    store::save_statuses(&table).map_err(|e| e.to_string())?;
    crate::model::set_table(table.clone());
    Ok(table)
}

/// 把新狀態插進日誌規則後的完整規則本文。
///
/// 只是算給前端看的預覽，**不寫檔**——要真的生效得使用者再按一次
/// 「寫進規則」，那條路走既有的 `save_rules`。
#[tauri::command]
pub fn rules_with_status(zh: String, hint: String, after_zh: String) -> RulesDto {
    let (text, present) = store::load_rules();
    let next = store::insert_status_into_rules(&text, zh.trim(), hint.trim(), after_zh.trim());
    RulesDto {
        text: next,
        default_text: store::default_rules(),
        present,
        path: store::claude_md_display(),
        backup: None,
    }
}

#[derive(Serialize)]
pub struct UpdateStatusResult {
    pub table: Vec<StatusDto>,
    /// 規則有沒有跟著改；沒裝規則就是 false
    pub rules_changed: bool,
    pub backup: Option<String>,
}

/// 改一個狀態的名稱與說明（內建的也可以改）。
///
/// 使用者按下「儲存」就會一併把 `~/.claude/CLAUDE.md` 裡的規則改名
/// （狀態表那一列、生命週期那行、其他提到它的地方），寫之前先備份。
/// 日誌檔一個字都不動——舊標籤的行會留在原地，改名後 app 就不認得它們。
#[tauri::command]
pub fn update_status(id: String, zh: String, hint: String) -> Result<UpdateStatusResult, String> {
    let zh = zh.trim().to_string();
    let hint = hint.trim().to_string();
    if zh.is_empty() {
        return Err("狀態名稱不能空白".into());
    }
    if zh.contains('`') {
        return Err("狀態名稱不能有反引號（`），那是日誌檔裡的標籤符號".into());
    }
    let mut table = crate::model::status_table();
    let at = table
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| format!("沒有這個狀態：{}", id))?;
    if table.iter().any(|s| s.id != id && s.zh == zh) {
        return Err(format!("已經有「{}」這個狀態了", zh));
    }
    let old_zh = table[at].zh.clone();
    let old_hint = table[at].hint.clone();
    if old_zh == zh && old_hint == hint {
        return Ok(UpdateStatusResult { table, rules_changed: false, backup: None });
    }
    table[at].zh = zh.clone();
    table[at].label = zh.clone();
    table[at].hint = hint.clone();
    store::save_statuses(&table).map_err(|e| e.to_string())?;
    crate::model::set_table(table.clone());

    let (text, present) = store::load_rules();
    if !present {
        return Ok(UpdateStatusResult { table, rules_changed: false, backup: None });
    }
    let next = store::rename_status_in_rules(&text, &old_zh, &zh, &hint);
    if next == text {
        return Ok(UpdateStatusResult { table, rules_changed: false, backup: None });
    }
    let stamp = format!("{}-{}", today_code(), Local::now().format("%H%M%S"));
    let backup = store::save_rules(&next, &stamp).map_err(|e| e.to_string())?;
    Ok(UpdateStatusResult { table, rules_changed: true, backup })
}

/// 刪掉一個自訂狀態。內建的八個刪不得。
///
/// 日誌檔一個字都不動——已經寫成那個標籤的行留在原地，只是 app 之後不認得它。
#[tauri::command]
pub fn delete_status(id: String) -> Result<Vec<StatusDto>, String> {
    let mut table = crate::model::status_table();
    let at = table
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| format!("沒有這個狀態：{}", id))?;
    if table[at].builtin {
        return Err(format!("「{}」是內建狀態，不能刪", table[at].zh));
    }
    table.remove(at);
    store::save_statuses(&table).map_err(|e| e.to_string())?;
    crate::model::set_table(table.clone());
    Ok(table)
}

#[tauri::command]
pub fn load_todos() -> Vec<Todo> {
    store::load_todos()
}

#[tauri::command]
pub fn save_todos(todos: Vec<Todo>) -> Result<(), String> {
    store::save_todos(&todos).map_err(|e| e.to_string())
}

/// 用系統瀏覽器開連結
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    open::that(url).map_err(|e| e.to_string())
}

/// 用系統預設程式開某一天的日誌檔（唯讀 app，編輯交給編輯器）
#[tauri::command]
pub fn open_log_file(code: String) -> Result<(), String> {
    let settings = store::load_settings();
    let path = parser::log_path(&PathBuf::from(settings.folder), &code);
    if !path.is_file() {
        return Err(format!("找不到檔案：{}", path.display()));
    }
    open::that(path).map_err(|e| e.to_string())
}

/* ---------- 寫進今天的日誌檔 ----------
   會動到 .md 的只有這一區：TODO 頁的「加到今日日誌」與看板改狀態。
   寫法都很保守：插一行，或只換某一行行首的狀態標籤，其他行一個字都不動，也不重排。 */

/// 寫入結果，讓前端知道是新建檔案、真的寫進去了、還是那一行本來就在。
#[derive(Serialize)]
pub struct AppendResult {
    /// 西元 8 碼
    pub code: String,
    pub file: String,
    pub project: String,
    /// 實際寫進去（或本來就存在）的那一行 markdown
    pub line: String,
    /// 那一行已經在檔案裡，這次沒有再寫一次
    pub duplicate: bool,
    /// 這次順手把檔案建出來
    pub created: bool,
}

/// 把 TODO 排成日誌檔裡的那一行，寫法要跟 `js/store.js` 的 `todoMarkdown()` 一致。
fn entry_line(status: &str, text: &str, url: &str) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("內容是空的，沒東西可以寫".into());
    }
    let status = status.trim();
    let tag = if status.is_empty() {
        String::new()
    } else {
        let st = crate::model::status_by_id(status)
            .ok_or_else(|| format!("沒有這個狀態：{}", status))?;
        format!("`{}` ", st.zh)
    };
    let url = url.trim();
    let body = if url.is_empty() {
        text.to_string()
    } else {
        format!("[{}]({})", text, url)
    };
    Ok(format!("- {}{}", tag, body))
}

/// 把一行併進日誌內容裡（純函式，方便測試）。
///
/// - 檔案是空的：直接開一個 `## 專案` 區塊
/// - 有該專案區塊：接在區塊裡**最後一個 bullet** 後面
/// - 沒有該專案區塊：在檔尾新增一個區塊
///
/// 回傳 `None` 代表一模一樣的行已經在檔案裡，不要重複寫。
fn merge_entry_line(text: &str, project: &str, line: &str) -> Option<String> {
    // 同一天同一行只寫一次
    if text.lines().any(|l| l.trim() == line) {
        return None;
    }

    let heading = format!("## {}", project);

    if text.trim().is_empty() {
        return Some(format!("{}\n\n{}\n", heading, line));
    }

    let mut lines: Vec<String> = text.lines().map(|s| s.to_string()).collect();

    match lines.iter().position(|l| l.trim() == heading) {
        Some(start) => {
            // 區塊到下一個 `## ` 標題為止，沒有下一個就到檔尾
            let end = lines
                .iter()
                .enumerate()
                .skip(start + 1)
                .find(|(_, l)| l.trim_start().starts_with("## "))
                .map(|(i, _)| i)
                .unwrap_or(lines.len());

            // 區塊裡最後一個 bullet 的下一行
            let mut at = None;
            for i in (start + 1..end).rev() {
                if lines[i].trim_start().starts_with("- ") {
                    at = Some(i + 1);
                    break;
                }
            }
            let at = match at {
                Some(i) => i,
                None => {
                    // 區塊裡還沒有任何 bullet：跳過標題後的空行再插進去
                    let mut i = start + 1;
                    while i < end && lines[i].trim().is_empty() {
                        i += 1;
                    }
                    // 區塊本來就是空的、後面還有別的區塊：補一行空白隔開
                    if i >= end && end < lines.len() {
                        lines.insert(end, String::new());
                    }
                    i
                }
            };
            lines.insert(at, line.to_string());
        }
        None => {
            // 檔尾新增一個專案區塊，前面留一行空白
            while lines.last().map(|l| l.trim().is_empty()).unwrap_or(false) {
                lines.pop();
            }
            lines.push(String::new());
            lines.push(heading);
            lines.push(String::new());
            lines.push(line.to_string());
        }
    }

    let mut out = lines.join("\n");
    out.push('\n');
    Some(out)
}

/// 寫檔。日誌檔在 `<年>/<月>/` 底下，那兩層資料夾可能還不存在，先補出來。
fn write_log(path: &Path, text: String) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("建不出資料夾 {}：{}", dir.display(), e))?;
    }
    std::fs::write(path, text).map_err(|e| format!("寫入失敗 {}：{}", path.display(), e))
}

/// 真正動檔案的部分，抽出來讓測試可以指到臨時資料夾。
fn append_entry_in(
    folder: &Path,
    code: &str,
    project: &str,
    status: &str,
    text: &str,
    url: &str,
) -> Result<AppendResult, String> {
    let project = {
        let p = project.trim();
        if p.is_empty() { "其他" } else { p }
    };
    let line = entry_line(status, text, url)?;

    let path = parser::log_path(folder, code);
    let file = path
        .strip_prefix(folder)
        .unwrap_or(&path)
        .to_string_lossy()
        .to_string();
    let created = !path.exists();
    let existing = if created {
        String::new()
    } else {
        std::fs::read_to_string(&path)
            .map_err(|e| format!("讀不到 {}：{}", path.display(), e))?
    };

    let result = AppendResult {
        code: code.to_string(),
        file: file.clone(),
        project: project.to_string(),
        line: line.clone(),
        duplicate: false,
        created,
    };

    match merge_entry_line(&existing, project, &line) {
        None => Ok(AppendResult { duplicate: true, created: false, ..result }),
        Some(next) => {
            write_log(&path, next)?;
            Ok(result)
        }
    }
}

/// 把一行的狀態標籤換成 `zh`（純函式）。
///
/// - 行首已經有狀態標籤：**只換標籤裡的字**，縮排、bullet 記號、標籤後面的空白、
///   標題、連結、括號補充全部原樣保留
/// - 還沒有標籤：在 bullet 記號後面補一個
/// - 不是 bullet 的行：原樣不動（正常不會發生，防呆）
fn set_line_status(line: &str, zh: &str) -> String {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(indent_len);

    let marker = match rest.chars().next() {
        Some(c) if c == '-' || c == '*' => c,
        _ => return line.to_string(),
    };
    let after_marker = &rest[marker.len_utf8()..];
    let gap_len = after_marker.len() - after_marker.trim_start().len();
    let (gap, body) = after_marker.split_at(gap_len);
    if gap.is_empty() {
        return line.to_string();
    }

    // 行首的行內程式碼、而且內容是已知狀態，才算舊標籤（判斷跟 parser.rs 一致）
    if let Some(after) = body.strip_prefix('`') {
        if let Some(end) = after.find('`') {
            if crate::model::status_by_zh(after[..end].trim()).is_some() {
                let tail = &after[end + 1..];
                return format!("{}{}{}`{}`{}", indent, marker, gap, zh, tail);
            }
        }
    }
    format!("{}{}{}`{}` {}", indent, marker, gap, zh, body)
}

/// 把第 `line_no` 行（0 起算）的狀態標籤換掉，其他行原樣保留（純函式）。
///
/// 行號超出範圍就回 `None`，讓呼叫端退回「新增一行」那條路。
fn replace_status_at(text: &str, line_no: usize, zh: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    if line_no >= lines.len() {
        return None;
    }
    let replaced = set_line_status(lines[line_no], zh);

    let mut out = String::new();
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(if i == line_no { replaced.as_str() } else { l });
    }
    // 原本檔尾有沒有換行就照舊，不多不少
    if text.ends_with('\n') {
        out.push('\n');
    }
    Some(out)
}

/// 看板改狀態實際寫了什麼
struct Written {
    /// 改了既有那一行的標籤（false 代表新增了一行）
    updated: bool,
    /// 這次順手把檔案建出來
    created: bool,
}

/// 把工作項目的新狀態寫進某一天的檔案，抽出來讓測試可以指到臨時資料夾。
///
/// `at` 是今天檔案裡屬於這支項目的行號；有就改那一行，沒有（或行號對不上）就新增一行。
fn write_status_in(
    folder: &Path,
    code: &str,
    at: Option<usize>,
    project: &str,
    status: &str,
    text: &str,
    url: &str,
) -> Result<Written, String> {
    let st = crate::model::status_by_id(status)
        .ok_or_else(|| format!("沒有這個狀態：{}", status))?;

    let path = parser::log_path(folder, code);
    let created = !path.exists();
    let existing = if created {
        String::new()
    } else {
        std::fs::read_to_string(&path)
            .map_err(|e| format!("讀不到 {}：{}", path.display(), e))?
    };

    if let Some(i) = at {
        if let Some(next) = replace_status_at(&existing, i, &st.zh) {
            if next != existing {
                write_log(&path, next)?;
            }
            return Ok(Written { updated: true, created: false });
        }
    }

    let project = {
        let p = project.trim();
        if p.is_empty() { "其他" } else { p }
    };
    let line = entry_line(status, text, url)?;
    if let Some(next) = merge_entry_line(&existing, project, &line) {
        write_log(&path, next)?;
    }
    Ok(Written { updated: false, created })
}

/// 把一筆 TODO 寫進**今天**的 `<年>/<月>/<西元8碼>.md`。
#[tauri::command]
pub fn append_entry(
    project: String,
    status: String,
    text: String,
    url: String,
) -> Result<AppendResult, String> {
    let settings = store::load_settings();
    let folder = PathBuf::from(&settings.folder);
    if !folder.is_dir() {
        return Err(format!("日誌資料夾不存在：{}", settings.folder));
    }
    append_entry_in(&folder, &today_code(), &project, &status, &text, &url)
}

/* ---------- 線上更新 ----------
   真正的邏輯在 update.rs，這裡只是薄薄一層指令。
   錯誤訊息已經在後端翻成中文，前端直接顯示就好。 */

/// 目前跑的版本號與下載頁。前端不要自己寫死版本，一律問這裡。
#[tauri::command]
pub fn app_version(app: tauri::AppHandle) -> crate::update::AppVersion {
    crate::update::app_version(&app)
}

/// 問更新來源有沒有新版。只讀 latest.json，不下載更新檔。
#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> Result<crate::update::UpdateInfo, String> {
    crate::update::check(app).await
}

/// 下載並安裝新版。下載期間會一直丟 `worklog://update-progress` 事件出去。
///
/// 裝完不會自己重開，前端顯示完訊息再呼叫 [`restart_app`]。
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    crate::update::install(app).await
}

/// 重開 app，讓剛裝好的版本跑起來
#[tauri::command]
pub fn restart_app(app: tauri::AppHandle) {
    crate::update::restart(app);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 每個測試自己一個臨時資料夾，絕對不碰使用者真正的日誌
    fn temp_folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "worklog-app-test-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn entry_line_matches_the_log_file_style() {
        assert_eq!(
            entry_line("building", "search-message-history：對話紀錄搜尋", "https://redmine.example.com/issues/32979").unwrap(),
            "- `實作中` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)"
        );
        assert_eq!(entry_line("", "議題對帳", "").unwrap(), "- 議題對帳");
        assert_eq!(entry_line("done", "議題對帳", "").unwrap(), "- `完成` 議題對帳");
        assert!(entry_line("nope", "隨便", "").is_err());
        assert!(entry_line("done", "   ", "").is_err());
    }

    #[test]
    fn empty_file_gets_a_fresh_project_section() {
        let out = merge_entry_line("", "project_a", "- `完成` 對帳").unwrap();
        assert_eq!(out, "## project_a\n\n- `完成` 對帳\n");
    }

    #[test]
    fn new_line_goes_after_the_last_bullet_of_its_project() {
        let src = "## project_a\n\n- `完成` 甲\n- `完成` 乙\n\n## project_b\n\n- `完成` 丙\n";
        let out = merge_entry_line(src, "project_a", "- `完成` 丁").unwrap();
        assert_eq!(out, "## project_a\n\n- `完成` 甲\n- `完成` 乙\n- `完成` 丁\n\n## project_b\n\n- `完成` 丙\n");
    }

    #[test]
    fn unknown_project_gets_appended_as_a_new_section() {
        let src = "## project_a\n\n- `完成` 甲\n";
        let out = merge_entry_line(src, "其他", "- `完成` 乙").unwrap();
        assert_eq!(out, "## project_a\n\n- `完成` 甲\n\n## 其他\n\n- `完成` 乙\n");
    }

    /// 有標題但底下還沒有 bullet 的區塊
    #[test]
    fn section_without_bullets_still_takes_the_line() {
        let src = "## project_a\n\n## project_b\n\n- `完成` 甲\n";
        let out = merge_entry_line(src, "project_a", "- `完成` 乙").unwrap();
        assert_eq!(out, "## project_a\n\n- `完成` 乙\n\n## project_b\n\n- `完成` 甲\n");
    }

    #[test]
    fn identical_line_is_not_written_twice() {
        let src = "## project_a\n\n- `完成` 甲\n";
        assert!(merge_entry_line(src, "project_a", "- `完成` 甲").is_none());
    }

    /// 只插一行，別的行一個字都不能動
    #[test]
    fn other_lines_are_left_untouched() {
        let src = "# 20260818\n\n## project_a\n\n- `完成` 甲（備註裡有 `反引號`）\n\n> 隨手筆記\n\n## project_b\n\n- 沒標狀態的一行\n";
        let out = merge_entry_line(src, "project_b", "- `完成` 乙").unwrap();
        for line in src.lines() {
            assert!(out.contains(line), "原本的行不見了：{}", line);
        }
        assert_eq!(out.lines().count(), src.lines().count() + 1);
    }

    #[test]
    fn append_entry_in_creates_then_appends() {
        let dir = temp_folder("append");
        let r = append_entry_in(&dir, "20260818", "project_a", "building", "甲", "").unwrap();
        assert!(r.created);
        assert!(!r.duplicate);
        let path = parser::log_path(&dir, "20260818");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "## project_a\n\n- `實作中` 甲\n");

        let r = append_entry_in(&dir, "20260818", "project_a", "review", "乙", "http://x/1").unwrap();
        assert!(!r.created);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "## project_a\n\n- `實作中` 甲\n- `待合併` [乙](http://x/1)\n"
        );

        // 一樣的行按第二次：不寫、回報 duplicate
        let r = append_entry_in(&dir, "20260818", "project_a", "review", "乙", "http://x/1").unwrap();
        assert!(r.duplicate);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "## project_a\n\n- `實作中` 甲\n- `待合併` [乙](http://x/1)\n"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn append_entry_in_defaults_to_the_other_project() {
        let dir = temp_folder("other");
        let r = append_entry_in(&dir, "20260818", "  ", "", "隨手記", "").unwrap();
        assert_eq!(r.project, "其他");
        assert_eq!(
            std::fs::read_to_string(parser::log_path(&dir, "20260818")).unwrap(),
            "## 其他\n\n- 隨手記\n"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /* ---------- 看板改狀態：直接寫進當天的 md ---------- */

    /// 走一次跟 `move_item` 一樣的流程：重讀資料夾 → 歸戶 → 定位 → 寫檔。
    /// 差別只在資料夾指到臨時目錄，絕對不碰使用者真正的日誌。
    fn drag(dir: &Path, code: &str, item_id: &str, status: &str) -> Written {
        let (mut days, _) = parser::scan(dir);
        let items = parser::derive_items(&mut days);
        let item = items
            .iter()
            .find(|i| i.id == item_id)
            .unwrap_or_else(|| panic!("找不到工作項目：{}", item_id));
        move_item_in(dir, code, &days, item, status).unwrap()
    }

    /// 重讀某一天，看某支工作項目現在的狀態
    fn status_of(dir: &Path, item_id: &str) -> String {
        let (mut days, _) = parser::scan(dir);
        parser::derive_items(&mut days)
            .into_iter()
            .find(|i| i.id == item_id)
            .unwrap_or_else(|| panic!("找不到工作項目：{}", item_id))
            .status
    }

    fn write(dir: &Path, code: &str, text: &str) {
        let path = parser::log_path(dir, code);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn read(dir: &Path, code: &str) -> String {
        std::fs::read_to_string(parser::log_path(dir, code)).unwrap()
    }

    /// 今天已經有那一行：只換狀態標籤，其他字元一個都不能變
    #[test]
    fn moving_an_item_only_swaps_the_status_tag_of_todays_line() {
        let dir = temp_folder("swap");
        let src = "# 20260818\n\n## project_a\n\n- `暫存` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)（等排程，MR 還沒開）\n\n> 隨手筆記\n";
        write(&dir, "20260818", src);

        let w = drag(&dir, "20260818", "search-message-history", "building");
        assert!(w.updated, "應該是改既有那一行，不是新增");
        assert!(!w.created);

        let out = read(&dir, "20260818");
        assert_eq!(out, src.replace("`暫存`", "`實作中`"));
        assert_eq!(out.lines().count(), src.lines().count(), "行數不該變");
        assert_eq!(status_of(&dir, "search-message-history"), "building");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 同樣的行出現兩次時，靠行號定位，不是字串比對
    #[test]
    fn the_right_line_is_picked_when_two_lines_look_alike() {
        let dir = temp_folder("dup");
        let src = "## project_a\n\n- `暫存` [dup-slug：一樣的標題](https://redmine.example.com/issues/1)\n\n## project_b\n\n- `暫存` [dup-slug：一樣的標題](https://redmine.example.com/issues/1)\n";
        write(&dir, "20260818", src);

        drag(&dir, "20260818", "dup-slug", "building");

        let out = read(&dir, "20260818");
        assert_eq!(out.matches("`實作中`").count(), 1, "只該有一行被改到");
        assert_eq!(out.matches("`暫存`").count(), 1, "另一行要原封不動");
        // 定位取的是最後一筆帶生命週期狀態的行
        assert!(out.ends_with("## project_b\n\n- `實作中` [dup-slug：一樣的標題](https://redmine.example.com/issues/1)\n"));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 那一行本來沒標狀態：補一個上去，標題與連結不動
    #[test]
    fn a_line_without_a_status_tag_gets_one() {
        let dir = temp_folder("notag");
        // 沒標狀態的行歸不了戶，所以先用同一個連結在別天建立這支項目
        write(&dir, "20260817", "## project_a\n\n- `提案中` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)\n");
        write(&dir, "20260818", "## project_a\n\n- [開分支動工](https://redmine.example.com/issues/32979)\n");

        let w = drag(&dir, "20260818", "search-message-history", "building");
        assert!(w.updated);
        assert_eq!(
            read(&dir, "20260818"),
            "## project_a\n\n- `實作中` [開分支動工](https://redmine.example.com/issues/32979)\n"
        );
        assert_eq!(status_of(&dir, "search-message-history"), "building");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 今天還沒有這支項目：在對的專案區塊補一行
    #[test]
    fn an_item_without_a_line_today_gets_a_new_one_in_its_project() {
        let dir = temp_folder("newline");
        write(&dir, "20260817", "## project_a\n\n- `暫存` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)\n");
        write(&dir, "20260818", "## project_a\n\n- `完成` 議題對帳\n\n## project_b\n\n- `完成` 別的事\n");

        let w = drag(&dir, "20260818", "search-message-history", "review");
        assert!(!w.updated, "今天還沒有那一行，應該是新增");
        assert!(!w.created);
        assert_eq!(
            read(&dir, "20260818"),
            "## project_a\n\n- `完成` 議題對帳\n- `待合併` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)\n\n## project_b\n\n- `完成` 別的事\n"
        );
        assert_eq!(status_of(&dir, "search-message-history"), "review");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 今天的檔案裡沒有那個專案區塊：在檔尾開一個
    #[test]
    fn a_missing_project_section_is_added_at_the_end() {
        let dir = temp_folder("newsection");
        write(&dir, "20260817", "## project_a\n\n- `暫存` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)\n");
        write(&dir, "20260818", "## 其他\n\n- `完成` 環境設定\n");

        drag(&dir, "20260818", "search-message-history", "building");

        assert_eq!(
            read(&dir, "20260818"),
            "## 其他\n\n- `完成` 環境設定\n\n## project_a\n\n- `實作中` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)\n"
        );
        assert_eq!(status_of(&dir, "search-message-history"), "building");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 今天的檔案還不存在：建一個
    #[test]
    fn todays_file_is_created_when_missing() {
        let dir = temp_folder("create");
        write(&dir, "20260817", "## project_a\n\n- `暫存` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)\n");

        let w = drag(&dir, "20260818", "search-message-history", "testing");
        assert!(w.created);
        assert!(!w.updated);
        assert_eq!(
            read(&dir, "20260818"),
            "## project_a\n\n- `測試中` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)\n"
        );
        assert_eq!(status_of(&dir, "search-message-history"), "testing");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// 連拖兩次：第二次改的是第一次寫出來的那一行，不會再長一行
    #[test]
    fn dragging_twice_in_a_day_keeps_a_single_line() {
        let dir = temp_folder("twice");
        write(&dir, "20260817", "## project_a\n\n- `暫存` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)\n");

        drag(&dir, "20260818", "search-message-history", "building");
        let w = drag(&dir, "20260818", "search-message-history", "review");
        assert!(w.updated);
        assert_eq!(read(&dir, "20260818").matches("- `").count(), 1, "同一天只該有一行");
        assert_eq!(status_of(&dir, "search-message-history"), "review");

        std::fs::remove_dir_all(&dir).ok();
    }

    /// `auto-` 開頭的 id 是雜湊，不寫進標題；靠連結一樣歸到同一支
    #[test]
    fn auto_items_do_not_get_the_hash_written_into_the_title() {
        let dir = temp_folder("auto");
        write(&dir, "20260817", "## project_b\n\n- `實作中` [feat: 上傳路徑改由後台設定](http://gitlab.example.com/group/project_b/-/merge_requests/392)\n");

        let (mut days, _) = parser::scan(&dir);
        let items = parser::derive_items(&mut days);
        let id = items[0].id.clone();
        assert!(id.starts_with("auto-"));

        drag(&dir, "20260818", &id, "review");
        let out = read(&dir, "20260818");
        assert!(!out.contains("auto-"), "雜湊 id 不該寫進日誌：{}", out);
        assert_eq!(
            out,
            "## project_b\n\n- `待合併` [feat: 上傳路徑改由後台設定](http://gitlab.example.com/group/project_b/-/merge_requests/392)\n"
        );
        assert_eq!(status_of(&dir, &id), "review", "重讀後還是同一支項目");

        std::fs::remove_dir_all(&dir).ok();
    }

    /* ---------- 換標籤的純函式 ---------- */

    #[test]
    fn set_line_status_only_touches_the_tag() {
        assert_eq!(
            set_line_status("- `暫存` [x：甲](http://a/1)（補充）", "實作中"),
            "- `實作中` [x：甲](http://a/1)（補充）"
        );
        // 沒有標籤就補一個
        assert_eq!(set_line_status("- [x：甲](http://a/1)", "實作中"), "- `實作中` [x：甲](http://a/1)");
        // 縮排、bullet 記號、標籤後面的空白都照舊
        assert_eq!(set_line_status(" * `暫存`  甲", "已歸檔"), " * `已歸檔`  甲");
        // 行首的反引號不是狀態就不算舊標籤，補一個在前面
        assert_eq!(set_line_status("- `~/.claude/CLAUDE.md` 改好了", "完成"), "- `完成` `~/.claude/CLAUDE.md` 改好了");
        // 不是 bullet 的行原樣不動
        assert_eq!(set_line_status("## project_a", "完成"), "## project_a");
    }

    #[test]
    fn replace_status_at_leaves_every_other_line_alone() {
        let src = "## project_a\n\n- `暫存` 甲\n- `暫存` 甲\n";
        let out = replace_status_at(src, 3, "實作中").unwrap();
        assert_eq!(out, "## project_a\n\n- `暫存` 甲\n- `實作中` 甲\n");
        // 行號超出範圍就讓呼叫端改走新增
        assert!(replace_status_at(src, 99, "實作中").is_none());
        // 檔尾本來沒有換行就不要補
        assert_eq!(replace_status_at("- `暫存` 甲", 0, "完成").unwrap(), "- `完成` 甲");
    }

    #[test]
    fn append_entry_in_rejects_a_bad_status_without_touching_the_file() {
        let dir = temp_folder("bad");
        assert!(append_entry_in(&dir, "20260818", "project_a", "nope", "甲", "").is_err());
        assert!(!parser::log_path(&dir, "20260818").exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
