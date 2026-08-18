//! 前端會呼叫的指令。

use std::path::{Path, PathBuf};

use chrono::{Datelike, Local};
use serde::Serialize;
use tauri_plugin_dialog::DialogExt;

use crate::model::{Day, Entry, PendingPoint, StatusDto, Workspace};
use crate::parser;
use crate::store::{self, Settings, Todo};

/// 今天的民國 7 碼
pub fn today_code() -> String {
    let d = Local::now().date_naive();
    format!("{}{:02}{:02}", d.year() - 1911, d.month(), d.day())
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
/// `CLAUDE.md.bak-<民國7碼>-<時分秒>`。
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
///
/// 還沒寫回 .md 的變更會併進當天的條目再一起推導，
/// 所以在看板上拖過的卡片，狀態立刻就是新的。
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

    let pending_map = store::load_pending();
    let mut pending: Vec<PendingPoint> = Vec::new();
    for (code, entries) in &pending_map {
        for e in entries {
            pending.push(PendingPoint { code: code.clone(), entry: e.clone() });
        }
        match days.iter_mut().find(|d| &d.code == code) {
            Some(day) => day.entries.extend(entries.iter().cloned()),
            None => days.push(Day {
                code: code.clone(),
                file: format!("{}.md", code),
                entries: entries.clone(),
            }),
        }
    }
    days.sort_by(|a, b| a.code.cmp(&b.code));

    let items = parser::derive_items(&mut days);
    let projects = parser::project_list(&days);

    Workspace {
        folder: settings.folder,
        folder_exists,
        today: today_code(),
        days,
        items,
        projects,
        skipped,
        pending,
    }
}

/// 看板上把卡片拖到別欄：不寫檔，只記一筆待寫回的變更。
///
/// 條目寫法要跟日誌檔一致，使用者複製出去就能直接貼。
#[tauri::command]
pub fn move_item(item_id: String, status_id: String) -> Result<Workspace, String> {
    let ws = load_workspace();
    let item = ws
        .items
        .iter()
        .find(|i| i.id == item_id)
        .ok_or_else(|| format!("找不到工作項目：{}", item_id))?;
    crate::model::status_by_id(&status_id)
        .ok_or_else(|| format!("沒有這個狀態：{}", status_id))?;

    if item.status == status_id {
        return Ok(ws);
    }

    // 已合併、待審查看 MR，其餘看議題
    let url = match status_id.as_str() {
        "review" | "archived" => item.mr.clone().or_else(|| item.issue.clone()),
        _ => item.issue.clone(),
    };

    // 寫成 `slug：描述`，跟日誌檔既有的寫法一致，
    // 貼回 md 之後下次讀還能歸到同一支工作項目
    let title = if item.title.starts_with(&format!("{}：", item.id)) {
        item.title.clone()
    } else {
        format!("{}：{}", item.id, item.title)
    };

    let entry = Entry {
        project: item.project.clone(),
        status: Some(status_id.clone()),
        title,
        url,
        note: None,
        item: Some(item.id.clone()),
        raw: String::new(),
        pending: true,
    };

    let today = today_code();
    let mut map = store::load_pending();
    map.entry(today).or_default().push(entry);
    store::save_pending(&map).map_err(|e| e.to_string())?;

    Ok(load_workspace())
}

/// 丟掉所有待寫回的變更（通常是已經請 Claude 寫進 md 了）
#[tauri::command]
pub fn clear_pending() -> Result<Workspace, String> {
    store::save_pending(&store::PendingMap::new()).map_err(|e| e.to_string())?;
    Ok(load_workspace())
}

#[tauri::command]
pub fn status_table() -> Vec<StatusDto> {
    crate::model::status_table()
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
    let path = PathBuf::from(settings.folder).join(format!("{}.md", code));
    if !path.is_file() {
        return Err(format!("找不到檔案：{}", path.display()));
    }
    open::that(path).map_err(|e| e.to_string())
}

/* ---------- 寫進今天的日誌檔 ----------
   這個 app 對日誌檔本來是唯讀的，只有這裡會寫。
   寫法很保守：只在既有內容裡插入一行，不重排、不改寫別的行。 */

/// 寫入結果，讓前端知道是新建檔案、真的寫進去了、還是那一行本來就在。
#[derive(Serialize)]
pub struct AppendResult {
    /// 民國 7 碼
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

    let file = format!("{}.md", code);
    let path = folder.join(&file);
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
            std::fs::write(&path, next)
                .map_err(|e| format!("寫入失敗 {}：{}", path.display(), e))?;
            Ok(result)
        }
    }
}

/// 把一筆 TODO 寫進**今天**的 `<民國7碼>.md`。
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
        let src = "# 1150818\n\n## project_a\n\n- `完成` 甲（備註裡有 `反引號`）\n\n> 隨手筆記\n\n## project_b\n\n- 沒標狀態的一行\n";
        let out = merge_entry_line(src, "project_b", "- `完成` 乙").unwrap();
        for line in src.lines() {
            assert!(out.contains(line), "原本的行不見了：{}", line);
        }
        assert_eq!(out.lines().count(), src.lines().count() + 1);
    }

    #[test]
    fn append_entry_in_creates_then_appends() {
        let dir = temp_folder("append");
        let r = append_entry_in(&dir, "1150818", "project_a", "building", "甲", "").unwrap();
        assert!(r.created);
        assert!(!r.duplicate);
        let path = dir.join("1150818.md");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "## project_a\n\n- `實作中` 甲\n");

        let r = append_entry_in(&dir, "1150818", "project_a", "review", "乙", "http://x/1").unwrap();
        assert!(!r.created);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "## project_a\n\n- `實作中` 甲\n- `待合併` [乙](http://x/1)\n"
        );

        // 一樣的行按第二次：不寫、回報 duplicate
        let r = append_entry_in(&dir, "1150818", "project_a", "review", "乙", "http://x/1").unwrap();
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
        let r = append_entry_in(&dir, "1150818", "  ", "", "隨手記", "").unwrap();
        assert_eq!(r.project, "其他");
        assert_eq!(std::fs::read_to_string(dir.join("1150818.md")).unwrap(), "## 其他\n\n- 隨手記\n");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn append_entry_in_rejects_a_bad_status_without_touching_the_file() {
        let dir = temp_folder("bad");
        assert!(append_entry_in(&dir, "1150818", "project_a", "nope", "甲", "").is_err());
        assert!(!dir.join("1150818.md").exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
