//! app 自己的檔案：設定與 TODO。
//!
//! 日誌資料夾是唯讀的，這裡寫的東西一律放在 app 自己的設定目錄，
//! 不會碰到使用者的日誌檔。

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::model::Entry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// 要讀哪個資料夾的 .md
    pub folder: String,
    /// 自架 GitLab 的位址，例如 http://gitlab.example.com；留空就照連結本身的主機
    #[serde(default)]
    pub gitlab_base: String,
    /// GitLab personal access token。機密，只存在這個檔案裡，不會送進日誌資料夾也不會印出來。
    #[serde(default)]
    pub gitlab_token: String,
    /// Redmine 位址，例如 https://redmine.example.com
    #[serde(default)]
    pub redmine_base: String,
    /// Redmine API key。同樣是機密。
    #[serde(default)]
    pub redmine_token: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            folder: default_folder(),
            gitlab_base: String::new(),
            gitlab_token: String::new(),
            redmine_base: String::new(),
            redmine_token: String::new(),
        }
    }
}

/// 預設就指到平常寫日誌的地方，第一次開就有東西看。
pub fn default_folder() -> String {
    let home = directories::UserDirs::new()
        .map(|d| d.home_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/"));
    home.join("Documents")
        .join("Obsidian Vault")
        .join("每日工作日誌")
        .to_string_lossy()
        .to_string()
}

fn config_dir() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("tw", "npust", "worklog-app")
        .context("找不到設定目錄")?;
    let dir = dirs.config_dir().to_path_buf();
    std::fs::create_dir_all(&dir).with_context(|| format!("建立設定目錄失敗：{}", dir.display()))?;
    Ok(dir)
}

fn settings_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("settings.json"))
}

fn todos_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("todos.json"))
}

pub fn load_settings() -> Settings {
    let path = match settings_path() {
        Ok(p) => p,
        Err(_) => return Settings::default(),
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Settings::default(),
    }
}

pub fn save_settings(s: &Settings) -> Result<()> {
    let path = settings_path()?;
    let text = serde_json::to_string_pretty(s)?;
    std::fs::write(&path, text).with_context(|| format!("寫入失敗：{}", path.display()))?;
    Ok(())
}

/* ---------- 日誌規則 ----------
   日誌檔是 Claude Code 照使用者層級提示詞寫出來的，而 Claude Code 讀的是
   `~/.claude/CLAUDE.md`。所以規則要真的生效，就得寫進那個檔——存在 app 自己的
   設定目錄沒有意義。

   這個 app 只認 `# 每日工作日誌` 那一段（到下一個同層級 `# ` 標題或檔尾為止），
   其他內容一律不碰。寫之前一定先備份。 */

/// 規則那一段的標題，兩邊（讀與寫）都靠它定位。
pub const RULES_HEADING: &str = "# 每日工作日誌";

/// 預設規則。release build 之後 docs/ 不一定在執行檔旁邊，所以在編譯期就嵌進 binary。
const RULES_DOC: &str = include_str!("../../docs/worklog-rules.md");

/// `docs/worklog-rules.md` 是一份說明文件，裡面才包著要貼進 CLAUDE.md 的規則本文，
/// 所以預設值要取的是文件裡 `# 每日工作日誌` 那一段，不是整份文件。
pub fn default_rules() -> String {
    extract_rules_section(RULES_DOC).unwrap_or_else(|| RULES_DOC.to_string())
}

/// Claude Code 的使用者層級提示詞檔
pub fn claude_md_path() -> Option<PathBuf> {
    directories::UserDirs::new().map(|d| d.home_dir().join(".claude").join("CLAUDE.md"))
}

pub fn claude_md_display() -> String {
    claude_md_path().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
}

/// 規則那一段在第幾行到第幾行（左閉右開）。找不到就是 `None`。
///
/// 段落從 `# 每日工作日誌` 那行開始，到下一個同層級 `# ` 標題之前，或檔尾。
/// `## ` 之類的下層標題不算結束。
fn rules_section_range(text: &str) -> Option<(usize, usize)> {
    let lines: Vec<&str> = text.lines().collect();
    let start = lines.iter().position(|l| l.trim() == RULES_HEADING)?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, l)| l.starts_with("# "))
        .map(|(i, _)| i)
        .unwrap_or(lines.len());
    Some((start, end))
}

/// 從整份 CLAUDE.md 抽出規則那一段（含標題行）。純函式。
pub fn extract_rules_section(text: &str) -> Option<String> {
    let (start, end) = rules_section_range(text)?;
    let lines: Vec<&str> = text.lines().collect();
    Some(lines[start..end].join("\n").trim_end().to_string())
}

/// 規則本文正規化：保證以 `# 每日工作日誌` 開頭，結尾沒有多餘空白。
/// 使用者不小心把標題刪掉時補回來，下次才找得到這一段。
fn normalized_rules(rules: &str) -> String {
    let body = rules.trim();
    let has_heading = body.lines().next().map(|l| l.trim() == RULES_HEADING).unwrap_or(false);
    if has_heading {
        body.to_string()
    } else {
        format!("{}\n\n{}", RULES_HEADING, body)
    }
}

/// 把新的規則放回整份 CLAUDE.md（純函式，方便測試）。
///
/// - 已經有那一段：**只換那一段**，前後一個字都不動（連段落之間空幾行都照舊）
/// - 沒有那一段：接在檔尾，前面補一行空白
/// - 空檔案：整份就是這一段
pub fn replace_rules_section(text: &str, rules: &str) -> String {
    let block = normalized_rules(rules);

    let out = match rules_section_range(text) {
        Some((start, end)) => {
            let lines: Vec<&str> = text.lines().collect();
            // 舊段落尾巴空了幾行就留幾行，後面那段的位置才不會被推上來
            let blanks = lines[start..end]
                .iter()
                .rev()
                .take_while(|l| l.trim().is_empty())
                .count();

            let mut next: Vec<String> = lines[..start].iter().map(|s| s.to_string()).collect();
            next.extend(block.lines().map(|s| s.to_string()));
            next.extend(std::iter::repeat_n(String::new(), blanks));
            next.extend(lines[end..].iter().map(|s| s.to_string()));
            next.join("\n")
        }
        None => {
            let head = text.trim_end();
            if head.is_empty() {
                block
            } else {
                format!("{}\n\n{}", head, block)
            }
        }
    };

    format!("{}\n", out.trim_end())
}

/// 讀目前生效的規則。回傳 (內容, 是不是真的在 CLAUDE.md 裡)。
///
/// 讀不到 `~/.claude/CLAUDE.md`、或裡面沒有那一段，就回內嵌的預設值。
pub fn load_rules() -> (String, bool) {
    let text = claude_md_path().and_then(|p| std::fs::read_to_string(p).ok());
    match text.as_deref().and_then(extract_rules_section) {
        Some(section) => (section, true),
        None => (default_rules(), false),
    }
}

/// 把規則寫進 `~/.claude/CLAUDE.md`，只動 `# 每日工作日誌` 那一段。
///
/// 寫之前先把整個檔複製一份成 `CLAUDE.md.bak-<stamp>`，回傳備份路徑
/// （檔案本來就不存在就沒有備份，回 `None`）。
pub fn save_rules(rules: &str, stamp: &str) -> Result<Option<String>> {
    let path = claude_md_path().context("找不到家目錄，不知道 CLAUDE.md 在哪")?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("建立資料夾失敗：{}", dir.display()))?;
    }

    let existing = std::fs::read_to_string(&path).unwrap_or_default();

    let backup = if path.is_file() {
        let bak = path.with_file_name(format!("CLAUDE.md.bak-{}", stamp));
        std::fs::copy(&path, &bak)
            .with_context(|| format!("備份失敗：{}", bak.display()))?;
        Some(bak.to_string_lossy().to_string())
    } else {
        None
    };

    let next = replace_rules_section(&existing, rules);
    std::fs::write(&path, next).with_context(|| format!("寫入失敗：{}", path.display()))?;
    Ok(backup)
}

/// TODO 是這個 app 唯一可寫的內容。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Todo {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub url: String,
    /// 狀態 id，可以留空
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub done: bool,
}

pub fn load_todos() -> Vec<Todo> {
    let path = match todos_path() {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

pub fn save_todos(list: &[Todo]) -> Result<()> {
    let path = todos_path()?;
    let text = serde_json::to_string_pretty(list)?;
    std::fs::write(&path, text).with_context(|| format!("寫入失敗：{}", path.display()))?;
    Ok(())
}

fn pending_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("pending.json"))
}

/// 還沒寫進 .md 的變更：日期（民國 7 碼）→ 那天要補的條目。
///
/// 這個 app 不寫日誌檔，所以在看板上拖卡片只會產生這裡的一筆，
/// 等使用者複製出去、請 Claude 寫進當天的 md。
pub type PendingMap = BTreeMap<String, Vec<Entry>>;

pub fn load_pending() -> PendingMap {
    let path = match pending_path() {
        Ok(p) => p,
        Err(_) => return PendingMap::new(),
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => PendingMap::new(),
    }
}

pub fn save_pending(map: &PendingMap) -> Result<()> {
    let path = pending_path()?;
    let text = serde_json::to_string_pretty(map)?;
    std::fs::write(&path, text).with_context(|| format!("寫入失敗：{}", path.display()))?;
    Ok(())
}

/// 讓設定頁可以直接顯示檔案放在哪
pub fn config_dir_display() -> String {
    config_dir().map(|p| p.to_string_lossy().to_string()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todos_survive_a_save_load_round_trip() {
        let list = vec![Todo {
            id: "t1".into(),
            text: "把提案寫完".into(),
            project: "project_a".into(),
            url: "https://redmine.example.com/issues/32990".into(),
            status: "proposing".into(),
            done: false,
        }];
        let text = serde_json::to_string(&list).unwrap();
        let back: Vec<Todo> = serde_json::from_str(&text).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].text, "把提案寫完");
        assert!(!back[0].done);
    }

    /// 舊檔案缺欄位時不該整包壞掉
    #[test]
    fn todos_tolerate_missing_optional_fields() {
        let back: Vec<Todo> = serde_json::from_str(r#"[{"id":"t1","text":"隨手記"}]"#).unwrap();
        assert_eq!(back[0].project, "");
        assert_eq!(back[0].status, "");
        assert!(!back[0].done);
    }

    /// 加了外部服務欄位之後，舊的 settings.json（只有 folder）還是要讀得起來
    #[test]
    fn settings_tolerate_missing_external_service_fields() {
        let s: Settings = serde_json::from_str(r#"{"folder":"/tmp/日誌"}"#).unwrap();
        assert_eq!(s.folder, "/tmp/日誌");
        assert_eq!(s.gitlab_base, "");
        assert_eq!(s.redmine_token, "");
    }

    /// 以前版本留下來的欄位（例如試作過的 entry_template）不該讓整包設定讀不起來
    #[test]
    fn settings_ignore_unknown_leftover_fields() {
        let s: Settings =
            serde_json::from_str(r#"{"folder":"/tmp/日誌","entry_template":"- {status} {link}"}"#)
                .unwrap();
        assert_eq!(s.folder, "/tmp/日誌");
    }

    /// 預設規則是編譯期嵌進來的，release build 之後也還在；
    /// 而且要是「可以直接貼進 CLAUDE.md 的規則本文」，不是整份說明文件
    #[test]
    fn default_rules_are_the_body_baked_into_the_binary() {
        let d = default_rules();
        assert!(d.starts_with(RULES_HEADING), "預設規則要從標題開始：{}", &d[..40.min(d.len())]);
        assert!(d.contains("## 路徑與檔名"));
        assert!(!d.contains("## 規則本文"), "說明文件的外殼不該被貼進 CLAUDE.md");
    }

    /* ---------- 段落取代（純函式，不碰真的 ~/.claude/CLAUDE.md） ---------- */

    #[test]
    fn an_existing_section_is_replaced_in_place() {
        let src = "# 別的規則\n\n不要動我\n\n# 每日工作日誌\n\n舊的規則\n\n# 又一段\n\n也不要動我\n";
        let out = replace_rules_section(src, "# 每日工作日誌\n\n新的規則");
        assert_eq!(
            out,
            "# 別的規則\n\n不要動我\n\n# 每日工作日誌\n\n新的規則\n\n# 又一段\n\n也不要動我\n"
        );
        assert!(!out.contains("舊的規則"));
    }

    #[test]
    fn a_missing_section_is_appended() {
        let src = "# 別的規則\n\n不要動我\n";
        let out = replace_rules_section(src, "# 每日工作日誌\n\n新的規則");
        assert_eq!(out, "# 別的規則\n\n不要動我\n\n# 每日工作日誌\n\n新的規則\n");
    }

    #[test]
    fn an_empty_file_just_gets_the_section() {
        assert_eq!(
            replace_rules_section("", "# 每日工作日誌\n\n新的規則"),
            "# 每日工作日誌\n\n新的規則\n"
        );
        assert_eq!(
            replace_rules_section("   \n\n", "# 每日工作日誌\n\n新的規則"),
            "# 每日工作日誌\n\n新的規則\n"
        );
    }

    #[test]
    fn a_section_at_the_end_of_the_file_is_replaced() {
        let src = "# 別的規則\n\n不要動我\n\n# 每日工作日誌\n\n舊的規則\n";
        let out = replace_rules_section(src, "# 每日工作日誌\n\n新的規則\n");
        assert_eq!(out, "# 別的規則\n\n不要動我\n\n# 每日工作日誌\n\n新的規則\n");
    }

    /// `## ` 這種下層標題不算段落結束
    #[test]
    fn sub_headings_do_not_end_the_section() {
        let src = "# 每日工作日誌\n\n## 格式\n\n- 一行\n\n# 後面這段是別人的\n\n別動\n";
        assert_eq!(
            extract_rules_section(src).unwrap(),
            "# 每日工作日誌\n\n## 格式\n\n- 一行"
        );
        let out = replace_rules_section(src, "# 每日工作日誌\n\n## 新格式");
        assert_eq!(out, "# 每日工作日誌\n\n## 新格式\n\n# 後面這段是別人的\n\n別動\n");
    }

    /// 標題被刪掉時要補回來，不然下次就找不到這一段了
    #[test]
    fn rules_without_the_heading_get_it_back() {
        let out = replace_rules_section("", "只有內文沒有標題");
        assert_eq!(out, "# 每日工作日誌\n\n只有內文沒有標題\n");
        assert!(extract_rules_section(&out).is_some());
    }

    #[test]
    fn extract_returns_none_when_the_section_is_absent() {
        assert!(extract_rules_section("# 別的規則\n\n沒有我要的\n").is_none());
    }

    #[test]
    fn default_folder_points_at_the_worklog_vault() {
        let f = default_folder();
        assert!(f.ends_with("每日工作日誌"), "預設路徑怪怪的：{}", f);
    }
}
