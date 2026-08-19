//! 線上更新：接 Tauri 官方 updater，更新檔放在 GitHub Releases。
//!
//! 流程只有兩步：`check()` 去讀 `latest.json` 問有沒有新版，`install()` 把新版抓下來換掉自己。
//! 兩支都會回中文訊息，連不上、沒有 Release、簽章對不上分得出來（見 [`friendly_error`]）。
//!
//! **這個 app 是 ad-hoc 簽章、沒有 Apple 公證**，所以從 GitHub 下載回來的更新檔會帶
//! quarantine 標記，macOS 有可能直接擋下來。因此這裡不承諾「一定裝得起來」：
//! 裝不起來就把原因講清楚，讓使用者自己去 [`RELEASES_PAGE`] 抓。

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

/// 更新檔的下載頁。線上更新失敗時讓使用者自己抓。
pub const RELEASES_PAGE: &str = "https://github.com/amoeric/worklog/releases/latest";

/// 下載進度事件的名字，前端 listen 這個
pub const PROGRESS_EVENT: &str = "worklog://update-progress";

/// 檢查結果。沒有新版時除了 `current` 之外都是空的。
#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    /// 目前跑的版本（來自 tauri.conf.json 的 version）
    pub current: String,
    pub available: bool,
    /// 新版版號
    pub version: String,
    /// 更新說明（latest.json 的 notes）
    pub notes: String,
    /// 發佈日期，只留 `YYYY-MM-DD`
    pub date: String,
    /// 自己下載的地方
    pub page: String,
}

impl UpdateInfo {
    fn none(current: String) -> Self {
        UpdateInfo {
            current,
            available: false,
            version: String::new(),
            notes: String::new(),
            date: String::new(),
            page: RELEASES_PAGE.to_string(),
        }
    }
}

/// 開設定頁就要知道的兩件事：現在跑的是哪一版、更新檔的下載頁在哪。
#[derive(Serialize, Clone)]
pub struct AppVersion {
    pub version: String,
    /// 線上更新失敗時，讓使用者自己抓的地方
    pub page: String,
}

/// 下載進度。`total` 有可能是 `None`（伺服器沒給 Content-Length）。
#[derive(Serialize, Clone)]
pub struct Progress {
    pub downloaded: u64,
    pub total: Option<u64>,
    /// 0–100；算不出來就是 `None`，前端改顯示不確定的進度
    pub percent: Option<u8>,
    /// 下載完了（還在安裝）
    pub done: bool,
}

/* ---------- 純函式：不打網路，都有測試 ---------- */

/// 把版本字串切成 `(主, 次, 修)`。前面的 `v` 與後面的 `-beta.1`、`+build` 都不算。
fn version_parts(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim();
    let v = v.strip_prefix(['v', 'V']).unwrap_or(v);
    let core = v.split(['-', '+']).next()?;

    let mut nums: Vec<u64> = Vec::new();
    for part in core.split('.') {
        nums.push(part.trim().parse::<u64>().ok()?);
    }
    // 少寫的位數當 0，所以 "1.2" 等於 "1.2.0"；多出來的第四段（1.2.3.4）認不得
    match nums.len() {
        1 => Some((nums[0], 0, 0)),
        2 => Some((nums[0], nums[1], 0)),
        3 => Some((nums[0], nums[1], nums[2])),
        _ => None,
    }
}

/// 遠端那個版本是不是真的比較新。
///
/// updater 自己已經比過一次了，這裡是第二道保險：萬一 `latest.json` 寫錯版號
/// （例如手滑退版），就不要拿舊的蓋掉新的。
/// 任何一邊看不懂就回 `true`——那代表判斷不了，信 updater 的決定。
pub fn is_newer(current: &str, candidate: &str) -> bool {
    match (version_parts(current), version_parts(candidate)) {
        (Some(a), Some(b)) => b > a,
        _ => true,
    }
}

/// 更新說明：把 CRLF 換掉、去掉頭尾空白，太長就截斷（設定頁不是 changelog 全文）。
pub fn clean_notes(body: Option<&str>) -> String {
    const MAX: usize = 4000;
    let text = body.unwrap_or("").replace("\r\n", "\n").replace('\r', "\n");
    let text = text.trim();
    if text.chars().count() <= MAX {
        return text.to_string();
    }
    let cut: String = text.chars().take(MAX).collect();
    format!("{}…（後面省略，完整內容看下載頁）", cut.trim_end())
}

/// `pub_date` 只留 `YYYY-MM-DD`；認不出來就當作沒有。
pub fn short_date(raw: Option<&str>) -> String {
    let raw = raw.unwrap_or("").trim();
    if raw.len() < 10 {
        return String::new();
    }
    let head = &raw[..10];
    let ok = head.chars().enumerate().all(|(i, c)| {
        if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() }
    });
    if ok { head.to_string() } else { String::new() }
}

/// 下載進度的百分比。總長度不知道或是 0 就回 `None`。
pub fn percent(downloaded: u64, total: Option<u64>) -> Option<u8> {
    let total = total?;
    if total == 0 {
        return None;
    }
    let p = downloaded.saturating_mul(100) / total;
    Some(p.min(100) as u8)
}

/// updater 的錯誤訊息是英文的，翻成看得懂而且分得出是哪一種的中文。
///
/// 認不出來就原封不動回傳（寧可給原文，也不要瞎猜）。
pub fn friendly_error(raw: &str) -> String {
    let raw = raw.trim();
    let s = raw.to_lowercase();

    let msg = if s.contains("does not have any endpoints") {
        "這個版本沒有設定更新來源，沒辦法線上更新"
    } else if s.contains("secure protocol") {
        "更新來源必須用 https"
    } else if s.contains("signature")
        || s.contains("minisign")
        || s.contains("verification failed")
        || s.contains("untrusted")
    {
        "更新檔的簽章對不上，為了安全起見不安裝（Release 上的 .sig 可能沒跟著換，或檔案被動過）"
    } else if s.contains("could not fetch a valid release json") {
        "更新來源上沒有可用的版本資訊，可能還沒發過 Release，或 latest.json 沒上傳"
    } else if s.contains("was not found in the response") || s.contains("fallback platforms") {
        "latest.json 裡沒有這台機器的平台（macOS 要有 darwin-aarch64 或 darwin-x86_64）"
    } else if s.contains("404") {
        "更新來源找不到（404），latest.json 可能還沒上傳到 Release"
    } else if s.contains("401") || s.contains("403") {
        "更新來源拒絕存取（要確認那個 repo 是公開的）"
    } else if s.contains("dns")
        || s.contains("error sending request")
        || s.contains("connection")
        || s.contains("connect")
        || s.contains("timed out")
        || s.contains("timeout")
        || s.contains("network")
        || s.contains("offline")
    {
        "連不上更新來源，檢查網路之後再試一次"
    } else if s.contains("expected value")
        || s.contains("missing field")
        || s.contains("invalid type")
        || s.contains("trailing characters")
    {
        "latest.json 的格式不對，讀不懂"
    } else if s.contains("unsupported os") || s.contains("unsupported application architecture") {
        "這台機器的作業系統或架構不支援線上更新"
    } else if s.contains("failed to determine updater package extract path") {
        "找不到要換掉的 app（開發模式下沒有 .app 可以換，要用打包過的版本才試得動）"
    } else if s.contains("authentication failed") {
        "系統要求的授權沒有通過（取消了或密碼不對）"
    } else if s.contains("permission denied")
        || s.contains("os error 13")
        || s.contains("read-only")
        || s.contains("operation not permitted")
    {
        "沒有權限換掉 app 本身（app 可能放在唯讀的位置，或需要管理者權限）"
    } else if s.contains("invalid updater binary format")
        || s.contains("binary for the current target not found")
        || s.contains("extract")
        || s.contains("archive")
    {
        "更新檔的內容不對，解不開（上傳的可能不是 .app.tar.gz）"
    } else {
        return raw.to_string();
    };

    msg.to_string()
}

/* ---------- 真的會動的部分 ---------- */

/// 目前跑的版本。從 `tauri.conf.json` 的 version 來，前端不要自己寫死。
pub fn current_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

/// 給設定頁用的：版本 + 下載頁。不打網路。
pub fn app_version(app: &AppHandle) -> AppVersion {
    AppVersion { version: current_version(app), page: RELEASES_PAGE.to_string() }
}

/// 去 endpoint 問一次。回 `None` 代表已經是最新版。
async fn probe(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, String> {
    let updater = app
        .updater()
        .map_err(|e| friendly_error(&e.to_string()))?;
    updater
        .check()
        .await
        .map_err(|e| friendly_error(&e.to_string()))
}

/// 檢查有沒有新版。不會下載任何東西。
pub async fn check(app: AppHandle) -> Result<UpdateInfo, String> {
    let current = current_version(&app);
    let found = probe(&app).await?;

    let update = match found {
        Some(u) => u,
        None => return Ok(UpdateInfo::none(current)),
    };
    // updater 說有新版，但版號要真的比較大才算
    if !is_newer(&current, &update.version) {
        return Ok(UpdateInfo::none(current));
    }

    Ok(UpdateInfo {
        current,
        available: true,
        version: update.version.clone(),
        notes: clean_notes(update.body.as_deref()),
        date: short_date(
            update
                .raw_json
                .get("pub_date")
                .and_then(|v| v.as_str()),
        ),
        page: RELEASES_PAGE.to_string(),
    })
}

/// 下載並安裝新版。
///
/// 為了不用把 `Update` 存在後端狀態裡，這裡會自己再問一次 endpoint；
/// 多打一次網路換來的是「兩支指令各自獨立」，不會有裝到過期資訊的問題。
///
/// 下載期間會一直丟 [`PROGRESS_EVENT`] 事件出去。裝完**不會**自己重開，
/// 由前端顯示完訊息再呼叫 `restart`。
pub async fn install(app: AppHandle) -> Result<(), String> {
    let update = probe(&app)
        .await?
        .ok_or_else(|| "已經是最新版了，沒有東西要裝".to_string())?;

    let chunk_handle = app.clone();
    let finish_handle = app.clone();
    let mut downloaded: u64 = 0;

    update
        .download_and_install(
            move |chunk, total| {
                downloaded = downloaded.saturating_add(chunk as u64);
                let _ = chunk_handle.emit(
                    PROGRESS_EVENT,
                    Progress {
                        downloaded,
                        total,
                        percent: percent(downloaded, total),
                        done: false,
                    },
                );
            },
            move || {
                let _ = finish_handle.emit(
                    PROGRESS_EVENT,
                    Progress { downloaded: 0, total: None, percent: Some(100), done: true },
                );
            },
        )
        .await
        .map_err(|e| friendly_error(&e.to_string()))?;

    Ok(())
}

/// 裝完之後重開，讓新版真的跑起來。
pub fn restart(app: AppHandle) {
    let handle = app.clone();
    // 重開要在主執行緒上做
    let _ = app.run_on_main_thread(move || handle.restart());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parts_reads_the_usual_shapes() {
        assert_eq!(version_parts("0.1.0"), Some((0, 1, 0)));
        assert_eq!(version_parts("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(version_parts(" 1.2.3 "), Some((1, 2, 3)));
        assert_eq!(version_parts("1.2"), Some((1, 2, 0)));
        assert_eq!(version_parts("2"), Some((2, 0, 0)));
        assert_eq!(version_parts("1.2.3-beta.1"), Some((1, 2, 3)));
        assert_eq!(version_parts("1.2.3+20260819"), Some((1, 2, 3)));
        assert_eq!(version_parts("1.2.3.4"), None);
        assert_eq!(version_parts("最新版"), None);
        assert_eq!(version_parts(""), None);
    }

    #[test]
    fn is_newer_only_says_yes_for_a_bigger_version() {
        assert!(is_newer("0.1.0", "0.1.1"));
        assert!(is_newer("0.1.0", "0.2.0"));
        assert!(is_newer("0.9.9", "1.0.0"));
        assert!(is_newer("0.1.0", "v0.1.1"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.2.0", "0.1.9"));
        assert!(!is_newer("1.0.0", "0.9.9"));
    }

    /// 比不出來就信 updater，不要因為版號寫得怪就擋掉更新
    #[test]
    fn is_newer_trusts_the_updater_when_it_cannot_tell() {
        assert!(is_newer("0.1.0", "怪版號"));
        assert!(is_newer("怪版號", "0.1.0"));
    }

    #[test]
    fn clean_notes_tidies_up_and_survives_nothing() {
        assert_eq!(clean_notes(None), "");
        assert_eq!(clean_notes(Some("  \n ")), "");
        assert_eq!(clean_notes(Some("第一行\r\n第二行\r\n")), "第一行\n第二行");
        let long: String = "字".repeat(5000);
        let out = clean_notes(Some(&long));
        assert!(out.chars().count() < 5000);
        assert!(out.ends_with("（後面省略，完整內容看下載頁）"));
    }

    #[test]
    fn short_date_keeps_only_the_day() {
        assert_eq!(short_date(Some("2026-08-19T10:20:30Z")), "2026-08-19");
        assert_eq!(short_date(Some("2026-08-19")), "2026-08-19");
        assert_eq!(short_date(Some("2026/08/19")), "");
        assert_eq!(short_date(Some("今天")), "");
        assert_eq!(short_date(None), "");
    }

    #[test]
    fn percent_needs_a_known_total() {
        assert_eq!(percent(0, Some(200)), Some(0));
        assert_eq!(percent(50, Some(200)), Some(25));
        assert_eq!(percent(200, Some(200)), Some(100));
        // 伺服器給的長度比實際短也不要爆掉
        assert_eq!(percent(300, Some(200)), Some(100));
        assert_eq!(percent(10, None), None);
        assert_eq!(percent(10, Some(0)), None);
    }

    /// 每一種失敗都要分得出來，不要全部變成同一句「更新失敗」
    #[test]
    fn friendly_error_tells_the_cases_apart() {
        let cases = [
            ("Updater does not have any endpoints set.", "沒有設定更新來源"),
            ("Could not fetch a valid release JSON from the remote", "還沒發過 Release"),
            ("the platform `darwin-aarch64` was not found in the response `platforms` object", "沒有這台機器的平台"),
            ("Download request failed with status: 404 Not Found", "404"),
            ("Download request failed with status: 403 Forbidden", "拒絕存取"),
            ("error sending request for url (https://github.com/...)", "連不上更新來源"),
            ("operation timed out", "連不上更新來源"),
            ("Signature verification failed", "簽章對不上"),
            ("expected value at line 1 column 1", "格式不對"),
            ("Permission denied (os error 13)", "沒有權限"),
            ("binary for the current target not found in the archive", "解不開"),
            ("Failed to determine updater package extract path.", "找不到要換掉的 app"),
            ("Authentication failed or was cancelled", "授權沒有通過"),
        ];
        for (raw, want) in cases {
            let got = friendly_error(raw);
            assert!(got.contains(want), "{} → {}（少了「{}」）", raw, got, want);
        }
    }

    /// 認不出來的錯誤保留原文，不要吞掉線索
    #[test]
    fn friendly_error_keeps_what_it_cannot_classify() {
        assert_eq!(friendly_error("something odd happened"), "something odd happened");
        assert_eq!(friendly_error("  怪怪的  "), "怪怪的");
    }

    /// 沒有新版的時候，除了目前版本以外都要是空的
    #[test]
    fn update_info_none_only_reports_the_current_version() {
        let info = UpdateInfo::none("0.1.0".into());
        assert!(!info.available);
        assert_eq!(info.current, "0.1.0");
        assert!(info.version.is_empty());
        assert!(info.notes.is_empty());
        assert_eq!(info.page, RELEASES_PAGE);
    }
}
