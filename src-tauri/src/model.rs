//! 資料結構與狀態表。
//!
//! 日誌檔是唯讀來源：一天一個 `<民國7碼>.md`。
//! 工作項目（item）不是檔案裡直接寫的，是從條目推導出來的——
//! 推導規則在 `parser.rs`，這裡只放型別。

use serde::{Deserialize, Serialize};

/// 七個工作狀態。前六個走生命週期，`done` 是一次性工作，不進流程軌。
/// `zh` 是日誌檔裡實際寫的中文標籤，`label` 是介面上顯示的英文。
pub struct Status {
    pub id: &'static str,
    pub label: &'static str,
    pub zh: &'static str,
    pub hint: &'static str,
    pub lifecycle: bool,
    /// 流程軌上與另一個狀態並列成分支（Parked / Building）
    pub branch: bool,
}

pub const STATUSES: &[Status] = &[
    Status { id: "todo",      label: "Todo",      zh: "待辦",   hint: "只有議題，還沒開始寫提案",       lifecycle: true,  branch: false },
    Status { id: "proposing", label: "Proposing", zh: "提案中", hint: "正在寫提案與規格，文件還沒齊",   lifecycle: true,  branch: false },
    Status { id: "parked",    label: "Parked",    zh: "暫存",   hint: "提案與規格都完成，刻意擱著等排程", lifecycle: true,  branch: true  },
    Status { id: "building",  label: "Building",  zh: "實作中", hint: "已開分支動工，程式碼正在寫",     lifecycle: true,  branch: true  },
    Status { id: "review",    label: "Review",    zh: "待合併", hint: "已開 MR，等審查與合併",          lifecycle: true,  branch: false },
    Status { id: "archived",  label: "Archived",  zh: "已歸檔", hint: "MR 已合併、規格已歸檔，這件事結束了", lifecycle: true, branch: false },
    Status { id: "done",      label: "Done",      zh: "完成",   hint: "不走生命週期的一次性工作",       lifecycle: false, branch: false },
];

pub fn status_by_zh(zh: &str) -> Option<&'static Status> {
    STATUSES.iter().find(|s| s.zh == zh)
}

pub fn status_by_id(id: &str) -> Option<&'static Status> {
    STATUSES.iter().find(|s| s.id == id)
}

/// 送給前端的狀態表，順序就是流程軌的順序。
#[derive(Serialize)]
pub struct StatusDto {
    pub id: &'static str,
    pub label: &'static str,
    pub zh: &'static str,
    pub hint: &'static str,
    pub lifecycle: bool,
    pub branch: bool,
}

pub fn status_table() -> Vec<StatusDto> {
    STATUSES
        .iter()
        .map(|s| StatusDto {
            id: s.id,
            label: s.label,
            zh: s.zh,
            hint: s.hint,
            lifecycle: s.lifecycle,
            branch: s.branch,
        })
        .collect()
}

/// 日誌檔裡的一行條目。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// 所屬專案，來自上方最近的 `## 專案名`
    pub project: String,
    /// 狀態 id；沒標狀態的行是 None
    pub status: Option<String>,
    /// 條目標題（連結的話是連結文字）
    pub title: String,
    /// 連結網址
    pub url: Option<String>,
    /// 連結後面的括號補充
    pub note: Option<String>,
    /// 歸戶到哪個工作項目（slug）；歸不到是 None
    pub item: Option<String>,
    /// 原始那一行，出問題時可以對照
    pub raw: String,
    /// 在 app 裡拖出來、還沒寫進 .md 的變更
    #[serde(default)]
    pub pending: bool,
}

/// 一天。
#[derive(Debug, Clone, Serialize)]
pub struct Day {
    /// 民國 7 碼，例如 1150817
    pub code: String,
    pub file: String,
    pub entries: Vec<Entry>,
}

/// 工作項目歷程上的一個點。
#[derive(Debug, Clone, Serialize)]
pub struct HistoryPoint {
    pub code: String,
    pub status: Option<String>,
    pub title: String,
    pub url: Option<String>,
    pub project: String,
    /// 這一筆還沒寫回 .md
    pub pending: bool,
}

/// 推導出來的工作項目。
#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub id: String,
    pub project: String,
    pub title: String,
    pub issue: Option<String>,
    pub mr: Option<String>,
    /// 目前狀態：最後一筆帶生命週期狀態的條目
    pub status: String,
    /// 進入目前狀態的日期
    pub since: String,
    pub history: Vec<HistoryPoint>,
}

/// 一次讀完整個資料夾的結果，前端拿到這包就夠畫所有頁面。
#[derive(Debug, Clone, Serialize)]
pub struct Workspace {
    pub folder: String,
    pub folder_exists: bool,
    pub today: String,
    pub days: Vec<Day>,
    pub items: Vec<Item>,
    pub projects: Vec<String>,
    /// 解析時看不懂的行，連同檔名一起回報，不吞掉
    pub skipped: Vec<String>,
    /// 還沒寫進 .md 的變更，依日期排好
    pub pending: Vec<PendingPoint>,
}

/// 一筆待寫回的變更，前端要能直接印出那一行 markdown。
#[derive(Debug, Clone, Serialize)]
pub struct PendingPoint {
    pub code: String,
    pub entry: Entry,
}
