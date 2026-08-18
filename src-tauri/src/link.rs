//! 日誌條目上的連結，直接在 app 裡看內容。
//!
//! 日誌檔裡的連結只有兩種：Redmine 議題與自架 GitLab 的 MR／議題。
//! 這裡先用純字串把連結拆成 [`Target`]（好測試、不碰網路），
//! 再照 target 打對應的 API，最後一律整理成同一個 [`LinkContent`]，
//! 前端就不用分兩套畫法。
//!
//! token 從 `store::Settings` 讀，只會放進 request header，不會出現在回傳值或訊息裡。

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::store::Settings;

/// API 逾時。內網連不到的時候不要讓畫面一直卡著。
const TIMEOUT_SECS: u64 = 10;

/// 連結指到哪裡。純粹從網址看得出來的部分，不需要設定也不需要連線。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Redmine 議題：`{origin}/issues/{id}`
    RedmineIssue { origin: String, id: String },
    /// GitLab MR：`{origin}/{project}/-/merge_requests/{iid}`
    GitlabMr { origin: String, project: String, iid: String },
    /// GitLab 議題：`{origin}/{project}/-/issues/{iid}`
    GitlabIssue { origin: String, project: String, iid: String },
}

/// 面板上的一行 metadata
#[derive(Debug, Clone, Serialize)]
pub struct MetaRow {
    pub label: String,
    pub value: String,
}

fn meta(label: &str, value: impl Into<String>) -> MetaRow {
    MetaRow { label: label.into(), value: value.into() }
}

/// 面板上的一則留言。內容維持 markdown 原文，交給前端用既有的轉換器渲染。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Comment {
    /// 作者顯示名（Redmine 的 `user.name`／GitLab 的 `author.name`）
    pub author: String,
    /// 已經整理成看得懂的時間字串，例如 `2026-08-17 17:12`
    pub time: String,
    /// 留言內文（markdown 原文）
    pub body: String,
}

/// 抓回來的內容。Redmine 與 GitLab 都整理成這個型別。
#[derive(Debug, Clone, Serialize)]
pub struct LinkContent {
    /// 來源服務：`redmine` / `gitlab`
    pub source: String,
    /// 前端已有的分類：`issue` / `mr`
    pub kind: String,
    /// 給人看的編號，例如 `group/project_a !64`、`#32979`
    pub reference: String,
    pub title: String,
    /// 原始狀態字串（opened / merged / closed 或 Redmine 的狀態名）
    pub state: String,
    /// 狀態的中文顯示
    pub state_label: String,
    /// 副標資訊，一行一列
    pub meta: Vec<MetaRow>,
    /// 描述全文（markdown 原文，前端只保留換行）
    pub description: String,
    /// 留言／討論串，照時間由舊到新
    pub comments: Vec<Comment>,
    /// 留言抓失敗的原因。主要內容照樣顯示，只有這一區會標示讀取失敗。
    pub comments_error: Option<String>,
    /// 原始連結，面板下方要附上
    pub url: String,
}

/* ---------- 網址解析（純函式） ---------- */

/// 拆成 `(origin, path)`。path 不含 query 與 fragment。
fn split_url(url: &str) -> Option<(String, String)> {
    let u = url.trim();
    let (scheme, rest) = if let Some(r) = u.strip_prefix("http://") {
        ("http", r)
    } else if let Some(r) = u.strip_prefix("https://") {
        ("https", r)
    } else {
        return None;
    };
    let rest = rest.split(['?', '#']).next().unwrap_or("");
    let (host, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    if host.is_empty() {
        return None;
    }
    Some((format!("{}://{}", scheme, host), path.to_string()))
}

/// 開頭那串數字，例如 `64/diffs` → `64`、`32979.json` → `32979`
fn leading_number(s: &str) -> Option<String> {
    let n: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
    if n.is_empty() { None } else { Some(n) }
}

/// 認得出來就回 Target，認不出來回 None。不連線、不看設定。
pub fn parse_link(url: &str) -> Option<Target> {
    let (origin, path) = split_url(url)?;
    let path = path.trim_end_matches('/');

    // GitLab 的網址一定有 `/-/`，前面是專案路徑（可能含子群組）
    if let Some((project, tail)) = path.split_once("/-/") {
        let project = project.trim_matches('/').to_string();
        if project.is_empty() {
            return None;
        }
        if let Some(iid) = tail.strip_prefix("merge_requests/").and_then(leading_number) {
            return Some(Target::GitlabMr { origin, project, iid });
        }
        if let Some(iid) = tail.strip_prefix("issues/").and_then(leading_number) {
            return Some(Target::GitlabIssue { origin, project, iid });
        }
        return None;
    }

    // Redmine：`/issues/<id>`，也吃裝在子路徑底下的 `/redmine/issues/<id>`
    if let Some((_, tail)) = path.rsplit_once("/issues/") {
        if let Some(id) = leading_number(tail) {
            return Some(Target::RedmineIssue { origin, id });
        }
    }

    None
}

/* ---------- 位址與 token ---------- */

fn host_of(base: &str) -> String {
    base.trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or("")
        .to_string()
}

/// 決定要打哪個位址，順便確認 token 有設。
///
/// 設定裡填了位址就以設定為準；如果跟連結本身的主機不一樣就直接停下來，
/// 免得把 token 送到別台機器。設定留空就照連結自己的主機走。
fn resolve(origin: &str, configured: &str, token: &str, name: &str) -> Result<String, String> {
    let cfg = configured.trim().trim_end_matches('/');
    let base = if cfg.is_empty() {
        origin.to_string()
    } else {
        // 使用者可能只填主機名，沒填 http://
        let cfg = if cfg.starts_with("http://") || cfg.starts_with("https://") {
            cfg.to_string()
        } else {
            let scheme = if origin.starts_with("https://") { "https" } else { "http" };
            format!("{}://{}", scheme, cfg)
        };
        if host_of(&cfg) != host_of(origin) {
            return Err(format!(
                "這個連結的主機是 {}，設定裡的 {} 位址卻是 {}。為了不把 token 送到別台機器，這次沒有去抓內容——請到「設定 → 外部服務」確認位址。",
                host_of(origin),
                name,
                host_of(&cfg),
            ));
        }
        cfg
    };

    if token.trim().is_empty() {
        return Err(format!(
            "還沒設定 {} token。到「設定 → 外部服務」填好之後，點連結就會直接在這裡顯示內容。",
            name
        ));
    }

    Ok(base)
}

/* ---------- HTTP ---------- */

fn client() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("建立 HTTP 連線失敗：{}", e))
}

/// 連不上、逾時這類錯誤翻成人話
fn network_error(e: &reqwest::Error, host: &str) -> String {
    if e.is_timeout() {
        format!("連 {} 超過 {} 秒沒有回應。內網要先連上 VPN 或公司網路。", host, TIMEOUT_SECS)
    } else if e.is_connect() {
        format!("連不上 {}。確認位址對不對，以及現在是不是在同一個網路裡。", host)
    } else {
        format!("連線 {} 失敗：{}", host, e)
    }
}

/// HTTP 狀態碼翻成人話。分辨 token 不對、沒權限、找不到。
fn status_error(status: reqwest::StatusCode, name: &str, what: &str) -> String {
    match status.as_u16() {
        401 => format!("{} 說 token 不對或已經過期（401）。到「設定 → 外部服務」重新填一次。", name),
        403 => format!("{} 拒絕存取（403）。這個 token 沒有看 {} 的權限。", name, what),
        404 => format!("{} 上找不到 {}（404）。可能是編號不對，也可能是 token 看不到這個專案。", name, what),
        _ => format!("{} 回了 HTTP {}。", name, status.as_u16()),
    }
}

/// `2026-08-17T09:12:33Z` → `2026-08-17 17:12`（本地時間）
fn local_time(raw: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(raw) {
        Ok(t) => t
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M")
            .to_string(),
        Err(_) => raw.to_string(),
    }
}

/* ---------- Redmine ---------- */

#[derive(Deserialize)]
struct RedmineNamed {
    name: String,
}

#[derive(Deserialize)]
struct RedmineIssue {
    id: u64,
    subject: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    status: Option<RedmineNamed>,
    #[serde(default)]
    tracker: Option<RedmineNamed>,
    #[serde(default)]
    project: Option<RedmineNamed>,
    #[serde(default)]
    priority: Option<RedmineNamed>,
    #[serde(default)]
    author: Option<RedmineNamed>,
    #[serde(default)]
    assigned_to: Option<RedmineNamed>,
    #[serde(default)]
    updated_on: Option<String>,
    /// `include=journals` 才會有；裡面同時混著留言與純欄位變更
    #[serde(default)]
    journals: Option<Vec<RedmineJournal>>,
}

/// Redmine 的一筆歷程。`notes` 是留言文字，`details` 是欄位變更紀錄。
#[derive(Deserialize)]
struct RedmineJournal {
    #[serde(default)]
    user: Option<RedmineNamed>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    created_on: Option<String>,
}

/// 把 journals 挑成留言。**只有欄位變更、`notes` 是空的那種是雜訊，直接丟掉**。
fn redmine_comments(journals: &[RedmineJournal]) -> Vec<Comment> {
    journals
        .iter()
        .filter_map(|j| {
            let body = j.notes.as_deref().unwrap_or("").trim();
            if body.is_empty() {
                return None;
            }
            Some(Comment {
                author: j
                    .user
                    .as_ref()
                    .map(|u| u.name.clone())
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| "（不明）".to_string()),
                time: j.created_on.as_deref().map(local_time).unwrap_or_default(),
                body: body.to_string(),
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct RedmineIssueResp {
    issue: RedmineIssue,
}

fn fetch_redmine(origin: &str, id: &str, url: &str, s: &Settings) -> Result<LinkContent, String> {
    let base = resolve(origin, &s.redmine_base, &s.redmine_token, "Redmine")?;
    // journals 一起要回來，留言就不用多打一次 API
    let api = format!("{}/issues/{}.json?include=journals", base, id);

    let resp = client()?
        .get(&api)
        .header("X-Redmine-API-Key", s.redmine_token.trim())
        .header("Accept", "application/json")
        .send()
        .map_err(|e| network_error(&e, &host_of(&base)))?;

    if !resp.status().is_success() {
        return Err(status_error(resp.status(), "Redmine", &format!("議題 #{}", id)));
    }

    let body: RedmineIssueResp = resp
        .json()
        .map_err(|e| format!("Redmine 回的內容看不懂（不是預期的 JSON）：{}", e))?;
    let issue = body.issue;
    let comments = issue.journals.as_deref().map(redmine_comments).unwrap_or_default();

    let name = |v: &Option<RedmineNamed>| v.as_ref().map(|n| n.name.clone()).unwrap_or_default();

    let mut rows = Vec::new();
    let tracker = name(&issue.tracker);
    if !tracker.is_empty() {
        rows.push(meta("追蹤類型", tracker));
    }
    let project = name(&issue.project);
    if !project.is_empty() {
        rows.push(meta("專案", project));
    }
    let assignee = name(&issue.assigned_to);
    rows.push(meta("指派給", if assignee.is_empty() { "（未指派）".to_string() } else { assignee }));
    let author = name(&issue.author);
    if !author.is_empty() {
        rows.push(meta("建立者", author));
    }
    let priority = name(&issue.priority);
    if !priority.is_empty() {
        rows.push(meta("優先權", priority));
    }
    if let Some(t) = &issue.updated_on {
        rows.push(meta("最後更新", local_time(t)));
    }

    let state = name(&issue.status);
    Ok(LinkContent {
        source: "redmine".into(),
        kind: "issue".into(),
        reference: format!("#{}", issue.id),
        title: issue.subject,
        state_label: if state.is_empty() { "（沒有狀態）".into() } else { state.clone() },
        state,
        meta: rows,
        description: issue.description.unwrap_or_default(),
        comments,
        // journals 是跟著主體一起回來的，不會單獨失敗
        comments_error: None,
        url: url.to_string(),
    })
}

/* ---------- GitLab ---------- */

#[derive(Deserialize)]
struct GitlabUser {
    name: String,
    #[serde(default)]
    username: String,
}

#[derive(Deserialize)]
struct GitlabItem {
    iid: u64,
    title: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    author: Option<GitlabUser>,
    #[serde(default)]
    assignee: Option<GitlabUser>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    source_branch: Option<String>,
    #[serde(default)]
    target_branch: Option<String>,
    #[serde(default)]
    draft: Option<bool>,
    #[serde(default)]
    labels: Vec<String>,
}

/// GitLab 的一則 note。`system: true` 是「xxx added label」這種系統訊息，不是人寫的。
#[derive(Deserialize)]
struct GitlabNote {
    #[serde(default)]
    author: Option<GitlabUser>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    system: bool,
}

/// 把 notes 挑成留言。**系統訊息與空內容一律丟掉**。
fn gitlab_comments(notes: &[GitlabNote]) -> Vec<Comment> {
    notes
        .iter()
        .filter(|n| !n.system)
        .filter_map(|n| {
            let body = n.body.as_deref().unwrap_or("").trim();
            if body.is_empty() {
                return None;
            }
            Some(Comment {
                author: n
                    .author
                    .as_ref()
                    .map(|a| a.name.clone())
                    .filter(|x| !x.trim().is_empty())
                    .unwrap_or_else(|| "（不明）".to_string()),
                time: n.created_at.as_deref().map(local_time).unwrap_or_default(),
                body: body.to_string(),
            })
        })
        .collect()
}

/// 留言要另外打一次 API。抓失敗不要讓整個面板失敗，所以錯誤原樣往上丟，
/// 由呼叫端放進 `comments_error`，主要內容照樣顯示。
fn fetch_gitlab_notes(
    base: &str,
    project: &str,
    iid: &str,
    is_mr: bool,
    s: &Settings,
) -> Result<Vec<Comment>, String> {
    let what = if is_mr { "merge_requests" } else { "issues" };
    let api = format!(
        "{}/api/v4/projects/{}/{}/{}/notes?sort=asc&per_page=100",
        base,
        urlencoding::encode(project),
        what,
        iid
    );

    let resp = client()?
        .get(&api)
        .header("PRIVATE-TOKEN", s.gitlab_token.trim())
        .header("Accept", "application/json")
        .send()
        .map_err(|e| network_error(&e, &host_of(base)))?;

    if !resp.status().is_success() {
        return Err(status_error(resp.status(), "GitLab", "留言"));
    }

    let notes: Vec<GitlabNote> = resp
        .json()
        .map_err(|e| format!("GitLab 回的留言看不懂（不是預期的 JSON）：{}", e))?;
    Ok(gitlab_comments(&notes))
}

/// GitLab 的 state 翻成中文
fn gitlab_state_label(state: &str, is_mr: bool) -> String {
    match state {
        "opened" => if is_mr { "開啟中" } else { "未結案" }.to_string(),
        "merged" => "已合併".to_string(),
        "closed" => if is_mr { "已關閉" } else { "已結案" }.to_string(),
        "locked" => "已鎖定".to_string(),
        "" => "（沒有狀態）".to_string(),
        other => other.to_string(),
    }
}

fn fetch_gitlab(
    origin: &str,
    project: &str,
    iid: &str,
    is_mr: bool,
    url: &str,
    s: &Settings,
) -> Result<LinkContent, String> {
    let base = resolve(origin, &s.gitlab_base, &s.gitlab_token, "GitLab")?;
    let what = if is_mr { "merge_requests" } else { "issues" };
    let api = format!(
        "{}/api/v4/projects/{}/{}/{}",
        base,
        urlencoding::encode(project),
        what,
        iid
    );

    let resp = client()?
        .get(&api)
        .header("PRIVATE-TOKEN", s.gitlab_token.trim())
        .header("Accept", "application/json")
        .send()
        .map_err(|e| network_error(&e, &host_of(&base)))?;

    if !resp.status().is_success() {
        let label = if is_mr { "MR" } else { "議題" };
        return Err(status_error(
            resp.status(),
            "GitLab",
            &format!("{} {}!{}", project, label, iid),
        ));
    }

    let it: GitlabItem = resp
        .json()
        .map_err(|e| format!("GitLab 回的內容看不懂（不是預期的 JSON）：{}", e))?;

    // 留言是額外一次呼叫；失敗就只記原因，主要內容照樣顯示
    let (comments, comments_error) = match fetch_gitlab_notes(&base, project, iid, is_mr, s) {
        Ok(list) => (list, None),
        Err(e) => (Vec::new(), Some(e)),
    };

    let who = |u: &Option<GitlabUser>| {
        u.as_ref()
            .map(|a| if a.username.is_empty() { a.name.clone() } else { format!("{}（{}）", a.name, a.username) })
            .unwrap_or_default()
    };

    let mut rows = vec![meta("專案", project.to_string())];
    let author = who(&it.author);
    if !author.is_empty() {
        rows.push(meta("作者", author));
    }
    let assignee = who(&it.assignee);
    if !assignee.is_empty() {
        rows.push(meta("指派給", assignee));
    }
    if is_mr {
        if let (Some(src), Some(dst)) = (&it.source_branch, &it.target_branch) {
            rows.push(meta("分支", format!("{} → {}", src, dst)));
        }
        if it.draft.unwrap_or(false) {
            rows.push(meta("草稿", "是"));
        }
    } else if !it.labels.is_empty() {
        rows.push(meta("標籤", it.labels.join("、")));
    }
    if let Some(t) = &it.updated_at {
        rows.push(meta("最後更新", local_time(t)));
    }

    Ok(LinkContent {
        source: "gitlab".into(),
        kind: if is_mr { "mr".into() } else { "issue".into() },
        reference: format!("{}{}{}", project, if is_mr { " !" } else { " #" }, it.iid),
        title: it.title,
        state_label: gitlab_state_label(&it.state, is_mr),
        state: it.state,
        meta: rows,
        description: it.description.unwrap_or_default(),
        comments,
        comments_error,
        url: url.to_string(),
    })
}

/* ---------- 描述／留言裡的圖片 ----------
   Redmine 與 GitLab 的附圖多半要帶 token 才拿得到，WebView 自己去 `<img src>` 多半 401 或空白，
   所以由後端用同一套設定（base 位址、token、10 秒逾時、同樣的主機比對）抓下來，
   轉成 `data:` URI 再交給前端。網址補全與型別／大小檢查都是純函式，測試不打網路。 */

/// 圖片大小上限。太大的東西不要整包塞進 WebView。
const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

/// 只認這幾種。其他型別（被導去登入頁的 HTML、PDF…）一律拒絕。
const IMAGE_TYPES: [&str; 5] = [
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/svg+xml",
];

/// 抓回來的一張圖。`data_uri` 直接給 `<img src>` 用。
#[derive(Debug, Clone, Serialize)]
pub struct ImageData {
    /// 正規化後的 MIME，例如 `image/png`
    pub mime: String,
    /// `data:image/png;base64,...`
    pub data_uri: String,
    /// 原始位元組數，前端不一定用得到，但出問題時好對照
    pub bytes: usize,
}

/// base64。只為了組 `data:` URI，不值得為此多一個相依套件。
fn base64_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let b0 = c[0] as u32;
        let b1 = *c.get(1).unwrap_or(&0) as u32;
        let b2 = *c.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if c.len() > 1 { T[(n >> 6 & 63) as usize] as char } else { '=' });
        out.push(if c.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

fn too_big(n: usize) -> String {
    format!(
        "這張圖 {:.1} MB，超過 {} MB 的上限，沒有載進來。",
        n as f64 / (1024.0 * 1024.0),
        MAX_IMAGE_BYTES / (1024 * 1024)
    )
}

/// 大小檢查（純函式）
fn check_size(n: usize) -> Result<(), String> {
    if n > MAX_IMAGE_BYTES { Err(too_big(n)) } else { Ok(()) }
}

/// Content-Type 檢查（純函式）。只放行圖片，順便把 `image/jpg` 正規化成 `image/jpeg`。
fn image_mime(content_type: &str) -> Result<String, String> {
    let t = content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let t = if t == "image/jpg" { "image/jpeg".to_string() } else { t };
    if IMAGE_TYPES.contains(&t.as_str()) {
        Ok(t)
    } else if t.is_empty() {
        Err("回來的東西沒說是什麼型別，不敢當成圖片。".into())
    } else {
        Err(format!(
            "拿回來的不是支援的圖片型別（{}）。可能被導去登入頁，也可能 token 看不到這個附件。",
            t
        ))
    }
}

/// 把描述裡的圖片網址補成絕對網址（純函式，不連線、不看設定）。
///
/// - 已經是絕對網址：只放行跟服務同一台主機的，免得把 token 送去別台
/// - `/uploads/...`：GitLab 的專案附件實際掛在 `{base}/{專案路徑}/uploads/...`，要補專案路徑
/// - 其他 `/` 開頭：直接接在 base 後面（Redmine 的 `/attachments/download/...` 就是這種）
/// - `uploads/...`：GitLab 偶爾寫成不帶開頭斜線的相對路徑，同樣補專案路徑
/// - 其餘相對路徑：補不出來就當抓不到，**不要猜**一個可能是錯的網址
fn image_url(src: &str, base: &str, target: &Target) -> Result<String, String> {
    let src = src.trim();
    if src.is_empty() {
        return Err("圖片網址是空的。".into());
    }

    if src.starts_with("http://") || src.starts_with("https://") {
        let (origin, _) = split_url(src).ok_or_else(|| "圖片網址看不懂。".to_string())?;
        if host_of(&origin) != host_of(base) {
            return Err(format!(
                "這張圖放在 {}，跟設定的服務不是同一台主機。為了不把 token 送過去，這張沒有抓。",
                host_of(&origin)
            ));
        }
        return Ok(src.to_string());
    }

    // `//host/path` 與其他 scheme（data:、javascript:…）一律不碰
    if src.starts_with("//") || src.contains("://") {
        return Err("圖片網址看不懂。".into());
    }

    let project = match target {
        Target::GitlabMr { project, .. } | Target::GitlabIssue { project, .. } => Some(project.as_str()),
        Target::RedmineIssue { .. } => None,
    };
    let base = base.trim_end_matches('/');

    if let Some(rest) = src.strip_prefix('/') {
        if let Some(p) = project {
            if rest.starts_with("uploads/") {
                return Ok(format!("{}/{}/{}", base, p, rest));
            }
        }
        return Ok(format!("{}/{}", base, rest));
    }

    if let Some(p) = project {
        if src.starts_with("uploads/") {
            return Ok(format!("{}/{}/{}", base, p, src));
        }
    }

    Err("這張圖寫的是相對路徑，補不出完整網址，所以沒有抓。".into())
}

/// 抓描述／留言裡的一張圖。
///
/// `page_url` 是那張圖所屬的議題／MR 連結：要靠它決定打哪一台、用哪把 token，
/// 以及 GitLab 的相對路徑要補上哪個專案。
pub fn fetch_image(page_url: &str, src: &str, s: &Settings) -> Result<ImageData, String> {
    let target = parse_link(page_url)
        .ok_or_else(|| "認不出這張圖屬於哪個服務，所以沒有抓。".to_string())?;

    let (base, header, token, name) = match &target {
        Target::RedmineIssue { origin, .. } => (
            resolve(origin, &s.redmine_base, &s.redmine_token, "Redmine")?,
            "X-Redmine-API-Key",
            s.redmine_token.trim().to_string(),
            "Redmine",
        ),
        Target::GitlabMr { origin, .. } | Target::GitlabIssue { origin, .. } => (
            resolve(origin, &s.gitlab_base, &s.gitlab_token, "GitLab")?,
            "PRIVATE-TOKEN",
            s.gitlab_token.trim().to_string(),
            "GitLab",
        ),
    };

    let url = image_url(src, &base, &target)?;

    let resp = client()?
        .get(&url)
        .header(header, token)
        .header("Accept", "image/*")
        .send()
        .map_err(|e| network_error(&e, &host_of(&base)))?;

    if !resp.status().is_success() {
        return Err(status_error(resp.status(), name, "這張圖"));
    }

    let mime = image_mime(
        resp.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or(""),
    )?;

    // 有 Content-Length 就先擋，省得把整包收下來才發現太大
    if let Some(n) = resp.content_length() {
        check_size(n as usize)?;
    }

    let bytes = resp.bytes().map_err(|e| format!("圖片沒收完：{}", e))?;
    check_size(bytes.len())?;

    Ok(ImageData {
        data_uri: format!("data:{};base64,{}", mime, base64_encode(&bytes)),
        mime,
        bytes: bytes.len(),
    })
}

/* ---------- 對外 ---------- */

/// 抓一個連結的內容。認不出來的網址會回錯誤，前端照樣可以「在瀏覽器開啟」。
pub fn fetch(url: &str, s: &Settings) -> Result<LinkContent, String> {
    match parse_link(url) {
        Some(Target::RedmineIssue { origin, id }) => fetch_redmine(&origin, &id, url, s),
        Some(Target::GitlabMr { origin, project, iid }) => {
            fetch_gitlab(&origin, &project, &iid, true, url, s)
        }
        Some(Target::GitlabIssue { origin, project, iid }) => {
            fetch_gitlab(&origin, &project, &iid, false, url, s)
        }
        None => Err("這個連結不是 Redmine 議題也不是 GitLab MR／議題，抓不到內容。".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redmine_issue_url_is_recognised() {
        assert_eq!(
            parse_link("https://redmine.example.com/issues/32979"),
            Some(Target::RedmineIssue {
                origin: "https://redmine.example.com".into(),
                id: "32979".into(),
            })
        );
        // 後面接東西、帶 query、結尾斜線都算同一筆
        assert_eq!(
            parse_link("https://redmine.example.com/issues/32979/?tab=history#note-3"),
            Some(Target::RedmineIssue {
                origin: "https://redmine.example.com".into(),
                id: "32979".into(),
            })
        );
    }

    #[test]
    fn gitlab_merge_request_url_is_recognised() {
        assert_eq!(
            parse_link("http://gitlab.example.com/group/project_a/-/merge_requests/64"),
            Some(Target::GitlabMr {
                origin: "http://gitlab.example.com".into(),
                project: "group/project_a".into(),
                iid: "64".into(),
            })
        );
        // MR 底下的分頁也要歸到同一筆
        assert_eq!(
            parse_link("http://gitlab.example.com/group/project_a/-/merge_requests/64/diffs"),
            Some(Target::GitlabMr {
                origin: "http://gitlab.example.com".into(),
                project: "group/project_a".into(),
                iid: "64".into(),
            })
        );
    }

    #[test]
    fn gitlab_issue_url_is_recognised() {
        assert_eq!(
            parse_link("http://gitlab.example.com/group/project_a/-/issues/12"),
            Some(Target::GitlabIssue {
                origin: "http://gitlab.example.com".into(),
                project: "group/project_a".into(),
                iid: "12".into(),
            })
        );
    }

    #[test]
    fn anything_else_is_not_recognised() {
        assert_eq!(parse_link("https://example.com/docs/readme"), None);
        assert_eq!(parse_link("https://redmine.example.com/issues/"), None);
        assert_eq!(parse_link("http://gitlab.example.com/group/project_a/-/pipelines/9"), None);
        assert_eq!(parse_link("mailto:someone@example.com"), None);
        assert_eq!(parse_link(""), None);
    }

    /// 沒設 token 就別打 API，直接講原因
    #[test]
    fn missing_token_is_reported_before_any_request() {
        let s = Settings::default();
        let err = fetch("https://redmine.example.com/issues/32979", &s).unwrap_err();
        assert!(err.contains("Redmine token"), "訊息不對：{}", err);
    }

    /// 設定的位址跟連結的主機不一樣：不要把 token 送過去
    #[test]
    fn a_different_host_stops_before_sending_the_token() {
        let mut s = Settings::default();
        s.gitlab_base = "http://10.0.0.9".into();
        s.gitlab_token = "不會被送出去".into();
        let err = fetch("http://gitlab.example.com/group/project_a/-/merge_requests/64", &s).unwrap_err();
        assert!(err.contains("gitlab.example.com"), "訊息不對：{}", err);
        assert!(!err.contains("不會被送出去"), "訊息不該帶到 token");
    }

    /* ---------- 留言的 JSON 解析（不打網路，餵假資料） ---------- */

    /// Redmine：`notes` 是空字串的純欄位變更是雜訊，不能出現在留言裡
    #[test]
    fn redmine_journals_without_notes_are_dropped() {
        let raw = r#"{
          "issue": {
            "id": 32979,
            "subject": "對話紀錄搜尋",
            "journals": [
              {
                "id": 1,
                "user": { "id": 7, "name": "王小明" },
                "notes": "規格確認過了，先做 v1。",
                "created_on": "2026-08-17T09:12:33Z",
                "details": []
              },
              {
                "id": 2,
                "user": { "id": 8, "name": "李小華" },
                "notes": "",
                "created_on": "2026-08-17T10:00:00Z",
                "details": [
                  { "property": "attr", "name": "status_id", "old_value": "1", "new_value": "2" }
                ]
              },
              {
                "id": 3,
                "user": { "id": 8, "name": "李小華" },
                "notes": "   ",
                "created_on": "2026-08-17T10:05:00Z",
                "details": []
              }
            ]
          }
        }"#;

        let body: RedmineIssueResp = serde_json::from_str(raw).expect("假的 Redmine JSON 要解得開");
        let list = redmine_comments(body.issue.journals.as_deref().unwrap());

        assert_eq!(list.len(), 1, "只有真的有 notes 的那一筆算留言：{:?}", list);
        assert_eq!(list[0].author, "王小明");
        assert_eq!(list[0].body, "規格確認過了，先做 v1。");
        // 時間會轉成本地時區，只確認格式化成「到分鐘」而且不是原字串
        assert_eq!(list[0].time.len(), "2026-08-17 17:12".len(), "時間格式不對：{}", list[0].time);
        assert!(list[0].time.starts_with("2026-08-17"), "時間不對：{}", list[0].time);
    }

    /// Redmine：沒有 `include=journals` 的舊回應（沒有這個欄位）也不能爆
    #[test]
    fn redmine_issue_without_journals_still_parses() {
        let raw = r#"{ "issue": { "id": 1, "subject": "沒有歷程" } }"#;
        let body: RedmineIssueResp = serde_json::from_str(raw).expect("要解得開");
        assert!(body.issue.journals.is_none());
    }

    /// GitLab：`system: true` 是「xxx added label」那種系統訊息，要濾掉
    #[test]
    fn gitlab_system_notes_are_dropped() {
        let raw = r#"[
          {
            "id": 101,
            "body": "changed title from **A** to **B**",
            "author": { "id": 3, "name": "王小明", "username": "ming" },
            "created_at": "2026-08-17T09:00:00.000Z",
            "system": true
          },
          {
            "id": 102,
            "body": "這段可以抽成 service，之後 file_service 也會用到。",
            "author": { "id": 4, "name": "李小華", "username": "hua" },
            "created_at": "2026-08-17T09:12:33.000Z",
            "system": false
          },
          {
            "id": 103,
            "body": "",
            "author": { "id": 4, "name": "李小華", "username": "hua" },
            "created_at": "2026-08-17T09:20:00.000Z",
            "system": false
          }
        ]"#;

        let notes: Vec<GitlabNote> = serde_json::from_str(raw).expect("假的 GitLab JSON 要解得開");
        let list = gitlab_comments(&notes);

        assert_eq!(list.len(), 1, "系統訊息與空內容都不算留言：{:?}", list);
        assert_eq!(list[0].author, "李小華");
        assert_eq!(list[0].body, "這段可以抽成 service，之後 file_service 也會用到。");
        assert_eq!(list[0].time.len(), "2026-08-17 17:12".len(), "時間格式不對：{}", list[0].time);
    }

    /* ---------- 圖片：網址補全與型別／大小檢查（純函式，不打網路） ---------- */

    fn gitlab_target() -> Target {
        Target::GitlabMr {
            origin: "http://gitlab.example.com".into(),
            project: "group/project_a".into(),
            iid: "64".into(),
        }
    }

    fn redmine_target() -> Target {
        Target::RedmineIssue { origin: "https://redmine.example.com".into(), id: "32979".into() }
    }

    /// GitLab 的 `/uploads/...` 是專案附件，要補上專案路徑才拿得到
    #[test]
    fn gitlab_uploads_get_the_project_path() {
        assert_eq!(
            image_url("/uploads/abc123/shot.png", "http://gitlab.example.com", &gitlab_target()).unwrap(),
            "http://gitlab.example.com/group/project_a/uploads/abc123/shot.png"
        );
        // 不帶開頭斜線的寫法也是同一回事
        assert_eq!(
            image_url("uploads/abc123/shot.png", "http://gitlab.example.com/", &gitlab_target()).unwrap(),
            "http://gitlab.example.com/group/project_a/uploads/abc123/shot.png"
        );
        // 已經含專案路徑的絕對路徑不要再補一次
        assert_eq!(
            image_url("/group/project_a/uploads/abc123/shot.png", "http://gitlab.example.com", &gitlab_target()).unwrap(),
            "http://gitlab.example.com/group/project_a/uploads/abc123/shot.png"
        );
    }

    /// Redmine 的附件是絕對路徑，直接接在 base 後面；沒有專案路徑要補
    #[test]
    fn redmine_absolute_path_hangs_off_the_base() {
        assert_eq!(
            image_url("/attachments/download/9527/shot.png", "https://redmine.example.com", &redmine_target()).unwrap(),
            "https://redmine.example.com/attachments/download/9527/shot.png"
        );
        // Redmine 沒有「專案路徑」可以補，光一個檔名補不出來
        assert!(image_url("shot.png", "https://redmine.example.com", &redmine_target()).is_err());
    }

    /// 同一台主機的絕對網址原樣用，別台主機一律不抓（token 不能送出去）
    #[test]
    fn an_image_on_another_host_is_refused() {
        assert_eq!(
            image_url("http://gitlab.example.com/x/y.png", "http://gitlab.example.com", &gitlab_target()).unwrap(),
            "http://gitlab.example.com/x/y.png"
        );
        let err = image_url("https://evil.example.com/y.png", "http://gitlab.example.com", &gitlab_target())
            .unwrap_err();
        assert!(err.contains("evil.example.com"), "訊息不對：{}", err);
    }

    /// 補不出來的寫法就當抓不到，不要猜一個可能是錯的網址
    #[test]
    fn unresolvable_image_sources_are_refused() {
        for bad in [
            "",
            "   ",
            "圖.png",
            "//cdn.example.com/y.png",
            "data:image/png;base64,AAAA",
            "javascript:alert(1)",
        ] {
            assert!(
                image_url(bad, "http://gitlab.example.com", &gitlab_target()).is_err(),
                "這個應該要被擋掉：{:?}",
                bad
            );
        }
    }

    /// 只收圖片型別；被導去登入頁（text/html）那種一定要擋下來
    #[test]
    fn only_image_types_are_accepted() {
        assert_eq!(image_mime("image/png").unwrap(), "image/png");
        assert_eq!(image_mime("IMAGE/PNG; charset=binary").unwrap(), "image/png");
        assert_eq!(image_mime("image/jpg").unwrap(), "image/jpeg");
        assert_eq!(image_mime("image/svg+xml").unwrap(), "image/svg+xml");
        assert!(image_mime("text/html; charset=utf-8").is_err());
        assert!(image_mime("application/pdf").is_err());
        assert!(image_mime("").is_err());
    }

    /// 超過上限就直接回錯誤，不要把整包塞進 WebView
    #[test]
    fn an_oversized_image_is_refused() {
        assert!(check_size(0).is_ok());
        assert!(check_size(MAX_IMAGE_BYTES).is_ok());
        let err = check_size(MAX_IMAGE_BYTES + 1).unwrap_err();
        assert!(err.contains("10 MB"), "訊息不對：{}", err);
    }

    /// base64 是自己寫的，拿標準向量對一次（含補 `=` 的兩種尾巴）
    #[test]
    fn base64_matches_the_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
        // PNG 檔頭那 8 個 byte，確認非 ASCII 也對
        assert_eq!(base64_encode(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]), "iVBORw0KGgo=");
    }

    /// 沒設 token 就別去抓圖，跟抓內容一樣先講原因
    #[test]
    fn fetch_image_without_a_token_stops_early() {
        let s = Settings::default();
        let err = fetch_image(
            "http://gitlab.example.com/group/project_a/-/merge_requests/64",
            "/uploads/abc/shot.png",
            &s,
        )
        .unwrap_err();
        assert!(err.contains("GitLab token"), "訊息不對：{}", err);
    }

    /// 沒有 `system` 欄位就當成人寫的留言（預設 false）
    #[test]
    fn gitlab_note_without_system_field_is_kept() {
        let raw = r#"[{ "id": 1, "body": "先這樣", "author": { "id": 1, "name": "王小明" } }]"#;
        let notes: Vec<GitlabNote> = serde_json::from_str(raw).expect("要解得開");
        let list = gitlab_comments(&notes);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].author, "王小明");
        assert_eq!(list[0].time, "", "沒有時間就留空字串");
    }
}
