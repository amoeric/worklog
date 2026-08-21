//! 把日誌裡所有 GitLab／Redmine 連結的目前狀態問一遍。
//!
//! ```sh
//! cargo run --example mrstate
//! ```
//!
//! 用途：日誌寫下去的狀態是「當天推進到哪」，不會自己更新；
//! 想知道那些 MR 後來合併了沒，用這支對一次。

use std::collections::BTreeMap;
use std::path::PathBuf;

use worklog_app::{link, parser, store};

fn main() {
    let settings = store::load_settings();
    let folder = PathBuf::from(&settings.folder);
    let (days, _) = parser::scan(&folder);

    // url -> (最早出現的那天, 標題)
    let mut links: BTreeMap<String, (String, String)> = BTreeMap::new();
    for d in &days {
        for e in &d.entries {
            if let Some(u) = &e.url {
                links.entry(u.clone()).or_insert((d.code.clone(), e.title.clone()));
            }
        }
    }

    println!("要查 {} 個連結\n", links.len());
    for (url, (code, title)) in &links {
        match link::fetch(url, &settings) {
            Ok(c) => println!("{}\t{}\t{}\t{}\t{}", code, c.state, c.reference, title, url),
            Err(e) => println!("{}\t查不到\t-\t{}\t{}（{}）", code, title, url, e),
        }
    }
}
