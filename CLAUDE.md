# 每日工作日誌 app

Rust（Tauri 2）＋ 靜態 HTML 的桌面 app，讀資料夾裡的 `<民國7碼>.md`（例如 `1150818.md`，
民國年 = 西元年 − 1911），把「每天做了什麼」與「每支工作項目走到哪一步」畫出來。
全貌看 `README.md`。

## 開始做事之前：先確認日誌規則裝好了

這個 app 只有在日誌檔照特定格式寫的時候才有東西可讀。格式是由 Claude Code 的
使用者層級提示詞維護的，範本在 `docs/worklog-rules.md`（設定頁的「日誌規則」也看得到、改得動，
改完會寫進 `~/.claude/CLAUDE.md`；`docs/` 那份是唯讀的預設值）。

所以接手這個專案時，**先檢查 `~/.claude/CLAUDE.md` 裡有沒有「每日工作日誌」那一段**：

- 有 → 什麼都不用做。
- 沒有 → 告訴使用者「你的 Claude 還沒有維護工作日誌的規則，這個 app 會沒有資料可讀」，
  並問要不要把 `docs/worklog-rules.md` 的「規則本文」整段附加到 `~/.claude/CLAUDE.md`。
  **問過再寫**，那是使用者的全域設定檔，不要自作主張改。

若使用者已經有自己的一套寫法（跟範本不同），不要覆蓋他的；改成回報差異，讓他決定要調規則還是調
`src-tauri/src/parser.rs` 的解析。

## 格式與解析是一對的

`docs/worklog-rules.md`（人怎麼寫）跟 `src-tauri/src/parser.rs`（程式怎麼讀）必須同步。
動其中一邊之前先看另一邊，並且**兩邊一起改**。解析規則都有測試，改完要跑 `cargo test`。

`docs/worklog-rules.md` 是用 `include_str!` 在編譯期嵌進 binary 的（`store.rs` 的 `RULES_DOC`），
所以：那個檔案**不能搬走或改名**，而且改完內容要重新編譯才會變成新的預設。

## 設定頁的「日誌規則」會改使用者的 CLAUDE.md

那一區編輯的是 `~/.claude/CLAUDE.md` 裡「# 每日工作日誌」那一段——Claude Code 真正會讀的地方。
寫入邏輯在 `store.rs`：`replace_rules_section()` 是純函式（整份文字 + 新規則 → 新的整份文字），
只換那一段、其他內容一個字都不動，寫之前先備份成 `CLAUDE.md.bak-<民國7碼>-<時分秒>`。
**只有使用者按按鈕才會寫**，不要加任何自動寫入。段落取代的測試不准碰真的 `~/.claude/CLAUDE.md`。

這份文件是給 Claude 看的，**app 不會照它改變解析行為**，使用者在那裡改了規則不等於 `parser.rs` 跟著變。

## 常用指令

```sh
cd src-tauri
cargo test                                 # 解析與寫檔規則的測試
cargo run                                  # 開發模式跑起來
cargo run --example probe                  # 不開視窗，直接印出解析結果
cargo run --example probe -- ~/某個資料夾
```

改前端（`ui/`）之後要重新編譯才會生效，因為前端是編進 binary 的。

**裝到 `/Applications` 只有一種正確做法**：換掉裡面的執行檔，不要複製 `target/release/bundle/macos/*.app`——
`cargo build --release` 只重編執行檔，**不會**更新那個打包好的 `.app`，複製過去等於裝了舊版
（這個坑踩過：連續幾輪修改都沒生效，因為一直在裝幾小時前的 bundle）。

```sh
cargo build --release
pkill -f 'worklog-app' || true
cp target/release/worklog-app "/Applications/每日工作日誌.app/Contents/MacOS/worklog-app"
codesign --force --deep --sign - "/Applications/每日工作日誌.app"
```

要重新產生整個 `.app`（換圖示、改版本、發 Release）才用 `cargo tauri build --bundles app`。
驗證裝對了沒：比對 `shasum` 或看 `ls -l` 的時間，別靠感覺。
安裝到 `/Applications` 的步驟看 `README.md` 的「打包與安裝」，
**換掉 binary 之後一定要 `codesign --force --deep --sign -` 重簽，不然 app 打不開**。

## 這個 app 對日誌檔的態度

只有兩條路會寫檔，兩條都在 `commands.rs` 的「寫進今天的日誌檔」那一區，而且都只動**今天**的檔案：

- TODO 頁的「加到今日日誌」（`append_entry`）：只 append 一行
- 看板拖卡片改狀態（`move_item`）：今天已經有那支工作項目的行就**只換行首的狀態標籤**
  （標題、連結、括號補充一個字都不動），沒有才新增一行

兩條都不重排、不改寫別的行。定位靠 `Entry.line`（解析時記下來的行號），不是拿整行去字串比對——
同一天可能有兩行長得一模一樣。純函式（`merge_entry_line`、`set_line_status`、`replace_status_at`）
都有測試，測試一律用臨時資料夾，不准碰使用者真的日誌資料夾。

要加新的寫檔功能前，先確認真的有必要。
