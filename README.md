# 每日工作日誌

讀資料夾裡的 `<民國7碼>.md`，把每天做了什麼、每支工作項目走到哪一步，畫成桌面 app。

Rust（Tauri 2）＋ 靜態 HTML。日誌檔幾乎只讀不寫：唯一會動到 `.md` 的是 TODO 頁的「加到今日日誌」，
而且只往今天的檔案插一行。

## 跑起來

```sh
cd src-tauri
cargo run
```

不開視窗、只想看解析結果：

```sh
cargo run --example probe                 # 用設定裡的資料夾
cargo run --example probe -- ~/某個資料夾   # 指定資料夾
cargo test                                # 解析規則的測試
```

## 打包與安裝

```sh
cargo install tauri-cli --version "^2" --locked   # 只要裝一次
cd src-tauri
cargo tauri build
cp -R "target/release/bundle/macos/每日工作日誌.app" /Applications/
```

`.dmg` 在 `target/release/bundle/dmg/`。

App 沒有簽章也沒有公證，所以：

- 從這台機器複製過去可以直接開
- 如果是透過網路傳給別台機器，第一次要用右鍵「打開」，或 `xattr -dr com.apple.quarantine <app>`
- 第一次開會跳「想要取用你『文件』檔案夾中的檔案」，要按**允許**，否則讀不到日誌
  （按錯了到「系統設定 → 隱私權與安全性 → 檔案與檔案夾」再打開）

圖示原稿是 `icon.svg`，改完重新產：

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
TAURI_SIGNING_PRIVATE_KEY_PATH=~/.tauri/worklog.key \
TAURI_SIGNING_PRIVATE_KEY_PASSWORD= \
cargo tauri build
```

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
- `url` 直接複製 Release 頁面上那顆檔案的連結最保險（檔名有中文的話 GitHub 會自己 percent-encode）

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

## 更新提示

開 app 之後會在背景查一次有沒有新版（每次開 app 只查一次，換分頁不會重查）：

- 有新版才會在右上角浮一則「有新版 vX.Y.Z」，按「更新」就跳到設定頁的「版本與更新」
- 查不到、連不上、還沒有任何 Release 一律安靜略過，不會跳錯誤打斷你
- 想主動查就去設定頁按「檢查更新」；那裡也顯示目前版本、更新說明、下載進度

## 設定

第一次開會用預設路徑 `~/Documents/Obsidian Vault/每日工作日誌`。
要換路徑：左邊「設定」→「選資料夾…」→「儲存並重讀」。

設定、TODO、待寫回的變更都存在 `~/Library/Application Support/tw.npust.worklog-app/`，
不會寫進日誌資料夾。（例外只有日誌規則：那一區直接改 `~/.claude/CLAUDE.md`，改前會備份。）日誌資料夾裡唯一會被寫到的是「加到今日日誌」新增的那一行。

設定頁最下面是「版本與更新」：目前版本、檢查更新、下載安裝新版，說明看〈發新版〉。

設定頁下面還有一區「外部服務」，填 GitLab 與 Redmine 的位址與 token，
填了之後點日誌裡的 MR／議題連結就會直接在 app 裡顯示內容（見下一節）。
再下面是「日誌規則」，那一區編輯的是 `~/.claude/CLAUDE.md`，見〈日誌是誰寫的〉。

- 位址例如 `http://gitlab.example.com`、`https://redmine.example.com`；留空就照連結本身的主機走
- token 存在同一個 `settings.json`，**不會**寫進日誌資料夾，存好之後畫面也不會再顯示出來
  （只會說「token 已設定」），要換就重填，要拿掉按「清除 token」
- token 欄留空按儲存＝不更動原本那把，所以只改位址不會把 token 洗掉
- 如果設定的位址跟連結的主機不一樣，app 會直接停下來並說明原因，不會把 token 送到別台機器

## 日誌是誰寫的

日誌檔不是這個 app 產的，是 Claude Code 照使用者層級提示詞的規則寫的：
`~/.claude/CLAUDE.md` 裡的「# 每日工作日誌」那一段。

規則可以在**設定頁的「日誌規則」**直接編輯，改完按「套用到 Claude」就會寫回那個檔：

- 載進來的內容照這個順序找：`~/.claude/CLAUDE.md` 的「# 每日工作日誌」段 → 沒有就用內建預設值
- 內建預設值是 `docs/worklog-rules.md` 裡的規則本文，用 `include_str!` 在**編譯期**嵌進 binary，
  所以 release build 之後不需要 `docs/` 在執行檔旁邊；也因為這樣，**改完那份 md 要重新編譯**才會變成新的預設
- 寫入時**只取代「# 每日工作日誌」那一段**，檔案其他內容一個字都不動；
  沒有那一段就接在檔尾，檔案不存在就建一個
- **寫之前一定先備份**：同目錄複製一份 `CLAUDE.md.bak-<民國7碼>-<時分秒>`，備份路徑會顯示在畫面上
- 只有按下按鈕才會寫，app 不會自己去動你的 CLAUDE.md
- 「還原成預設」把內建預設值載回編輯框（還沒寫檔，要再按「套用到 Claude」）
- 「複製全部」複製到剪貼簿

這份規則是**寫給 Claude 看的**，app 不會照它改變解析行為：解析規則寫死在
`src-tauri/src/parser.rs`。`docs/worklog-rules.md`（人怎麼寫）與 `parser.rs`（程式怎麼讀）
是一對的，要改就兩邊一起改。

## 它讀得懂什麼

只讀資料夾第一層、檔名是七碼數字的 `.md`，例如 `1150817.md`（民國年月日）。

```markdown
## project_a

- `已歸檔` [feat: 對話頁補上左側清單](http://gitlab.example.com/group/project_a/-/merge_requests/64)
- `已歸檔` [chore: archive room-page-full-layout](http://gitlab.example.com/group/project_a/-/merge_requests/65)
- `暫存` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)
- `完成` 議題對帳：兩邊未結案數對齊
```

- `## 名字` 是專案分區
- 每行開頭的行內程式碼是狀態，只有這八個算數：待辦、提案中、暫存、實作中、測試中、待合併、已歸檔、完成
- 行首以外的反引號不算狀態，所以 ``（寫入 `~/.claude/CLAUDE.md`）`` 不會被誤判
- 連結後面的括號是補充說明，會被分開存
- 沒標狀態的行也會讀進來，只是不影響工作項目的狀態
- 讀不懂的行不會被吞掉，會列在設定頁的「沒讀懂的行」

## 工作項目是怎麼算出來的

日誌檔裡沒有「工作項目」這種東西，是從條目歸戶推出來的。歸戶規則照順序試：

1. 標題開頭是 `slug：描述` —— slug 就是工作項目代號
2. 標題是 `chore: archive <slug>` —— 封存 MR 的固定寫法
3. 條目的連結，在別的地方出現在某個 slug 名下（同一支 change 的議題連結會重複出現）
4. 下一行是同專案的 `chore: archive <slug>` —— 實作 MR 與封存 MR 成對出現，前一行跟著後一行歸戶
5. 前四條都對不上，但那一行**帶生命週期狀態**（待辦／提案中／暫存／實作中／測試中／待合併／已歸檔）
   —— 讓它自己成為一支工作項目，標題與專案就用那一行的

第 5 條的 id 是算出來的（`auto-<16 碼十六進位>`），算法固定，所以同一件事跨天寫還是同一支：
有連結就用連結當 key（先去掉結尾斜線、`?` 之後與 `#` 之後），沒連結就用「專案＋標題」當 key。
沒標狀態、以及標 `完成` 的行不走這條，所以一次性雜項不會灌爆看板；那些行只會出現在當天的日誌裡。

**目前狀態＝最後一筆帶生命週期狀態的條目**。`完成` 不算生命週期，所以一次性工作不會變成工作項目。
狀態不是點出來的，要推進就在當天的 md 加一行。

## 五個分頁

導覽是頂端一條工具列，分頁在正中間的分段控制上。

| 分頁 | 看什麼 |
| --- | --- |
| 月曆 | 首頁。整個月每天列出各狀態幾筆，點某天進看板 |
| 看板 | 七欄生命週期，卡片可拖曳改狀態；可只看某一天推進的項目 |
| 日誌 | 可選範圍：單日／7 天／30 天／全部／自訂起訖。← → 換日或整段移動 |
| TODO | 隨手記；一鍵「加到今日日誌」寫進今天的 md，或按「複製 md」貼給 Claude |
| 設定 | 日誌資料夾、外部服務、日誌規則、讀到幾個檔、沒讀懂的行 |

工作項目歷程在 `work-item.html`，從看板卡片標題進去。

## 搜尋

工具列左上角、「工作日誌」與麵包屑右邊有一個膠囊搜尋框，**只在月曆、看板、日誌這三頁出現**
（TODO 與設定沒有可過濾的清單）。打字就地過濾，不會重讀資料。

- 打完字等 **150ms** 才過濾，不是每個字都重畫
- **中文輸入法組字中不算數**：組字期間輸入框裡是注音符號，濾出來一定是空的，
  所以 `compositionstart` 到 `compositionend` 之間一律不觸發，選完字才過濾
  （判斷跟 TODO 頁的 Enter 是同一套：`isComposing`、`keyCode === 229`、`compositionend` 之後的時間差）
- **Esc** 清空並取消過濾，**⌘F**（Windows／Linux 是 Ctrl+F）聚焦到搜尋框
- 關鍵字寫進網址的 `?q=`，重新整理還在；切到別的分頁也會帶著走，回來就還是同一組過濾

比對規則三頁一致，寫在 `ui/js/store.js` 的 `matchesSearch()`：

- 一律**不分大小寫**
- 關鍵字用空白切成多個詞時，**每個詞都要命中**（AND）；全形空白也算分隔
- 空字串或只有空白＝不過濾

三頁各自濾什麼、怎麼呈現：

| 分頁 | 過濾什麼 | 比對範圍 |
| --- | --- | --- |
| 月曆 | 格子裡的狀態統計只算命中的條目 | 條目標題、專案、狀態中英文標籤、連結網址 |
| 看板 | 卡片。七欄一律留著，只是卡片變少、每欄的數字跟著變 | 工作項目標題、專案、狀態標籤、議題／MR 網址、項目 id |
| 日誌 | 條目。單日與跨日兩種模式都生效 | 同月曆 |

- 月曆完全沒命中的日子**不會消失**（格線結構要完整），而是整格淡掉、統計歸零；
  上方「N 天有紀錄 · 共 M 筆」也是過濾後的數字
- 日誌某個專案／某一天全部被濾掉就不留空的區塊；全部沒命中會寫「沒有符合的條目」
- 三頁在有關鍵字時都會出現一條提示，寫著「正在用『xxx』過濾，共 N 筆」，旁邊有「清除」，
  免得自己開著過濾卻忘記了

Shell 只管輸入框與關鍵字：關鍵字一變就丟一顆 `worklog:search` 自訂事件出去
（`event.detail` 是關鍵字），怎麼過濾、怎麼重畫是各頁自己的事；
頁面第一次畫的時候用 `Shell.searchQuery()` 拿目前的關鍵字。

日誌頁的範圍怎麼看：

- **單日**：依專案分區，點檔名用編輯器開原檔，← → 一天一天走
- **7 天／30 天／自訂**：一天一段（新的在上面），條目自己標專案；← → 整段前後移動
- **全部**：讀到的每一天
- 每段右邊有「只看這天」可以切回單日
- 目前範圍會寫進網址（`daily-log.html?d=1150817&range=week`），重新整理不會跳掉

## 點連結會怎樣

MR 與議題連結**不會**直接跳瀏覽器，而是在 app 裡開一個面板，顯示後端打 API 抓回來的內容：
標題、狀態、幾行 metadata（專案、作者／指派者、分支、最後更新…）、描述全文，最後是**留言**。
面板下方一定有原始連結與「在瀏覽器開啟」，要看完整頁面按那顆才會叫系統瀏覽器。

- Redmine 走 `GET {base}/issues/{id}.json?include=journals`，GitLab 走 `GET {base}/api/v4/projects/{專案路徑}/merge_requests/{iid}`（議題就是 `/issues/{iid}`）
- 沒設定 token、網址認不出來、401／403／404、連不上、逾時（10 秒），面板都會直接寫出是哪一種，
  而且照樣給「在瀏覽器開啟」，不會卡住
- 描述是 markdown，自己轉成 HTML（先逐字轉義再組標籤），沒有引任何 markdown 套件
- **整個面板只有一根捲軸**：描述區與留言區都不自己捲，內容多長就多長，滑鼠滾到哪裡都是在捲面板。
  只有程式碼區塊與表格留著自己的**橫向**捲動
- 描述下面是**留言**區，標題列寫則數，每則是「作者・時間」小字加內文，之間用一條髮絲線隔開，
  內文跟描述走同一套 markdown 轉換；沒有留言就寫「沒有留言」
- 留言的來源：Redmine 是 `journals` 裡有 `notes` 的那幾筆（**純欄位變更不顯示**），
  GitLab 是 `GET .../notes?sort=asc&per_page=100`（**`system: true` 的系統訊息濾掉**）
- GitLab 的留言是額外一次 API 呼叫，**抓失敗不會拖垮整個面板**：上面的內容照樣顯示，
  只有留言區寫「留言讀取失敗：<原因>」
- Esc 或點面板外面關掉
- 其餘外部連結維持原本行為：一律丟給系統瀏覽器，不會把 app 視窗導航掉

描述與留言裡的**圖片會真的顯示出來**，但不是讓 WebView 直接去載：

- Redmine／GitLab 的附圖多半要帶 token，而且 markdown 裡常寫成相對路徑
  （GitLab 常見 `/uploads/<hash>/name.png`、Redmine 常見 `/attachments/download/...`），
  直接 `<img src>` 多半 401 或空白
- 所以 markdown 轉換時**只放一個佔位**（原始網址記在 `data-img-src`），面板畫完之後才非同步呼叫後端
  `fetch_image`，後端用跟抓內容同一套設定（base 位址、token、10 秒逾時、同樣的主機比對）去抓，
  回傳 `data:` URI，前端才換成 `<img>`
- 網址補全：GitLab 的 `/uploads/...` 會補成 `{base}/{專案路徑}/uploads/...`；其他絕對路徑直接接在 base 後面；
  補不出來的相對路徑就當抓不到，**不猜**一個可能是錯的網址
- 只收 `image/png`、`jpeg`、`gif`、`webp`、`svg+xml`，而且**上限 10 MB**，其他一律拒絕
  （被導去登入頁的 `text/html` 就是這樣擋下來的）
- 圖片跟連結一樣不會送去別台主機：圖在別的 host 就直接不抓，token 不外流
- 一則描述／留言裡有好幾張圖就各抓各的，**失敗的那張不影響其他張**：那張退回原本
  「可點的連結」樣子，旁邊用灰字寫失敗原因
- 前端只接受後端回傳、而且真的是 `data:image/…;base64,` 開頭的字串，其他一律不塞進 `src`

## TODO 頁

輸入區在清單**下面**（標題列只留 TODO 與計數），專案預設是「其他」。每一筆有四顆按鈕：

| 按鈕 | 做什麼 |
| --- | --- |
| 編輯 | 那一列就地展開成輸入框，文字、專案、網址、狀態都能改 |
| 加到今日日誌 | 真的寫進今天的 `<民國7碼>.md`，寫完那筆標成完成（不刪掉） |
| 複製 md | 把那一行 markdown 複製到剪貼簿，方便貼給 Claude |
| 刪除 | 只刪 app 裡的 TODO，不動日誌檔 |

編輯是就地展開，不跳 modal，也不影響其他列：

- Enter 儲存、Esc 取消；取消就是不存，值原封不動回到原本的樣子
- 專案可以選既有的、清成「不標專案」，或用「＋ 新增專案…」開一個新的
- 存檔走 `Log.updateTodo(id, fields)`：改記憶體再整包丟給後端的 `save_todos`，跟新增同一條路
- 存完立刻重畫清單，新開的專案也會出現在下面輸入區的選單裡

打字按 Enter 就新增／儲存這個便利留著，但**中文輸入法組字中的 Enter 不算**：
`isComposing`、`keyCode === 229`、以及 `compositionend` 之後極短時間內的 Enter 都直接放過，
所以注音選完字按 Enter 只會確認選字，不會誤送出（WKWebView 會先送 `compositionend` 才送 keydown，
只靠 `isComposing` 擋不住，才需要那道時間差）。

「加到今日日誌」是這個 app 唯一會寫日誌檔的地方（後端 `append_entry`），寫得很保守：

- 檔案不存在就建一個，內容是 `## <專案>` 加那一行
- 檔案存在就找 `## <專案>` 區塊，接在該區塊**最後一個 bullet 後面**；找不到該專案就在檔尾開一個新區塊
- **只插一行**，既有的行一個字都不動，也不重排
- 一模一樣的行同一天只寫一次，重複按會提示「已經有了」，不會寫第二次
- 寫完自動重讀資料，日誌頁與月曆立刻看得到

那一行的寫法跟「複製 md」拿到的完全一樣：

```markdown
- `實作中` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)
```

## 拖卡片不會改到你的 md

看板上把卡片拖到別欄，app **不寫日誌檔**，只記一筆「待寫回」的變更：

- 存在 `pending.json`，重開 app 還在
- 這筆變更會併進當天的條目一起算，所以卡片立刻就在新欄位，日誌頁那行會標「未寫回」
- 上方會出現「未寫回 N」徽章
- 看板的提示條可以「複製全部」——複製出來的就是日誌檔那幾行，貼給 Claude 寫進當天的 md
- 寫進去之後按「丟掉」，待寫回就清空，資料回到單一來源：md

複製出來的行長這樣，跟你既有的寫法一致，所以貼回去之後還能歸到同一支工作項目：

```markdown
- `實作中` [search-message-history：對話紀錄搜尋](https://redmine.example.com/issues/32979)
```

## 結構

```
ui/                前端（靜態檔，沒有 build step）
  index.html       月曆首頁
  board.html       看板（拖曳改狀態）
  js/store.js      向後端要資料，所有頁面共用；頁面程式包在 Log.ready() 裡；搜尋的比對規則也在這
  js/shell.js      頂端工具列（含搜尋框）、連結面板、未寫回徽章、資料夾異常橫幅
  css/app.css      全站隱藏捲軸
  vendor/          Tailwind Play CDN 的本地副本
  fonts/           Inter，離線可用（其餘用系統字）
src-tauri/src/
  parser.rs        md 解析與工作項目歸戶（規則都有測試）
  model.rs         狀態表與資料結構
  store.rs         設定（含外部服務 token）、日誌規則（讀寫 ~/.claude/CLAUDE.md）、TODO、待寫回變更
  link.rs          連結解析與 Redmine／GitLab API、附圖代抓（解析與檢查規則有測試，不打網路）
  commands.rs      前端呼叫的指令（含 move_item / clear_pending / append_entry / fetch_link / fetch_image）
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
右邊是未寫回徽章與深淺色。搜尋框是膠囊、髮絲線邊、左邊一個放大鏡，有字才出現右邊的清除鈕；
寬度有上限，窄視窗自己縮，不會把麵包屑擠掉。
每一頁都鋪滿寬度（不收窄置中），切分頁時上方不會跳動。捲軸全站隱藏（`ui/css/app.css`），
所以可捲動的區域**盡量只留一層**：連結面板就只有主體那一根捲軸，標題列／內容區／底部連結列
左右內距一致（`px-6`），內容區底下多留一段空白，配上連結列的髮絲線，捲到底看得出來下面還有東西。

## 已知限制

- app 沒有簽章與公證，只適合自己用；線上更新也因此有可能被 macOS 擋掉，失敗時要自己去下載頁抓
- 拖曳只在桌機有效；手機版寬度請用卡片上的下拉選單改狀態
- 看板的「複製全部」用瀏覽器剪貼簿 API，沒有另外接系統剪貼簿
- 工作項目沒有任務數（N/M 項），因為日誌檔裡本來就沒寫
- 歸戶失敗的條目目前只能靠改日誌檔的寫法修正，app 裡不能手動指派
- 第 5 條歸出來的項目認的是連結，不認語意：同一支 change 的規格議題與實作 MR 如果沒有共同的
  slug 或連結，會被算成兩支。要合併就在標題前面補 `slug：`
