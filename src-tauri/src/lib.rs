//! 每日工作日誌：讀資料夾裡的 `<年>/<月>/<西元8碼>.md`，把日期視角與工作項目視角算出來。
//!
//! 拆成 lib 是為了讓 `examples/probe.rs` 可以不開視窗就驗證解析結果。

pub mod commands;
pub mod link;
pub mod model;
pub mod parser;
pub mod store;
pub mod update;
