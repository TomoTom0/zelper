# TASK-24 外部慣行調査: 配布形式の標準・仕様系

- 調査日: 2026-08-24
- 調査方法: 一次情報源（公式サイト・公式docs）の直接fetch + 補足web検索
- 目的: coding agentがzelperを文法エラー・誤用なく使いこなせるための文書配布形態 (a) docs/配下agent向けガイド (b) Agent Skills/skill形式 (c) zelper docsサブコマンド (d) llms.txt (e) AGENTS.md の比較検討の基礎資料
- 備考: 本ファイルは旧作業dir（repo管理外）の調査資材をMR-37で移設したもの

---

## 1. AGENTS.md標準

### 正本URL

- サイト・仕様: https://agents.md
-（サイト内に独立したspec文書はなく、landing page自身が正本。stewardはAgentic AI Foundation / Linux Foundation）

### 形式・仕様の要点

- 位置づけ: 「README for agents」。coding agentにproject context・指示を与えるための専用の予測可能な置き場所。60k以上のOSS projectで使用とされる
- 形式: 通常のMarkdownのみ。必須field・schema・frontmatterは一切なし（FAQ: "Are there required fields? No. AGENTS.md is just standard Markdown. Use any headings you like; the agent simply parses the text you provide."）
- 記述慣習としてサイトが挙げるsection: project overview / build・test commands / code style guidelines / testing instructions / security considerations / PR・commit指針等（「新しいteammateに伝えることは何でも」）
- 配置規則:
  - 原則: repository rootに`AGENTS.md`
  - monorepo: 各package内にもnest可。agentは編集対象fileに最も近いfileを読む（nearest wins）。OpenAI本体repoで88個のAGENTS.mdが存在する例を紹介
- 競合解決: 編集fileに最も近いAGENTS.mdが優先。明示的なuserのchat指示はすべてに優先
- agentが自動実行するか: test/buildコマンドを記載すればagentは関連checkを実行し失敗を修正しようとする（FAQ: "Yes—if you list them."）
- 起源・管理: OpenAI Codex, Amp, Jules (Google), Cursor, Factoryの協同で成立。現在はAgentic AI Foundation（Linux Foundation配下）がsteward

### 公式対応tool（agents.mdサイト掲載、2026-08時点）

Kilo Code / goose / opencode / Ona / Autopilot & Coded Agents (UiPath) / Gemini CLI (Google) / Jules (Google) / Aider / Amp / Cursor / RooCode / Zed / Semgrep / Augment Code / Windsurf (Cognition) / Codex (OpenAI) / GitHub Copilot coding agent / Devin (Cognition) / Junie (JetBrains) / Warp / Phoenix / Factory / VS Code

tool個別の読み込み設定例（サイトFAQより）:

- Aider: `.aider.conf.yml`に`read: AGENTS.md`
- Gemini CLI: `.gemini/settings.json`に`{ "context": { "fileName": "AGENTS.md" } }`

### Claude Codeの対応状況（重要）

- **Claude CodeはAGENTS.mdを公式サポートしない**。公式docs（ https://code.claude.com/docs/en/memory ）に明記: "Claude Code reads `CLAUDE.md`, not `AGENTS.md`."
- 代理手段（同docs公式）:
  - `CLAUDE.md`内に`@AGENTS.md` importを書く（両toolで同一指示を重複なく読ませる)
  - symlink: `ln -s AGENTS.md CLAUDE.md`
  - `/init`（`CLAUDE_CODE_NEW_INIT=1`）がAGENTS.mdを読み込んでCLAUDE.md生成に反映
  - `/import`（Claude Code v2.1.213+）でAGENTS.md等の他agent設定をCLAUDE.mdへ一回コピー
- CLAUDE.md側の配置・読み込み規則（zelper比較の参考）: `./CLAUDE.md` or `./.claude/CLAUDE.md`（project）、`~/.claude/CLAUDE.md`（user）、`CLAUDE.local.md`（local）、managed policy（`/etc/claude-code/CLAUDE.md`等）。cwdから上位dir全部を連結ロード、subdirは対象file読み取り時にon-demand

### zelper適用時に必要な具体的要件

- zelper repo自体への配置: repo rootの`AGENTS.md`1file。必須項目なし・Markdown自由形式
- 注意: AGENTS.mdは「そのrepoで作業するagentへの指示」の規格であり、zelperというtoolの使い方をend userのagentに配布する規格ではない。zelper利用ガイドを配布する場合は「user各自のrepoのAGENTS.mdに追記してもらうsnippet」という形になる。Claude CodeのみはAGENTS.mdを読まないため別経路（CLAUDE.md import等）が必要

---

## 2. Agent Skills標準（agentskills.io）

### 正本URL

- Overview: https://agentskills.io
- 仕様書: https://agentskills.io/specification
- 開発: Anthropic発、open standard（GitHub: https://github.com/agentskills/agentskills 、Discordあり）

### 形式・仕様の要点

- 位置づけ: AI agentに専門知識・workflowを与える軽量なopen format。「build once, use across any skills-compatible agent」
- 構成単位: skill = `SKILL.md`を含むdirectory

```
my-skill/
├── SKILL.md          # 必須: metadata + instructions
├── scripts/          # 任意: 実行code
├── references/       # 任意: 参照doc
├── assets/           # 任意: template等
└── ...               # その他自由
```

- SKILL.md = YAML frontmatter + Markdown body

frontmatter field（仕様書より）:

| field | 必須 | 制約 |
|---|---|---|
| `name` | 必須 | 1-64文字。unicode小文字英数字`a-z0-9`と`-`のみ。先頭/末尾`-`禁止、連続`--`禁止。**親directory名と一致必須** |
| `description` | 必須 | 1-1024文字。「何をするか」と「いつ使うか」の両方を記述し、agentが関連taskを識別するkeywordを含める |
| `license` | 任意 | license名またはbundled license file名。短く |
| `compatibility` | 任意 | 1-500文字。環境要件（対象product・必要system package・network等）がある場合のみ |
| `metadata` | 任意 | string key -> string valueのmap。拡張property用。key名は衝突回避で一意に |
| `allowed-tools` | 任意 | 事前承認されるtoolのspace区切りlist。**experimental**。実装間でsupport差あり |

- body: 形式制限なし。推奨section: step-by-step instructions / 入出力例 / 一般的edge case
- progressive disclosure（3段階。skill設計の核）:
  1. **Metadata**（約100 token）: 起動時に全skillの`name`+`description`のみ読み込み
  2. **Instructions**（推奨5,000 token未満）: 起動判定後にSKILL.md全文を読み込み
  3. **Resources**（必要時）: scripts/references/assets内のfileを必要になった時点で読み込み
- size指針: SKILL.mdは500行未満推奨。詳細は別fileへ分離
- file参照: skill rootからの相対path。参照は1階層まで（深いnest参照を避ける）
- 検証tool: `skills-ref` reference libraryでfrontmatter・命名規則をvalidate可能

### 採用tool（agentskills.io client showcase掲載、2026-08時点）

主要どころのみ: Claude Code / Claude (claude.ai) / ChatGPT & Codex (OpenAI) / GitHub Copilot / VS Code / Cursor / Gemini CLI / Amp / OpenCode / OpenHands / Goose / Junie (JetBrains) / Roo Code / Kiro / Windsurfは未掲載（Warp, Devinも未掲載） / その他多数（Tabnine, TRAE, Factory, Pulumi Neo等、合計40+）

→ AGENTS.md対応listとSkills対応listはほぼ逆補集合的なtoolもある（例: Devin/WarpはAGENTS.mdのみ、GitHub Copilot/VS Code/Codex/Gemini CLI/Cursorは両対応）。両方出せばほぼ主要tool全域をカバーできる

### zelper適用時に必要な具体的要件

- 最小構成: `zelper/SKILL.md`（directory名と`name: zelper`を一致させる）
- 必須frontmatter: `name`（小文字・hyphen規則準拠）+ `description`（1-1024文字。「zellij session/pane操作CLIの使い方」と「いつ使うか（多pane・coding agent管理をしたい時）」+ トリガーkeyword: zellij, tmux的, pane, layout, rename, resize, send, remap等）
- 詳細文法・JSON output仕様は`references/`へ分離（progressive disclosureに沿える。SKILL.mdは500行・5k token以内）
- tool間portabilityを最大化するならfrontmatterはspec 6 fieldのみに限定（次節のClaude Code拡張fieldを使うと他toolでerrorになる場合がある）

---

## 3. Claude Code skills docs（code.claude.com/docs）

### 正本URL

- Skills: https://code.claude.com/docs/en/skills （markdown版: https://code.claude.com/docs/en/skills.md ）
- Memory（AGENTS.md対応状況の根拠）: https://code.claude.com/docs/en/memory
- 前提: 「Claude Code skills follow the Agent Skills open standard」+ Claude Code独自拡張（invocation control / subagent実行 / dynamic context injection）

### 配置場所（「Where skills live」表より）

| Location | Path | 適用範囲 |
|---|---|---|
| Enterprise | managed settings内`.claude/skills/`（例: Linux `/etc/claude-code/.claude/skills/<name>/`） | 組織全user |
| Personal | `~/.claude/skills/<skill-name>/SKILL.md` | 自分の全project |
| Project | `.claude/skills/<skill-name>/SKILL.md` | そのprojectのみ |
| Plugin | `<plugin>/skills/<skill-name>/SKILL.md` | plugin有効環境 |

- 同名衝突の解決: enterprise > personal > project。project skillはbundled skill（例: `/code-review`）を同名で上書き。plugin skillは`plugin-name:skill-name`namespaceで衝突しない
- 起動dirからrepo rootまでの各`.claude/skills/`を起動時にload。subdirの`.claude/skills/`はそのsubdirのfileをClaudeが読み書きした時にload（directory修飾名`apps/web:deploy`で共存）
- custom command（`.claude/commands/*.md`）はskillsに統合済み。同名ならskill優先
- live change detection: session中のskill追加・編集・削除を即時反映（restart不要）

### 発動条件・descriptionの書き方

- skill一覧（name+description）は常時contextに読み込まれ、Claudeはdescriptionを見て自動でskillを選択。`/skill-name`での直接invokeも可能
- descriptionの書き方（docs・troubleshootingより）:
  - 「What the skill does and when to use it」を書く。userが自然に口にするkeywordを含める
  - **key use caseを先頭に置く**: `description`+`when_to_use`の合計はlistingで1,536文字で切られる
  - skill listing全体の予算はcontext windowの1%。あふれると使用頻度の低いskillのdescriptionから削られる
  - 発動しない時: descriptionにkeywordがない / 発動しすぎる時: descriptionをよりspecificに
- frontmatter YAMLが不正でもbodyは読まれるがdescriptionが空になり自動発動しない（`--debug`でparse error確認）

### frontmatter（Claude Code拡張込み全field）

`name`, `description`（推奨）, `when_to_use`, `argument-hint`, `arguments`, `disable-model-invocation`, `user-invocable`, `allowed-tools`, `disallowed-tools`, `model`, `effort`, `context`（fork）, `agent`, `background`, `hooks`, `paths`（globで発動file制限）, `shell`, `metadata`, `license`, `compatibility`

- 全field任意、推奨は`description`のみ
- 重要な拡張:
  - `disable-model-invocation: true`: 自動発動禁止（user手動invoke限定）
  - `user-invocable: false`: userの`/`invoke禁止（Claude自動発動のみ。背景知識type向け）
  - `allowed-tools`: skillをinvokeしたturnのみtoolを事前承認（例: `Bash(git add *)`）。次のuser messageで失効
  - `context: fork` + `agent: Explore`等: skillをsubagent実行
- **portabilityの要**: claude.ai upload / Skills API / `package_skill.py`経由ではspec 6 field（`name`, `description`, `license`, `compatibility`, `metadata`, `allowed-tools`）以外はhard error（"Unexpected key(s) in SKILL.md frontmatter"）。Claude Code本地では全field OK。6 fieldに絞ればClaude Codeでもそのまま読める

### その他の仕様（zelper skill設計に直接関係するもの）

- `SKILL.md`は500行未満推奨（Tip明記）。supporting files（`reference.md`, `examples.md`, `scripts/`）をSKILL.mdから参照させ、必要時に読ませる
- skill contentはinvoke後session終了までcontextに残る。「毎行がrecurring token cost」のため簡潔に
- `${CLAUDE_SKILL_DIR}`: skill directoryの絶対path。body内script実行に使える
- dynamic context injection: `` !`command` `` でskill読み込み時にshell commandを実行し出力を埋め込み（例: `` !`zelper list --json` `` のような用法も技術的に可能）
- 配布: project skillは`.claude/skills/`をversion管理にcommit。plugin化・managed配布も可

### zelper適用時に必要な具体的要件

- 配布先として現実的な2経路: (1) zelper repoに`.claude/skills/zelper/SKILL.md`をcommitしてproject skill配布（zelper開発自身のため）(2) userが`~/.claude/skills/zelper/`へ置くpersonal skill（zelperというtoolの利用guideとして）。公式配布形態としてはinstall手順 or plugin marketplace
- description設計が発動品質の核心: 「zelper/zellij/多pane/coding agent管理」等のtrigger語と用途を1,536文字以内で先頭に
- 他tool互換を取るならfrontmatterは`name`+`description`（+必要なら`allowed-tools`）のみ

---

## 4. llms.txt（llmstxt.org）

### 正本URL

- 仕様: https://llmstxt.org （v2。author: Jeremy Howard / Answer.AI。published 2024-09-03、modified 2026-08-10）
- GitHub: 本体pageからlink（issue管理）

### 形式・仕様の要点

- 目的: websiteがLLM-friendlyな内容を提供するためのmarkdown file。agentがweb pageから情報を得る際、HTMLのnav/広告/JSを除去する損失を避け、簡潔で専門的な情報を1箇所に集める。**trainingよりinference（agentのon-demand利用）向け**
- 配置: site rootの`/llms.txt`、または任意subpath（例: `/docs/llms.txt`）。fileはそのpath配下のURLをcoverし、複数あれば最もspecificなものを使う（`index.html`や`robots.txt`と同じpath慣習。RFC 8615 `.well-known/`はshared hostで使えないため不採用）
- 形式（この順序で、以下のsection構成）:
  1. 任意のBOM
  2. **H1: project/site名（唯一の必須section）**
  3. blockquote: projectの短い要約（file理解に必要なkey情報）
  4. 0個以上の非heading section（paragraph/list等の追加情報）
  5. 0個以上のH2区切りの「file list」: markdown listで必須のhyperlink `[name](url)` + 任意で`:`とnotes
- `## Optional` sectionは慣習として「contextを短くしたい時にagentがskipしてよい」secondary情報に使う
- v2の追加提案: (1) 情報pageと同一URLにmarkdown版を置く（`page.html.md` or `page.md`）(2) 発見用link relation `rel="alternate" type="text/markdown"`（pageのmarkdown版へ）と`rel="describedby"`（llms.txtへ）をHTML `<link>`またはHTTP `Link:` headerで
- llms.txt内のlinkはmarkdown版などLLM-friendlyなcontentを指すべき。file自体はcontextに収まる規模に保ち、詳細はlink先に置く（＝progressive disclosureと同型の設計）
- sitemap.xml・robots.txtとの差: sitemapは全page列挙でcontextに収まらずLLM版URLも含まない。robots.txtはcrawler許可、llms.txtはon-demand情報提供

### 採用実例・勢い（v2本文より）

- 数千siteがpublish。documentation platform（Mintlify, GitBook, Yoast, AIOSEO, Wix）が自動生成。Chrome Lighthouseがagentic browsing checkとしてaudit
- OpenAI・Anthropic・Geminiが自身のdeveloper docsでllms.txtをpublish
- 本調査でも実在確認: `https://agentskills.io/llms.txt`（docs index）・`https://code.claude.com/docs/llms.txt`（同）。**agent tooling系docsの標準慣行になりつつある**
- directory: llmstxt.site / directory.llmstxt.cloud / llmstxthub.com

### 批判・限界（web検索に基づく。要再調査の粒度）

- 主要検索engine・LLM providerがllms.txtを読むと公式表明したものはない（採用は出し手側に偏る chicken-and-egg問題）
- 協調的前提の脆弱性: 無視・改変されうる（Patrick Stox等の批判）
- 「実質効果なし（dud）」論: Medium（Kais Priestersbach "The llms.txt is dead"）
- 効果検証: SE Rankingが300k domain調査でAI引用への効果を実証できず
- spam/bias vectorになる懸念、hype vs reality（Mintlify自身も否定的観測を紹介）
- 位置づけのまとめ: 正式standardではなくproposal。ただしcoding agentがdocs siteを見に行く用途（zelperの文脈）では、docsをagent利用する側の慣行として成立しつつある

### zelper適用時に必要な具体的要件

- 前提: llms.txtは「websiteのpath」の慣習。GitHub repoのみでは配布できない（GitHub Pages等のdocs siteがあれば`/llms.txt`として配置）
- 必須要件: H1（=zelper）+ blockquote要約 + H2 section（Docs / CLI reference / JSON output / Examples等）にmarkdown link
- zelperにdocs siteが無い場合、この形式は直接使えず、代替としてrepo内のmarkdown（README・docs/）をllms.txtと同じ構造原則（H1要約 -> link list -> 詳細はlink先）で書くことが流用可能

---

## 5. 比較検討のための横断まとめ

| 観点 | AGENTS.md | Agent Skills | Claude Code skills | llms.txt |
|---|---|---|---|---|
| 正本 | agents.md | agentskills.io/specification | code.claude.com/docs/en/skills | llmstxt.org |
| 標準化度 | Linux Foundation管理・de facto（必須fieldなし） | Anthropic発open standard（6 field厳密仕様） | Agent Skills準拠+独自拡張 | 個人proposal（v2） |
| 対象 | repoで作業するagentへの指示 | 複数agent productで使えるskill配布 | Claude Code利用時のskill | website訪問agent |
| 必須要素 | なし（Markdown自由） | SKILL.md + name + description | 同左（descriptionのみ推奨） | H1のみ必須 |
| zelperでの役割 | 利用guide snippetの配布先（user各自のrepo） | 主候補: 利用guideの正式な配布形式 | zelper skillの動作検証環境・最大の利用者層 | docs siteがあれば |

### 調査から見える構図（zelper要件書との対応）

1. **zelperの「coding agentに使われるtool」という性質にはAgent Skills形式が最も近い**: stable JSON outputを含む利用法をprogressive disclosure（description常駐 -> SKILL.md -> references詳細）で配布でき、Claude Code / Codex / Copilot / Gemini CLI / Cursor等主要toolが対応済み
2. **AGENTS.mdとSkillsは補完関係**: AGENTS.mdはuserのrepo側に置く指示、Skillsは配布可能なpackage。Claude CodeだけはAGENTS.md非対応のため、AGENTS.md snippetを配布する場合はCLAUDE.md import手順も併記が必要
3. **llms.txtはdocs siteの慣行**: agent tooling系（agentskills.io, Claude Code docs自身）が採用済み。zelperがweb docsを持つか否かが適用可否の分岐
4. **docsサブコマンド（候補c）に該当する外部標準は本調査の4観点には存在しない**: 端末内出力は標準化対象外。ただしllms.txtと同じ構造原則（概要 -> 一覧 -> 詳細）を端末内出力に適用する設計は無矛盾（要再調査: 他CLIのdocsサブコマンド実例は今回の調査観点外）

## 出典一覧

- AGENTS.md: https://agents.md
- Claude Code memory docs（AGENTS.md非対応の明記）: https://code.claude.com/docs/en/memory
- Agent Skills overview: https://agentskills.io
- Agent Skills specification: https://agentskills.io/specification
- Claude Code skills docs: https://code.claude.com/docs/en/skills
- llms.txt v2: https://llmstxt.org
- llms.txt批判系（検索経由）: https://llmtxt.info/benefits-limitations/ / https://medium.com/@kaispriestersbach/the-llms-txt-is-dead-more-precisely-a-dud-ab7bee4f469c / https://seranking.com/blog/llms-txt/ / https://www.mintlify.com/blog/the-value-of-llms-txt-hype-or-real
- Claude Code AGENTS.md対応要望issue: https://github.com/anthropics/claude-code/issues/6235
