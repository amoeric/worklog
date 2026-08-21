//! 不開視窗，直接把某個資料夾解析結果印出來。
//!
//! ```sh
//! cargo run --example probe                      # 用設定裡的資料夾
//! cargo run --example probe -- ~/some/folder     # 指定資料夾
//! ```

use std::path::PathBuf;

use worklog_app::{parser, store};

fn main() {
    // 自訂狀態也要認得，不然使用者加的標籤會被當成看不懂的行
    worklog_app::model::set_table(store::load_statuses());

    let arg = std::env::args().nth(1);
    let folder = match arg {
        Some(p) => PathBuf::from(p),
        None => PathBuf::from(store::load_settings().folder),
    };

    println!("資料夾：{}", folder.display());
    if !folder.is_dir() {
        println!("  ！找不到這個資料夾");
        return;
    }

    let (mut days, skipped) = parser::scan(&folder);
    let items = parser::derive_items(&mut days);
    let projects = parser::project_list(&days);

    let entries: usize = days.iter().map(|d| d.entries.len()).sum();
    println!("日誌檔 {} 個、條目 {} 筆、工作項目 {} 支", days.len(), entries, items.len());
    println!("專案：{}", projects.join("、"));

    println!("\n--- 每天 ---");
    for d in &days {
        println!("{}  {} 筆", d.file, d.entries.len());
        for e in &d.entries {
            println!(
                "   [{}] {:<9} {}{}",
                e.project,
                e.status.clone().unwrap_or_else(|| "-".into()),
                e.title,
                e.item.as_ref().map(|i| format!("  → {}", i)).unwrap_or_default()
            );
        }
    }

    println!("\n--- 工作項目 ---");
    for it in &items {
        println!(
            "{:<10} {:<32} {} 起，{} 筆歷程  {}",
            it.status,
            it.id,
            it.since,
            it.history.len(),
            it.title
        );
    }

    if !skipped.is_empty() {
        println!("\n--- 沒讀懂的行 ---");
        for s in &skipped {
            println!("  {}", s);
        }
    }
}
