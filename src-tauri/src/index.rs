//! 給 Claude Code 讀的工作項目索引（`_items.md`）。
//!
//! 為什麼要這個檔：日誌規則要求同一支 change 的每一行都帶同一個 `slug：` 前綴，
//! 但 Claude 每天開新對話，記不得前幾天用過哪些 slug，就會替同一件事另外發明一個，
//! 看板上那支 change 於是裂成好幾張卡。這裡把「還沒歸檔的工作項目 → slug」寫成一份
//! 清單放在日誌資料夾根目錄，規則本文叫 Claude 動筆前先讀它，照抄既有的 slug。
//!
//! 這個檔是**產出**，不是資料：內容整份重寫，使用者手改會被蓋掉（檔頭有寫）。
//! 它不會被解析回來——`parser::is_log_file()` 只認 8 碼數字檔名，
//! 而且 `scan_dir()` 只往數字資料夾裡走，`_items.md` 在根目錄，兩道都擋掉。
//!
//! 產生內容的 [`render`] 是純函式，寫檔的 [`write`] 只在內容真的變了才動硬碟。

use std::path::Path;

use crate::model::{status_by_id, Item};

/// 索引檔名。放在日誌資料夾根目錄。
pub const FILE: &str = "_items.md";

/// 已經結束的項目要留在清單上幾天。
///
/// 留一小段是因為結束後偶爾還有收尾（補文件、跟進 bug），那時候還是得沿用同一個 slug；
/// 但留太久清單只會越長越吵，Claude 要掃的東西變多。
const KEEP_FINISHED_DAYS: i64 = 30;

/// `20260817` → 從 0001-01-01 起算的天數。看不懂的字串回 None。
///
/// 只拿來算兩個日期差幾天，不需要真正的曆法物件，所以自己算。
fn day_number(code: &str) -> Option<i64> {
    if code.len() != 8 || !code.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let y: i64 = code[0..4].parse().ok()?;
    let m: i64 = code[4..6].parse().ok()?;
    let d: i64 = code[6..8].parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    // Howard Hinnant 的 days_from_civil：把三月當成年初，閏日就落在年尾，不用特判
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146097 + doe)
}

/// `20260817` → `2026-08-17`；看不懂就原樣回傳
fn dashed(code: &str) -> String {
    match day_number(code) {
        Some(_) => format!("{}-{}-{}", &code[0..4], &code[4..6], &code[6..8]),
        None => code.to_string(),
    }
}

/// 這件事結束了沒。已歸檔，或落在不走生命週期的狀態（內建的 `完成`、
/// 以及使用者自己加的非生命週期狀態）都算結束。
fn finished(status: &str) -> bool {
    if status == "archived" {
        return true;
    }
    // 查不到的狀態當成還在進行，寧可多列也不要漏掉
    status_by_id(status).map(|s| !s.lifecycle).unwrap_or(false)
}

/// 這支工作項目要不要列進清單。
///
/// - `auto-…` 開頭的是解析時算出來的 fallback id，不是人寫的 slug，抄過去沒有意義
/// - 已經結束的只留最近 [`KEEP_FINISHED_DAYS`] 天
fn keep(item: &Item, today: &str) -> bool {
    if item.id.starts_with("auto-") {
        return false;
    }
    if !finished(&item.status) {
        return true;
    }
    match (day_number(today), day_number(&item.since)) {
        (Some(now), Some(then)) => now - then <= KEEP_FINISHED_DAYS,
        // 日期看不懂就留著，寧可多列也不要漏掉還在進行的東西
        _ => true,
    }
}

/// 這支項目最後一次出現在日誌裡是哪一天
fn last_seen(item: &Item) -> String {
    item.history
        .last()
        .map(|p| p.code.clone())
        .unwrap_or_else(|| item.since.clone())
}

/// 「這是什麼」那一欄最多幾個字。
///
/// 這欄只是讓 Claude 認出「喔這件事我做過」，不是完整說明；
/// 日誌裡偶爾會出現一整段的標題，整段抄過來只會把表格撐爛、也吃掉 Claude 的注意力。
const TITLE_MAX: usize = 40;

/// 表格欄位裡的 `|` 會把欄切斷，換成全形的
fn cell(s: &str) -> String {
    s.trim().replace('|', "｜")
}

/// 太長的標題截短。按字元數算，中文一個字就是一個字元。
fn short(s: &str) -> String {
    let t = cell(s);
    if t.chars().count() <= TITLE_MAX {
        return t;
    }
    t.chars().take(TITLE_MAX).collect::<String>() + "…"
}

/// 把工作項目排成索引檔的內容（純函式）。
///
/// `today` 是西元 8 碼，用來算已歸檔的項目過期沒。
/// 項目照傳進來的順序分到各自的專案底下，專案第一次出現的順序就是區塊順序。
pub fn render(items: &[Item], today: &str) -> String {
    let mut out = String::new();
    out.push_str("# 工作項目索引\n\n");
    out.push_str("這份檔案由「每日工作日誌」app 自動產生，每次開啟或寫入日誌時整份重寫。\n");
    out.push_str("**手動編輯會被蓋掉。**\n\n");
    out.push_str("寫今天的日誌之前先讀這裡：要記的事情如果已經在下面，就沿用它的 slug，\n");
    out.push_str("不要另外取一個新的——同一支 change 換了 slug，看板上就會裂成兩張卡。\n");
    out.push_str("下面沒有的才是新的一件事，這時才取新 slug。\n\n");
    out.push_str(&format!("最後更新：{}\n", dashed(today)));

    let live: Vec<&Item> = items.iter().filter(|i| keep(i, today)).collect();

    if live.is_empty() {
        out.push_str("\n（目前沒有帶 slug 的工作項目）\n");
        return out;
    }

    // 專案照第一次出現的順序排，項目維持 items 原本的排序（狀態表順序）
    let mut projects: Vec<String> = Vec::new();
    for i in &live {
        if !projects.iter().any(|p| p == &i.project) {
            projects.push(i.project.clone());
        }
    }

    for project in projects {
        out.push_str(&format!("\n## {}\n\n", project));
        out.push_str("| slug | 這是什麼 | 目前狀態 | 最後出現 |\n");
        out.push_str("| --- | --- | --- | --- |\n");
        for i in live.iter().filter(|i| i.project == project) {
            let status = status_by_id(&i.status).map(|s| s.zh).unwrap_or_else(|| i.status.clone());
            out.push_str(&format!(
                "| `{}` | {} | {} | {} |\n",
                cell(&i.id),
                short(&i.title),
                cell(&status),
                dashed(&last_seen(i)),
            ));
        }
    }

    out
}

/// 把索引寫進日誌資料夾。內容跟現有的一樣就不動硬碟。
///
/// 寫不成功不是大事（資料夾唯讀、被別的程式鎖住…），回報但不擋住讀日誌，
/// 所以回傳 `Result` 讓呼叫端自己決定要不要理。
pub fn write(folder: &Path, items: &[Item], today: &str) -> Result<(), String> {
    let path = folder.join(FILE);
    let text = render(items, today);
    if let Ok(old) = std::fs::read_to_string(&path) {
        if old == text {
            return Ok(());
        }
    }
    std::fs::write(&path, text).map_err(|e| format!("寫不出 {}：{}", path.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::HistoryPoint;

    fn point(code: &str) -> HistoryPoint {
        HistoryPoint {
            code: code.to_string(),
            status: Some("building".into()),
            title: "x".into(),
            url: None,
            project: "proj".into(),
            detail: Vec::new(),
        }
    }

    fn item(id: &str, project: &str, title: &str, status: &str, since: &str) -> Item {
        Item {
            id: id.to_string(),
            project: project.to_string(),
            title: title.to_string(),
            issue: None,
            mr: None,
            status: status.to_string(),
            since: since.to_string(),
            history: vec![point(since)],
        }
    }

    #[test]
    fn day_number_counts_days_between_dates() {
        let a = day_number("20260817").unwrap();
        let b = day_number("20260901").unwrap();
        assert_eq!(b - a, 15);
        // 跨閏年 2 月
        let c = day_number("20240228").unwrap();
        let d = day_number("20240301").unwrap();
        assert_eq!(d - c, 2);
        assert!(day_number("2026081").is_none());
        assert!(day_number("2026ab17").is_none());
    }

    #[test]
    fn lists_slugged_items_grouped_by_project() {
        let items = vec![
            item("search-history", "chat_hub", "對話紀錄搜尋", "building", "20260824"),
            item("entry-detail", "worklog-app", "日誌摘要詳情", "proposing", "20260825"),
        ];
        let out = render(&items, "20260825");
        assert!(out.contains("## chat_hub"));
        assert!(out.contains("| `search-history` | 對話紀錄搜尋 | 實作中 | 2026-08-24 |"));
        assert!(out.contains("## worklog-app"));
        assert!(out.contains("| `entry-detail` | 日誌摘要詳情 | 提案中 | 2026-08-25 |"));
    }

    #[test]
    fn skips_auto_ids() {
        let items = vec![item("auto-0123456789abcdef", "proj", "沒有 slug 的事", "building", "20260825")];
        let out = render(&items, "20260825");
        assert!(!out.contains("auto-"));
        assert!(out.contains("目前沒有帶 slug 的工作項目"));
    }

    #[test]
    fn one_off_done_items_also_expire() {
        let items = vec![
            item("old-chore", "proj", "很久以前做完的雜事", "done", "20260701"),
            item("today-chore", "proj", "今天做完的雜事", "done", "20260825"),
        ];
        let out = render(&items, "20260825");
        assert!(!out.contains("old-chore"));
        assert!(out.contains("today-chore"));
    }

    #[test]
    fn drops_long_archived_items_but_keeps_recent_ones() {
        let items = vec![
            item("old-thing", "proj", "很久以前歸檔", "archived", "20260701"),
            item("just-merged", "proj", "剛剛歸檔", "archived", "20260820"),
            item("in-progress", "proj", "還在做", "building", "20260601"),
        ];
        let out = render(&items, "20260825");
        assert!(!out.contains("old-thing"));
        assert!(out.contains("just-merged"));
        // 進行中的不管多舊都留著
        assert!(out.contains("in-progress"));
    }

    #[test]
    fn long_titles_are_cut_short() {
        let long = "一".repeat(80);
        let items = vec![item("a-slug", "proj", &long, "building", "20260825")];
        let out = render(&items, "20260825");
        let row = out.lines().find(|l| l.contains("a-slug")).unwrap();
        assert!(row.contains("…"));
        // 截到 40 個字，加上省略號
        assert!(row.contains(&("一".repeat(40) + "…")));
        assert!(!row.contains(&"一".repeat(41)));
    }

    #[test]
    fn escapes_pipes_so_the_table_does_not_break() {
        let items = vec![item("a-b-c", "proj", "標題裡有 | 管線", "building", "20260825")];
        let out = render(&items, "20260825");
        assert!(out.contains("標題裡有 ｜ 管線"));
    }

    #[test]
    fn write_creates_the_file_and_skips_identical_rewrites() {
        let dir = std::env::temp_dir().join(format!("worklog-index-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let items = vec![item("a-slug", "proj", "一件事", "building", "20260825")];

        write(&dir, &items, "20260825").unwrap();
        let path = dir.join(FILE);
        let first = std::fs::metadata(&path).unwrap().modified().unwrap();

        // 內容一樣就不該再寫一次
        write(&dir, &items, "20260825").unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), first);
        assert!(std::fs::read_to_string(&path).unwrap().contains("a-slug"));

        std::fs::remove_dir_all(&dir).ok();
    }
}
