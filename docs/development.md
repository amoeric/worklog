# 開發

這份是給要改這個 app 的人看的。使用說明在 [README](../README.md)。

## 跑起來

```sh
cd src-tauri
cargo run                                 # 開發模式
cargo test                                # 解析與寫檔規則的測試
cargo run --example probe                 # 不開視窗，只印解析結果
cargo run --example probe -- ~/某個資料夾   # 指定資料夾
```

前端在 `ui/`，是用 `include_str!` 編進 binary 的，**改完要重新編譯才會生效**。

## 兩份文件要一起改

`docs/worklog-rules.md`（人怎麼寫日誌）跟 `src-tauri/src/parser.rs`（程式怎麼讀）是一對的，
動其中一邊之前先看另一邊。解析規則都有測試，改完跑 `cargo test`。

`docs/worklog-rules.md` 也是用 `include_str!` 嵌進 binary 的（`store.rs` 的 `RULES_DOC`），
所以那個檔**不能搬走或改名**，改完要重新編譯才會變成新的預設值。

## 結構

```
ui/                前端（靜態檔，沒有 build step）
  index.html       月曆首頁
  board.html       看板（拖曳改狀態）
  js/store.js      向後端要資料，所有頁面共用；頁面程式包在 Log.ready() 裡；搜尋的比對規則也在這
  js/shell.js      頂端工具列（含搜尋框）、連結面板、提示（toast）、資料夾異常橫幅
  css/app.css      全站隱藏捲軸
  vendor/          Tailwind Play CDN 的本地副本
  fonts/           Inter，離線可用（其餘用系統字）
src-tauri/src/
  parser.rs        md 解析與工作項目歸戶（規則都有測試）
  model.rs         狀態表與資料結構
  index.rs         產生給 Claude 讀的 `_items.md` slug 索引（純函式，有測試）
  store.rs         設定（含外部服務 token）、日誌規則（讀寫 ~/.claude/CLAUDE.md）、TODO
  link.rs          連結解析與 Redmine／GitLab API、附圖代抓（解析與檢查規則有測試，不打網路）
  commands.rs      前端呼叫的指令（含 move_item / append_entry / fetch_link / fetch_image；寫 .md 的規則與測試都在這）
  update.rs        線上更新（讀 GitHub Releases 的 latest.json、下載安裝；版本比較與錯誤翻譯有測試，不打網路）
```

## 視覺

macOS 行事曆（月檢視）風格，規範是設計稿的 `brand-spec.md`：

- 幾乎沒有顏色。白面板、灰工具列、1px 髮絲線就是全部的分隔
- 月格是全出血的髮絲線網格，不加圓角、不加卡片
- 清單一律「群組內嵌清單」：白面板 + 圓角 12px + 列與列之間一條髮絲線
- 按鈕是膠囊（`rounded-full`）+ 1px 邊 + 極淡陰影
- 狀態＝行事曆的事件：淡色底 + 實心圓點，沒有外框也沒有圖示
- 只有「今天」與主要動作是紅的（`accent`）
- 字用系統字（SF Pro / PingFang TC），不再用 Cal Sans
- 色票在 `ui/js/tailwind-config.js`（`st.<狀態>.dot / tint / dtint`），
  取用一律透過 `Log.statusClass()`、`Log.groupClass()`、`Log.statusDotClass()`、`Log.eventRow()`

導覽是頂端一條工具列：左邊品牌、麵包屑與搜尋框（只有月曆／看板／日誌有），正中間是分段控制，
右邊是深淺色切換。搜尋框是膠囊、髮絲線邊、左邊一個放大鏡，有字才出現右邊的清除鈕；
寬度有上限，窄視窗自己縮，不會把麵包屑擠掉。
每一頁都鋪滿寬度（不收窄置中），切分頁時上方不會跳動。捲軸全站隱藏（`ui/css/app.css`），
所以可捲動的區域**盡量只留一層**：連結面板就只有主體那一根捲軸，標題列／內容區／底部連結列
左右內距一致（`px-6`），內容區底下多留一段空白，配上連結列的髮絲線，捲到底看得出來下面還有東西。

## 自己打包

```sh
cargo install tauri-cli --version "^2" --locked   # 只要裝一次
cd src-tauri
cargo tauri build
```

產出在 `target/release/bundle/`（`macos/` 是 .app、`dmg/` 是 .dmg）。

換圖示（原稿是 `icon.svg`）：

```sh
rsvg-convert -w 840 -h 840 icon.svg -o /tmp/i.png
magick -size 1024x1024 xc:none /tmp/i.png -gravity center -composite /tmp/icon.png
cargo tauri icon /tmp/icon.png          # 會蓋掉 src-tauri/icons/
```

## 發新版

更新檔放在 GitHub Releases（<https://github.com/amoeric/worklog>），app 去讀的是固定網址
`https://github.com/amoeric/worklog/releases/latest/download/latest.json`。

1. 改 `src-tauri/tauri.conf.json` 的 `version`（例如 `0.1.0` → `0.1.1`）。
   版號只能往上加，app 會比對版號，比現在的小或一樣就當作沒有新版。
2. 帶著私鑰打包（`bundle.createUpdaterArtifacts` 已經開著，所以會多產出更新檔與簽章）：

```sh
cd src-tauri
TAURI_SIGNING_PRIVATE_KEY="$HOME/.tauri/worklog.key" \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="" \
cargo tauri build
```

變數名是 `TAURI_SIGNING_PRIVATE_KEY`（值可以是私鑰檔的路徑）。**不是 `..._PATH`**——
名字寫錯的話 bundle 照樣產得出來，只是最後會說「找到公鑰但沒有私鑰」，`.sig` 不會生出來。

3. 產出在 `src-tauri/target/release/bundle/macos/`：
   - `每日工作日誌.app.tar.gz` ← 更新檔
   - `每日工作日誌.app.tar.gz.sig` ← 簽章，內容是一串 base64，等一下要整個貼進 `latest.json`
4. 在 GitHub 開一個 Release（tag 例如 `v0.1.1`），把 `.app.tar.gz` 與**自己寫的 `latest.json`** 一起上傳。
   `latest.json` 這個檔名不能改，endpoint 是寫死的。

`latest.json` 長這樣：

```json
{
  "version": "0.1.1",
  "notes": "設定頁加了「版本與更新」，可以在 app 裡檢查與安裝新版。",
  "pub_date": "2026-08-19T10:00:00Z",
  "platforms": {
    "darwin-aarch64": {
      "signature": "把 .app.tar.gz.sig 的內容整個貼進來",
      "url": "https://github.com/amoeric/worklog/releases/download/v0.1.1/%E6%AF%8F%E6%97%A5%E5%B7%A5%E4%BD%9C%E6%97%A5%E8%AA%8C.app.tar.gz"
    }
  }
}
```

- `version` 不要加 `v`，要跟 `tauri.conf.json` 的一致
- `pub_date` 是 RFC 3339，app 只顯示前面的 `YYYY-MM-DD`
- `notes` 會原樣顯示在設定頁的「版本與更新」，寫給人看的中文就好
- `platforms` 的 key 是「系統-架構」：Apple Silicon 是 `darwin-aarch64`，Intel 是 `darwin-x86_64`。
  兩種機器都要照顧就各打一份、各放一個 key；沒有對應的 key，那台機器會被告知「沒有這台機器的平台」
- `url` **一定要照 Release 頁面上那顆檔案實際的名字寫**。GitHub 上傳時會把檔名裡的
  非 ASCII 字元**整段拿掉**（不是 percent-encode）：`每日工作日誌.app.tar.gz` 上去之後
  叫 `app.tar.gz`，`每日工作日誌_0.1.5_aarch64.dmg` 叫 `_0.1.5_aarch64.dmg`。
  照原本的中文檔名去組網址會 404，而且 app 那邊只會說「下載失敗」，很難看出是這個原因。
  發完用 `gh release view <tag> --json assets` 對一次名字，再 `curl -I` 那個網址確認是 200

### 私鑰

簽章用的私鑰在 `~/.tauri/worklog.key`，對應的公鑰已經寫在 `tauri.conf.json` 的
`plugins.updater.pubkey`。

- **弄丟就再也發不了更新。** app 只認這一把私鑰簽出來的更新檔，換一把等於要每個人手動重裝
- **絕對不能進版控**，也不要貼進 issue、log 或截圖
- 打包時用環境變數餵進去就好，不要寫進任何檔案

### 線上更新不保證成功

這個 app 是 ad-hoc 簽章、**沒有 Apple 公證**。從 GitHub 下載回來的 `.app.tar.gz` 會帶
quarantine 標記，macOS 有可能直接擋下來或跳警告，所以更新這件事只能盡力，不能保證：

- 設定頁的「版本與更新」失敗時會寫出是哪一種問題（連不上／還沒有 Release／簽章對不上／沒有權限…）
- 旁邊一直留著「開啟下載頁」，隨時可以自己抓下來換
- 手動換完記得 `xattr -dr com.apple.quarantine <app>`，必要時再 `codesign --force --deep --sign - <app>`
