# 每日工作日誌 app

Rust（Tauri 2）＋ 靜態 HTML 的桌面 app，讀資料夾裡的 `<西元年>/<月>/<西元8碼>.md`（例如
`2026/08/20260818.md`），把「每天做了什麼」與「每支工作項目走到哪一步」畫出來。
使用說明看 `README.md`（**只寫給使用者**：安裝、日誌怎麼寫、每頁能幹嘛、會動到哪些檔案）。
開發、打包、發版、目錄結構、視覺規範在 `docs/development.md`。
**不要把開發或實作細節寫回 README。**

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

## 狀態表是動態的

內建八個是 `model.rs` 的 `STATUSES`（const），但使用者可以在看板加自己的狀態，整張表存在
設定目錄的 `statuses.json`。`main.rs` 開機時 `model::set_table(store::load_statuses())` 灌進去，
之後 `status_by_id()` / `status_by_zh()` / `status_table()` 查的都是那張動態表——**不要再直接讀
`STATUSES` const**，那只是預設值（`builtin_table()` 用它，`merge_builtin()` 負責把缺的補回來）。

沒灌之前就是內建那八個，所以測試不必碰使用者的設定檔；`examples/` 裡的工具要自己灌一次，
不然使用者自訂的標籤會被當成看不懂的行。

新增狀態會問要不要同步到 `~/.claude/CLAUDE.md` 的規則（`insert_status_into_rules()` 是純函式，
只動生命週期那行與狀態表格，有測試）。一樣是**使用者按了才寫**。

## 設定頁的「日誌規則」會改使用者的 CLAUDE.md

那一區編輯的是 `~/.claude/CLAUDE.md` 裡「# 每日工作日誌」那一段——Claude Code 真正會讀的地方。
寫入邏輯在 `store.rs`：`replace_rules_section()` 是純函式（整份文字 + 新規則 → 新的整份文字），
只換那一段、其他內容一個字都不動，寫之前先備份成 `CLAUDE.md.bak-<西元8碼>-<時分秒>`。
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
open "/Applications/每日工作日誌.app"          # ← 這行不能省
```

**最後那行 `open` 是步驟的一部分，不是可選的。** 前面的 `pkill` 把使用者正在用的視窗關掉了，
不開回來的話他桌上只剩被關掉前的印象，會以為改動沒生效——而且沒辦法驗收。
（這個坑踩過：連續幾輪都有正確安裝，但每輪都把 app 殺掉不開回來，使用者看到的一直是舊畫面。）

要重新產生整個 `.app`（換圖示、改版本、發 Release）才用 `cargo tauri build --bundles app`。
驗證裝對了沒：看跑起來的是不是剛裝的那個執行檔，別靠感覺——

```sh
PID=$(pgrep -f worklog-app | head -1)
ps -p "$PID" -o comm=            # 路徑要是 /Applications/… 那個
ls -l "$(ps -p "$PID" -o comm=)" # 時間要是剛才那一分鐘
```
安裝到 `/Applications` 的步驟看 `README.md` 的「打包與安裝」，
**換掉 binary 之後一定要 `codesign --force --deep --sign -` 重簽，不然 app 打不開**。

## 摘要與詳情

一條日誌是「摘要一行 ＋ 選擇性的詳情」。詳情是那一行底下縮排兩格的子 bullet，
解析時收進 `Entry.detail`（以前是直接丟掉的），跟著 `HistoryPoint.detail` 走到工作項目頁。
摘要必須自己看得懂，詳情只放 MR / issue 上找不到的東西——寫法規則在 `docs/worklog-rules.md`。

前端兩處會顯示：日誌頁摘要右邊有展開鈕（預設收起），工作項目頁的歷程直接攤開。
兩處都用 `Shell.inlineMarkdown`（只跑行內語法，不包段落）。

## `_items.md`：讓 Claude 知道這是同一件事

`index.rs` 在每次 `load_workspace()` 時，把「還沒歸檔、且有真 slug 的工作項目」寫成
日誌資料夾根目錄的 `_items.md`。規則本文叫 Claude 動筆前先讀它，照抄既有的 slug——
不然每天開新對話的 Claude 會替同一支 change 另外發明一個名字，看板上就裂成兩張卡。

- 這是**產出**不是資料：整份重寫，內容沒變就不碰硬碟
- 不會被解析回來：`is_log_file()` 只認 8 碼數字檔名，`scan_dir()` 又只往數字資料夾走
- `auto-…` 開頭的 fallback id 不是人寫的 slug，不列進去
- 已經結束的（已歸檔，或落在非生命週期狀態如 `完成`）留 30 天（`KEEP_FINISHED_DAYS`），
  留一段是因為結束後偶爾還有收尾

`render()` 是純函式，有測試；`write()` 失敗只會進 `Workspace.skipped`，不擋讀日誌。

## Spectra 標記

帶真 slug 的工作項目是 Spectra change，卡片與標題前面會出現 Spectra 的 app icon。
判準跟 `_items.md` 一模一樣：`id` 不是 `auto-` 開頭就是（`store.js` 的 `isSpectra()`），
所以兩邊要改就一起改。

圖是從 `/Applications/Spectra.app/Contents/Resources/icon.icns` 抽出來縮成 64px 再
base64 內嵌在 `store.js` 的 `SPECTRA_ICON`——前端整包編進 binary，讀不到外部檔案。
換圖就重抽一次，不要改成外部路徑。

顯示的地方只有「工作項目」那三頁（看板卡片、狀態清單、工作項目頁），
日誌頁是「條目」不是「項目」，刻意不放。

## 這個 app 對日誌檔的態度

只有兩條路會寫檔，兩條都在 `commands.rs` 的「寫進今天的日誌檔」那一區，而且都只動**今天**的檔案：

- TODO 頁的「加到今日日誌」（`append_entry`）：只 append 一行
- 看板拖卡片改狀態（`move_item`）：今天已經有那支工作項目的行就**只換行首的狀態標籤**
  （標題、連結、括號補充一個字都不動），沒有才新增一行

兩條都不重排、不改寫別的行。定位靠 `Entry.line`（解析時記下來的行號），不是拿整行去字串比對——
同一天可能有兩行長得一模一樣。純函式（`merge_entry_line`、`set_line_status`、`replace_status_at`）
都有測試，測試一律用臨時資料夾，不准碰使用者真的日誌資料夾。

要加新的寫檔功能前，先確認真的有必要。
