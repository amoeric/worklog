//! 資料結構與狀態表。
//!
//! 日誌檔一天一個 `<年>/<月>/<西元8碼>.md`。
//! 工作項目（item）不是檔案裡直接寫的，是從條目推導出來的——
//! 推導規則在 `parser.rs`，這裡只放型別。

use std::sync::{LazyLock, RwLock};

use serde::{Deserialize, Serialize};

/// 八個工作狀態。前七個走生命週期，`done` 是一次性工作，不進流程軌。
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
    Status { id: "testing",   label: "Testing",   zh: "測試中", hint: "已推上 staging／測試環境，等驗證", lifecycle: true,  branch: false },
    Status { id: "review",    label: "Review",    zh: "待合併", hint: "已開 MR，等審查與合併",          lifecycle: true,  branch: false },
    Status { id: "archived",  label: "Archived",  zh: "已歸檔", hint: "MR 已合併、規格已歸檔，這件事結束了", lifecycle: true, branch: false },
    Status { id: "done",      label: "Done",      zh: "完成",   hint: "不走生命週期的一次性工作",       lifecycle: false, branch: false },
];

pub fn status_by_zh(zh: &str) -> Option<StatusDto> {
    status_table().into_iter().find(|s| s.zh == zh)
}

pub fn status_by_id(id: &str) -> Option<StatusDto> {
    status_table().into_iter().find(|s| s.id == id)
}

/// 自訂狀態的三個顏色。內建八個的顏色寫在前端的 Tailwind 設定裡，這裡是 None。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusColor {
    /// 實心圓點
    pub dot: String,
    /// 淺色底
    pub tint: String,
    /// 深色底
    pub dtint: String,
}

/// 狀態表上的一格。內建八個是 `builtin: true`，其餘是使用者自己加的。
///
/// 前端拿到的順序就是看板由左到右的欄序。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusDto {
    pub id: String,
    pub label: String,
    pub zh: String,
    pub hint: String,
    #[serde(default)]
    pub lifecycle: bool,
    #[serde(default)]
    pub branch: bool,
    #[serde(default)]
    pub color: Option<StatusColor>,
    #[serde(default)]
    pub builtin: bool,
}

/// 內建那八個，照寫死的順序。
pub fn builtin_table() -> Vec<StatusDto> {
    STATUSES
        .iter()
        .map(|s| StatusDto {
            id: s.id.to_string(),
            label: s.label.to_string(),
            zh: s.zh.to_string(),
            hint: s.hint.to_string(),
            lifecycle: s.lifecycle,
            branch: s.branch,
            color: None,
            builtin: true,
        })
        .collect()
}

/// 目前生效的狀態表。
///
/// 解析（`parser.rs`）與寫檔（`commands.rs`）都要認得使用者自己加的狀態，
/// 所以表放在這裡讓兩邊共用；app 啟動時由 `store::load_statuses()` 灌進來。
/// 沒灌之前就是內建那八個——測試因此不必碰使用者的設定檔。
static TABLE: LazyLock<RwLock<Vec<StatusDto>>> = LazyLock::new(|| RwLock::new(builtin_table()));

pub fn set_table(list: Vec<StatusDto>) {
    if let Ok(mut t) = TABLE.write() {
        *t = list;
    }
}

pub fn status_table() -> Vec<StatusDto> {
    TABLE.read().map(|t| t.clone()).unwrap_or_else(|_| builtin_table())
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
    /// 這一行在檔案裡的行號（0 起算）。看板改狀態時靠它定位到「就是這一行」，
    /// 不用字串比對整行去猜——一模一樣的行可能出現兩次。
    #[serde(default)]
    pub line: usize,
}

/// 一天。
#[derive(Debug, Clone, Serialize)]
pub struct Day {
    /// 西元 8 碼，例如 20260817
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
}
