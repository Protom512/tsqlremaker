# Design: Document Links (`:r` パス解決) — #119

> CTO 事前条件 (レトロスペクティブ P5「後で決める禁止」)。この design.md は
> **実装開始前** に `:r` パス解決セマンティクスを完全に事前指定する。実装者
> (Task 2 以降) は本書の決定を実装するのみで、設計判断を先送りしてはならない。

## 0. 概要

LSP `textDocument/documentLink` + `documentLink/resolve` を実装し、SQLCMD
`:r <path>` ディレクティブのパス部分をクリック可能なリンクにする (`#119`)。
ASE 開発者は `isql`/sqlcmd バッチを複数ファイルに分割して運用するため、`:r`
で取り込むファイル群をエディタ上で直接開けることが価値を持つ。

### 0.1 前提 (Reality-Check 済み・実証番号付き)

| # | 事実 | 実証手段 |
|---|------|---------|
| F-1 | Parser は `:r` を認識しない。`DocumentAnalysis::statements` には `:r` 行は AST ノードとして現れない | `crates/tsql-parser` に SQLCMD ディレクティブ構文なし (AST 19 variants 参照) |
| F-2 | Lexer は `:r` を `Colon` + `Ident("r")` の **2 トークン** に分割する。単一キーワードではない | empirical probe: `Lexer::new(":r a.sql")` → `Colon(0-1)` `Ident("r")(1-2)` |
| F-3 | Lexer には **行頭認識がない**。行頭の `:r` と行中の `foo :r 'x.sql'` はトークン列として区別不可能 (改行トークンなし) | empirical probe: `"foo :r 'bar.sql'"` と `":r 'bar.sql'"` のトークン形状同一 |
| F-4 | クォート付きパスは `String` トークン1個になる (単一引用符)。二重引用符は `QuotedIdent` になる | empirical probe: `:r 'a/b.sql'` → `String(3-13)`、`:r "a/b.sql"` → `QuotedIdent` |
| F-5 | クォート **なし** パスは `Ident/Slash/Dot/DotDot` に断片化し、Windows バックスラッシュ `\` は `Unknown` トークン (空テキスト・長さ0 span) を生成して**壊れる** | empirical probe: `:r a\b.sql` → `Ident("a")` `Unknown("")` `Ident("b")` ... |
| F-6 | `Url::join` はファイル URL 上で呼ぶと「最終パスセグメント(ファイル名)を置換する」RFC 3986 相対参照解決を行う。すなわち doc URL にそのまま join すれば **文書ディレクトリ相対** になる | empirical probe: `file:///proj/scripts/main.sql`.join(`"init.sql"`) → `file:///proj/scripts/init.sql` |
| F-7 | `Url::join` は `..` を **クランプしない**。`"../../../../etc/passwd"` → `file:///etc/passwd` とワークスペース外へ逸脱する | empirical probe |
| F-8 | `Url::join` はバックスラッシュ `\` を暗黙に `/` に正規化する現バージョンだが、これは**非保証**(url crate の実装詳細)。明示正規化が堅牢 | empirical probe: `"sub\\init.sql"` → `.../sub/init.sql` (正規化される) |
| F-9 | doc URL からディレクトリ URL を得る正しい方法は `doc.join(".")` (trailing slash 付き)。`path_segments_mut().pop()` は trailing slash を落とし join 結果が変わるので**使ってはならない** | empirical probe: `join(".")` → `file:///proj/scripts/` (○)、`pop()` → `file:///proj/scripts` (×、`init.sql` join で `/proj/init.sql` になる) |
| F-10 | `lsp-types 0.94` の `DocumentLink` は `range: Range` / `target: Option<Url>` / `tooltip: Option<String>` / `data: Option<Value>` を持つ。`DocumentLinkOptions { resolve_provider: Option<bool>, work_done_progress_options }` | `lsp-types-0.94.1/src/document_link.rs` 直接確認 |

> F-2/F-3/F-5/F-6/F-7/F-8/F-9 はすべて破棄可能な `examples/*_probe.rs` で実証
> 済み (コミットせず破棄)。レトロスペクティブ P-004「パーサー/ランタイム挙動に
> 依存する見積もりは empirical probe で前提を実証」に準拠。

---

## 1. LSP メソッド

| メソッド | 目的 |
|---------|------|
| `textDocument/documentLink` | `:r` 行のパス範囲に未解決リンク (Range + `data`) を返す |
| `documentLink/resolve` | `data` から対象 URI を復元し `target` を埋める |

---

## 2. 解決する設計判断 (先送り禁止項目)

### 2.1 `(a)` ベースパス解決 = **文書相対** (document-relative) 既定 + **ワークスペース相対** (workspace-relative) フォールバック

`:r path` の `path` をどの基準で解決するか:

```
path が相対 (先頭が `/`・ドライブレター(`C:`)・`%VAR%` でない):
  1. 文書相対: base = 文書URI のディレクトリ  (F-6/F-9 参照)
     → doc.join(".").join(path)  # ただし後述の正規化・クランプ付き
  2. フォールバック: 文書相対で解決した URL が指すファイルが
     DocumentStore に存在しない場合、ワークスペース相対で再試行:
     → workspace_root_uri.join(path)
path が絶対 (`/`・ドライブレター・`%VAR%` 先頭):
  → そのまま Url::parse または doc.join(path) (F-6 で絶対パスも処理可)
```

**根拠**: 実際の sqlcmd/ASE 開発では `:r` は「現在のスクリプトファイルからの
相対パス」が既定 (sqlcmd の `-i` ファイルからの相対)。しかしプロジェクト全体を
ワークスペースとして開いている場合は「ワークスペースルート相対」の方が便利な
ケースがあるため、文書相対で解決できなかった場合のフォールバックとして扱う。
**両方のリンクを同時に出すのではなく、文書相対優先・失敗時フォールバックの
単一リンク** とする (二重リンクはクライアント表示が混乱する)。

**非スコープ**: ワークスペースルート URI の取得ロジック本体 (server.rs の
`workspace_folders` / 初期化時 root_uri) は Task 3 で配線する。本 design では
`workspace_root: Option<&Url>` を純粋関数に渡す契約のみ定義 (§4.3)。

### 2.2 `(b)` Windows バックスラッシュ正規化

`:r` のパス文字列を `Url::join` に渡す**直前**に `\` を `/` に置換する:

```rust
let normalized: String = raw_path.replace('\\', "/");
```

**根拠 (F-8)**: 現 url crate は `\` を暗黙正規化するが、これは RFC 3986 で
要求されておらず版によって挙動が変わりうる。ASE on Windows の `:r` では
バックスラッシュパスが頻出するため、**明示正規化のみを信頼** する。
正規化は **クォート有無に関わらず** 行う (クォートなしの `\` は F-5 で既に
トークン断片化で壊れているため、クォート付き String からのみ有効に働く — §3
参照)。

### 2.3 `(c)` トークンスキャン戦略 = **Colon + Ident(r|R) + String**

F-1/F-2 により AST 走査は機能しない (`:r` は AST に現れない)。従って
**`DocumentAnalysis::tokens` を直接走査** して `:r` ディレクティブを検出する。

**検出パターン (3 トークン連続)**:

```
トークン[i]   = TokenKind::Colon
トークン[i+1] = TokenKind::Ident かつ text が "r" または "R" (大文字小文字無視)
トークン[i+2] = TokenKind::String        ← パス (単一引用符。これを正式対応)
                 OR TokenKind::QuotedIdent ← パス (二重引用符。best-effort 対応)
```

**パス文字列の取得**:
- `String` トークン: テキストは `'scripts/init.sql'` のように**引用符を含む**。
  先頭末尾の `'` を剥がして中身を取り出す (§4.4 `strip_quotes`)。
- `QuotedIdent` トークン: テキストは `"scripts/init.sql"`。同様に `"` を剥がす。
  ※ sqlcmd の正規形は単一引用符だが、二重引用符も ASE で使われることがある
  ため best-effort で対応する。

**クォートなしパスは非対応 (非スコープ)**: F-5 で示した通り、クォートなし
パスは `Ident/Slash/Dot/DotDot` に断片化し、特に Windows バックスラッシュで
`Unknown` トークンが混入して**正確なパス再構築が不可能**。クォートなし `:r`
はリンクを生成せず黙塞ぎする (graceful skip)。これは文書化された制約であり
実装者の判断に委ねない。理由: クォートなしパスのトークン再結合は、コメント
や演算子混入時の曖昧性があり、堅牢な実装が不可能なため。

**broken-span ガード (CTO 条件)**: inlay_hints.rs:105 と対称に、**Colon 位置
と String/QuotedIdent span の両方** に `span.start < span.end` ガードを適用する:

```rust
// Colon span (通常 start=0,end=1 で健全だがマルチライン broken-span で壊れうる)
if colon.span.start >= colon.span.end { continue; }
// パストークン span
if path_token.span.start >= path_token.span.end { continue; }
```

### 2.4 `(d)` 相対パス `..` オーバーフロー処理 = **クランプ** (workspace 境界で打切り)

F-7 により `Url::join` は `..` をクランプしない (`/etc/passwd` まで逸脱)。
設計決定:

```
文書相対解決の結果 URL がワークスペースルート URL の配下に無い場合
(= `..` でワークスペース外へ逸脱):
  → そのリンクは **生成しない** (drop)。target に安全でない外部パスを
     指すリンクを誤って開かせるリスクを避ける。
```

**クランプでなく drop を選ぶ理由**: `..` クランプで「意図しない別ファイル」
を提示するより、リンクなし(クリック不可)の方が誤爆が少ない。sqlcmd の `:r` で
ワークスペース外参照は通常エラー(取り込み失敗)になるため、リンク不可が
セマンティクスにも合致する。

**判定方法**: `resolved_url.as_str().starts_with(workspace_root.as_str())`
または `resolved_url.path().starts_with(workspace_root.path())`。`workspace_root`
が `None` (単一ファイル オープン等) の場合は `..` クランプ判定をスキップし、
文書相対の結果をそのまま採用 (クライアンド側で開けなければ開かない)。

### 2.5 `(e)` `resolve_provider = Some(true)` + `link.data` による URI 復元

`DocumentLink::target` の計算には **文書 URI** と **ワークスペースルート URI**
が必要だが、`documentLink/resolve` リクエストは `textDocument` を持たない
(code_lens.rs と同じ制約)。従って:

- `documentLink` フェーズ: Range + `data` のみ返す (`target = None`・未解決)
- `documentLink/resolve` フェーズ: `data` から文書 URI と生パスを復元し、
  §2.1–2.4 の解決ロジックで `target` を埋める

**`resolve_provider = Some(true)` とする** (inlay_hints MVP の `false` とは
意図的に相違)。理由: target URI 計算は文書 URI + ワークスペースルートを要し、
これらは resolve 時にしか server から取り出せないため、遅延解決が本質的。
また code_lens の二段階パターン (PR #174 で実証済み) と同じ `data` 復元方式を
踏襲し、純粋関数の単体テスト可能性を保つ。

---

## 3. 行頭制約 — 唯一の未解決設計問に対する決定 (CTO 指摘事項)

> CTO feedback: 「lexer に行頭認識がないため、行頭でない stray `:r` と
> 真の `:r` ディレクティブを区別できない。design.md で位置を制約するか、
> 任意の Colon+Ident(r/R)+String を `:r` とみなすかを決定せよ。」

**決定: 行頭制約を採用する** (sqlcmd セマンティクスに合致)。

`:r` は SQLCMD ディレクティブであり、sqlcmd では**行頭** (先頭の空白/タブを
許容) に無いとディレクティブとして扱われない。F-3 によりトークン形状では
区別できないため、**`LineIndex` で Colon トークンが行頭にあるか判定**する。

**行頭判定アルゴリズム**:

```rust
fn is_at_line_start(analysis: &DocumentAnalysis, colon_span: Span) -> bool {
    let (line, _) = analysis.line_index.offset_to_position(colon_span.start);
    let line_start_offset = analysis.line_index.line_offset(line as usize) as u32;
    // colon_span.start から行頭へ遡り、間が空白/タブのみであることを確認
    let src_bytes = analysis.source.as_bytes();
    let mut o = line_start_offset as usize;
    while o < colon_span.start as usize {
        match src_bytes.get(o) {
            Some(b' ') | Some(b'\t') => o += 1,
            _ => return false, // 空白以外が先行 → 行頭ではない
        }
    }
    true // 行頭(先行は空白/タブのみ)
}
```

**根拠**: 行頭制約の方が正確性が高い (CTO も「line-start constraint for
correctness を支持」と明記)。実装コストは `LineIndex` が既存のため低い。
副次効果: 式中の `a:b` や誤入力 `x :r 'y.sql'` を false positive として
拾わない。

**トレードオフ**: CRLF (`\r\n`) は `line_index.rs` が行境界を吸収済み
(`get_line` の CRLF テスト参照)。BOM 先頭行は非対応 (非スコープ)。

---

## 4. データ構造と関数シグネチャ (dangling 参照なし・全フィールド列挙)

### 4.1 既存型 (参照元・変更なし)

```rust
// crates/ase-ls-core/src/analysis.rs (既存・公開)
pub struct DocumentAnalysis {
    pub source: String,
    pub line_index: LineIndex,
    pub tokens: Vec<OwnedToken>,
    pub statements: Vec<tsql_parser::ast::Statement>,
    pub parse_errors: Vec<tsql_parser::ParseError>,
    pub symbol_table: SymbolTable,
}
pub struct OwnedToken {
    pub kind: TokenKind,        // tsql_token::TokenKind
    pub text: Arc<str>,         // クォート含む生テキスト
    pub span: Span,             // tsql_token::Span { start: u32, end: u32 } (byte offset)
}

// crates/ase-ls-core/src/line_index.rs (既存・公開メソッドのみ使用)
impl LineIndex {
    pub fn offset_to_position(&self, offset: u32) -> (u32, u32);  // (line, char)
    pub fn line_offset(&self, line: usize) -> usize;               // 行頭 byte offset
    pub fn line_count(&self) -> usize;
}

// crates/ase-ls-core/src/config.rs (Task 5 で追加・本 design が契約を規定)
// DocumentLinkConfig の形状は §4.5 で完全指定。
```

### 4.2 新規: `DocumentLinkData` (link.data に stash するペイロード)

`documentLink/resolve` で対象 URI を復元するため、未解決リンクの `data` に
以下を stash する (code_lens.rs `LensData`・inlay_hints.rs `InlayHintData` と
対称の serde ペイロード)。

```rust
// crates/ase-ls-core/src/document_links.rs (新規)
use serde::{Deserialize, Serialize};

/// `DocumentLink::data` に格納し、`documentLink/resolve` で対象 URI を
/// 復元するためのペイロード (#119)。resolve リクエストは textDocument を
/// 持たないため、文書 URI と生パスをここに埋め込む (code_lens.rs LensData
/// と同じ理由付け)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentLinkData {
    /// リンク元文書の URI (resolve 時に analysis を再取得するため)。
    uri: String,
    /// `:r` に続くパス文字列 (クォート剥離済み・バックスラッシュ未正規化)。
    /// resolve 時に §2.2 の正規化 + §2.1 の解決を適用する。
    path: String,
}
```

**フィールド完全列挙**: `uri: String`, `path: String` のみ。追加フィールドなし。

### 4.3 新規: 純粋関数シグネチャ (単体テスト可能・server 非依存)

```rust
// crates/ase-ls-core/src/document_links.rs (新規)
use crate::analysis::DocumentAnalysis;
use lsp_types::{DocumentLink, Range, Url};

/// `:r` ディレクティブのパス範囲に未解決 DocumentLink を生成する (#119)。
///
/// トークンストリームを走査し、§2.3 の 3 トークンパターン
/// (Colon + Ident(r/R) + String/QuotedIdent) で `:r` を検出。
/// §3 の行頭制約と §2.3 の broken-span ガードを適用。
/// `target = None` (未解決)・`data` に DocumentLinkData を stash。
/// パス解決(§2.1/2.2/2.4) は resolve 時まで遅延する。
///
/// # Panics
/// Never. 不正/クォートなしパス・broken span は黙塞ぎスキップ。
#[must_use]
pub fn document_links(
    analysis: &DocumentAnalysis,
    uri: &Url,
    config: &DocumentLinkConfig,
) -> Vec<DocumentLink>;

/// DocumentLink::data から文書 URI を抽出 (resolve handler が analysis
/// 取得に使う)。data 不正/欠落時は None。
#[must_use]
pub fn link_uri(link: &DocumentLink) -> Option<String>;

/// DocumentLink を解決し target URI を埋める (#119)。
///
/// `data` から DocumentLinkData を復元し、§2.1 (文書相対+wsフォールバック)・
/// §2.2 (バックスラッシュ正規化)・§2.4 (`..` クランプ drop) を適用して
/// target を計算。`workspace_root` が None の場合はクランプ判定をスキップ。
/// 解決不能(data 不正・クランプ drop)の場合は元リンクをそのまま返す
/// (code_lens.rs resolve_lens のフォールバックと対称)。
///
/// **純粋関数契約**: server/lock に依存しない。analysis と workspace_root
/// を引数で受け取るため単体テスト可能。
#[must_use]
pub fn resolve_link(
    link: DocumentLink,
    analysis: &DocumentAnalysis,
    workspace_root: Option<&Url>,
) -> Option<DocumentLink>;
```

### 4.4 新規: 内部ヘルパー (private)

```rust
/// 単一/二重引用符をパス文字列から剥離する。
/// `'a/b.sql'` → `a/b.sql`、`"a/b.sql"` → `a/b.sql`。
/// 引用符なしはそのまま返す。
fn strip_quotes(text: &str) -> &str;

/// §3 の行頭判定。Colon トークンが行の先頭(先行は空白/タブのみ)にあるか。
fn is_at_line_start(analysis: &DocumentAnalysis, colon_span: Span) -> bool;

/// §2.1/2.2/2.4 の解決ロジック。
/// 文書相隔て優先 + ws フォールバック + `..` クランプ drop。
/// 解決した URL、または解決不能(クランプ drop 含む)なら None。
fn resolve_path_to_url(
    raw_path: &str,
    doc_uri: &Url,
    workspace_root: Option<&Url>,
) -> Option<Url>;
```

### 4.5 新規: `DocumentLinkConfig` (config.rs 追加・本 design が完全指定)

```rust
// crates/ase-ls-core/src/config.rs (Task 5 で追加)
/// Document Link 挙動 (#119)。
/// デフォルトは有効 (pre-#119 では機能自体が無かったため、未設定時に
/// リンクを生成しないことは後方互換性を損なわない)。オプトアウト可能。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DocumentLinkConfig {
    /// `:r` ディレクティブのパスに DocumentLink を生成するか (default: true)。
    pub enable_document_links: bool,
}

impl Default for DocumentLinkConfig {
    fn default() -> Self {
        Self { enable_document_links: true }
    }
}
```

`Config` 構造体に `pub document_links: DocumentLinkConfig` フィールドを追加
(`#[serde(default)]` 付き Config への追加は `InlayConfig` と同様)。
`Config::from_value` に `document_links: from_section(root.get("documentLinks"))`
を追加 (§4.6)。

### 4.6 config.rs 配線 (変更点・Task 5)

```rust
// Config 構造体にフィールド追加 (既存 inlay の直後):
pub struct Config {
    pub formatting: FormattingConfig,
    pub diagnostics: DiagnosticsConfig,
    pub completion: CompletionConfig,
    pub inlay: InlayConfig,
    pub document_links: DocumentLinkConfig,  // ← 追加
}

// Config::from_value に追加:
document_links: from_section(root.get("documentLinks")),
```

wire フォーマット (camelCase): `{ "ase-ls": { "documentLinks": { "enableDocumentLinks": false } } }`

### 4.7 server.rs 配線 (変更点・Task 3)

capability 宣言 (`ServerCapabilities` に追加)。

> **lsp-types 0.94 型の注意**: `document_link_provider` は
> `Option<DocumentLinkOptions>` であり `OneOf` で包まれていない
> (server.rs:577 の `code_lens_provider: Some(CodeLensOptions { .. })` と同形式。
> `inlay_hint_provider` の方が `OneOf<...>` で包まれているため混同しやすい —
> 0.94 では document_link は包まれない。実証: `lsp-types-0.94.1/src/lib.rs:1983`)。

```rust
document_link_provider: Some(DocumentLinkOptions {
    resolve_provider: Some(true),   // §2.5
    work_done_progress_options: WorkDoneProgressOptions { work_done_progress: None },
}),
```

handler (thin adapter・code_lens/inlay_hints と対称):

```rust
async fn document_link(&self, params: DocumentLinkParams)
    -> Result<Option<Vec<DocumentLink>>> {
    let uri = &params.text_document.uri;
    let Some(analysis) = self.get_analysis(uri).await else { return Ok(None); };
    let cfg = self.config.read().await.document_links.clone();
    Ok(Some(document_links::document_links(&analysis, uri, &cfg)))
}

async fn document_link_resolve(&self, params: DocumentLink)
    -> Result<DocumentLink> {
    // code_lens_resolve と対称: link.data から URI 復元 → analysis 取得 →
    // workspace_root 取得 → resolve_link。フォールバックで元リンク返却。
    match document_links::link_uri(&params)
        .and_then(|s| Url::parse(&s).ok())
    {
        Some(uri) => {
            if let Some(analysis) = self.get_analysis(&uri).await {
                let ws = self.workspace_root().await;  // 既存の workspace root 取得
                if let Some(resolved) = document_links::resolve_link(
                    params.clone(), &analysis, ws.as_ref(),
                ) {
                    return Ok(resolved);
                }
            }
            Ok(params)
        }
        None => Ok(params),
    }
}
```

`workspace_root()` の取得方法は server.rs の既存 workspace folder 管理に従う
(Task 3 で確認・本 design では `Option<Url>` を返す契約のみ規定)。

---

## 5. ユースケース (受け入れ基準の検証可能仕様)

### UC-1: クォート付き相対パス (標準)

```
入力 (file:///proj/scripts/main.sql):
:r 'sub/init.sql'
```
期待:
- `documentLink`: Range = パス部分 (`'sub/init.sql'` の範囲)、target=None、data=DocumentLinkData
- `resolve`: target = `file:///proj/scripts/sub/init.sql` (文書相対)

### UC-2: Windows バックスラッシュ (クォート内)

```
入力 (file:///proj/scripts/main.sql):
:r 'sub\init.sql'
```
期待:
- resolve 時に `\` → `/` 正規化 (§2.2)
- target = `file:///proj/scripts/sub/init.sql`

### UC-3 (エッジ): クォートなしパス → リンク生成なし

```
入力:
:r sub/init.sql
```
期待: リンク0件 (クォート必須・§2.3 非スコープ)。broken ではなく空配列。

### UC-4 (エッジ): 行頭でない `:r` → リンク生成なし

```
入力:
foo :r 'bar.sql'
```
期待: リンク0件 (§3 行頭制約)。

### UC-5 (エッジ): `..` オーバーフロー → drop

```
入力 (file:///proj/scripts/main.sql, ws_root=file:///proj/):
:r '../../../../etc/passwd'
```
期待: resolve 時に ws 配下でないため target 計算せず None → 元リンク返却。

### UC-6 (エッジ): broken span → skip

multi-line broken-span (parser の `span.end=0` 問題) でパストークン span が
壊れている場合、ガードでスキップ (inlay_hints.rs:105 と対称)。

### UC-7: ワークスペースフォールバック — **非スコープ (別 issue・L)**

文書相対で解決したファイルが存在しない場合の ws_root 相対再試行は、
DocumentStore 存在確認を要し純粋関数境界の再設計を伴うため **MVP では扱わない**
(§5.1 参照)。§4.3 の `resolve_link` 純粋関数シグネチャは **文書相対のみ** を
前提とし、`workspace_root` 引数は `..` クランプ判定 (§2.4) のみに使用する。
**§2.1 の「ワークスペース相対フォールバック」は目標仕様だが、MVP 実装範囲から
は外す** — §2.1 は将来 L 機能が `resolve_link` シグネチャを拡張する際の設計意図
として残すが、Task 2-7 (本 #119) は文書相対のみを実装する。

### 5.1 段階的スコープ (MVP vs フォローアップ)

**MVP (本 #119)**:
- UC-1, UC-2, UC-3, UC-4, UC-5, UC-6 (文書相対のみ)
- `resolve_path_to_url` (§4.4) は文書相対のみ計算 + `..` クランプ drop
- `resolve_link` シグネチャは §4.3 の通り (変更なし・`workspace_root` は §2.4 クランプ判定のみ)

**フォローアップ (別 issue・L)**:
- UC-7 ワークスペースフォールバック (DocumentStore 存在確認が必要・
  純粋関数境界の再設計を伴うため分離)。§2.1 を完全実装する際に着手。
- クォートなしパスのトークン再結合 (F-5 で非現実的・低優先)

---

## 6. 契約 (暗黙期待の文書化)

| 契約 | 内容 |
|------|------|
| C-1 | `document_links` は server/lock に依存しない純粋関数 (code_lens/inlay_hints と対称) |
| C-2 | `resolve_link` は `link` を消費(move)し、解決不能時は元 link を `Some` で返す (drop しない・code_lens resolve_lens と対称) |
| C-3 | 行頭判定(§3)は `LineIndex` 経由で source bytes を読む。CRLF は line_index が吸収済み |
| C-4 | バックスラッシュ正規化(§2.2)は resolve 時(`resolve_path_to_url`)に1回のみ。document_links フェーズでは生パスを data に stash |
| C-5 | `..` クランプ(§2.4)は `workspace_root: Some` の場合のみ。None なら判定スキップ(単一ファイルオープン互換) |
| C-6 | クォート剥離(§4.4 strip_quotes)は document_links フェーズで行い、data.path は剥離済みを格納 |

---

## 7. テスト計画 (Task 6)

単体テスト (document_links.rs 内 `#[cfg(test)]`・3つの `#[allow]` 付与):

正常系 (≥4):
1. UC-1 クォート付き相対パス → 1リンク・data あり・target なし
2. UC-1 resolve → target = 文書相対 URL
3. UC-2 バックスラッシュ resolve → 正規化済み URL
4. 大文字 `:R` 認識

エッジケース (≥4):
5. UC-3 クォートなし → 0リンク
6. UC-4 行頭でない → 0リンク
7. UC-5 `..` オーバーフロー → resolve で None (元リンク返却)
8. UC-6 broken span → skip
9. data 欠落 resolve → None
10. 二重引用符 QuotedIdent → best-effort リンク生成
11. config `enable_document_links: false` → 0リンク

config.rs テスト: DocumentLinkConfig default=true、camelCase ラウンドトリップ、
from_value セクション独立 fallback (InlayConfig テストと対称)。

---

## 8. 非スコープ

- ワークスペースフォールバック (UC-7) — L 機能・別 issue (§5.1)
- クォートなしパスのトークン再結合 (F-5 で非現実的)
- `:setvar` 等他の SQLCMD ディレクティブ (本 issue は `:r` のみ)
- ターゲットファイルの内容プレビュー / ジャンプ先定義表示
- BOM 先頭行の行頭判定
- リンクの tooltip 表示 (MVP では None・将来拡張)

---

## 9. 依存

- `DocumentAnalysis` (既存・analysis.rs)
- `LineIndex` (既存・line_index.rs) — §3 行頭判定
- `DocumentLinkConfig` (新規・config.rs・§4.5) — Task 5
- `lsp-types 0.94` `DocumentLink`/`DocumentLinkOptions` (F-10)
- code_lens.rs `LensData` パターン (PR #174) — data stash 設計の参照元
- inlay_hints.rs broken-span ガード (PR #176) — §2.3 ガードの参照元
