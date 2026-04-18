# LSP Server Development Skill

このプロジェクトでの Language Server 開発に特化したスキル。

## トリガー

- "language server", "LSP", "lsp-types", "tower-lsp"
- 新しいLSP機能の実装
- `ase-ls` / `ase-ls-core` クレートの変更

---

## アーキテクチャ

```
ase-ls (tower-lsp サーバー)
  │
  │  LSP プロトコル (JSON-RPC over stdio)
  │
  └── ase-ls-core (LSP ロジック)
        │
        ├── code_actions      → DDLクイックフィックス（SELECT *展開、INSERT骨組み、TRY...CATCH）
        ├── completion        → 補完候補生成
        ├── definition        → Go to Definition（シンボルテーブル使用）
        ├── diagnostics       → パースエラー → Diagnostic
        ├── folding           → フォールディング範囲
        ├── formatting        → SQLフォーマッティング（キーワード大文字化、改行）
        ├── hover             → 型情報・スキーマ情報表示
        ├── references        → Find References（トークンマッチング）
        ├── rename            → シンボル一括リネーム
        ├── semantic_tokens   → セマンティックハイライト
        ├── signature_help    → 関数シグネチャ・パラメータ表示
        ├── symbol_table      → ASTからのシンボル抽出基盤
        ├── symbols           → ドキュメントシンボル
        └── workspace_symbols → ワークスペース横断シンボル検索
              │
              └── tsql-parser → tsql-lexer → tsql-token
```

### 責務分離

| クレート | 責務 | 依存 |
|---------|------|------|
| `ase-ls` | LSP通信、ドキュメント管理 | tower-lsp, ase-ls-core |
| `ase-ls-core` | LSPロジック（型変換等） | lsp-types, tsql-parser |
| `tsql-parser` | SQL解析 | tsql-lexer |
| `tsql-lexer` | 字句解析 | tsql-token |

---

## バージョン互換性（最重要）

| パッケージ | バージョン | 備考 |
|-----------|-----------|------|
| tower-lsp | 0.20 | LSP フレームワーク |
| lsp-types | **0.94** | tower-lsp 0.20 とペア |
| tokio | 1 | 非同期ランタイム |

**⚠️ lsp-types のバージョンを変えてはならない。** 0.97 等は tower-lsp 0.20 と非互換。

---

## 新しいLSP機能追加のテンプレート

### ase-ls-core 側

```rust
// crates/ase-ls-core/src/new_feature.rs

use crate::offset_to_position;
use lsp_types::*;

/// 新機能のエントリポイント
pub fn new_feature(source: &str) -> Option<NewFeatureResult> {
    // 1. Parser/Lexer でソースを解析
    let mut parser = tsql_parser::Parser::new(source);
    let statements = match parser.parse() {
        Ok(s) => s,
        Err(_) => return None,
    };

    // 2. 結果を LSP 型に変換
    let results: Vec<_> = statements
        .iter()
        .filter_map(|stmt| convert_statement(stmt))
        .collect();

    if results.is_empty() {
        None
    } else {
        Some(results)
    }
}

fn convert_statement(stmt: &Statement) -> Option<SomeLspType> {
    match stmt {
        // ...
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        // ...
    }
}
```

### ase-ls 側

```rust
// crates/ase-ls/src/server.rs にハンドラーを追加

// ServerCapabilities に機能を登録
async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
    Ok(InitializeResult {
        capabilities: ServerCapabilities {
            // 既存の機能...
            new_feature_provider: Some(/* ... */),
            ..ServerCapabilities::default()
        },
        // ...
    })
}

// ハンドラー実装
async fn new_feature_handler(&self, params: NewFeatureParams) -> Result<Option<NewFeatureResult>> {
    let docs = self.documents.read().await;
    if let Some(source) = docs.get(params.text_document.uri.as_str()) {
        Ok(new_feature::new_feature(source))
    } else {
        Ok(None)
    }
}
```

### Cargo.toml の更新

```toml
# ase-ls-core に必要なら依存を追加
[dependencies]
# 既存の依存に加えて...
```

---

## よく使うLSP型変換パターン

### バイトオフセット → LSP Position

```rust
// ase-ls-core で提供
use crate::offset_to_position;

let (line, character) = offset_to_position(source, byte_offset);
// line, character は 0-indexed（LSP仕様）
```

### Span → LSP Range

```rust
use lsp_types::{Position, Range};

fn span_to_range(source: &str, span: &Span) -> Range {
    let (start_line, start_char) = offset_to_position(source, span.start);
    let (end_line, end_char) = offset_to_position(source, span.end);
    Range {
        start: Position { line: start_line, character: start_char },
        end: Position { line: end_line, character: end_char },
    }
}
```

### SemanticTokens の差分エンコーディング

```rust
// LSP は差分エンコーディングを要求
let delta_line = line.saturating_sub(prev_line);
let delta_start = if delta_line == 0 {
    character.saturating_sub(prev_char)
} else {
    character
};

tokens.push(SemanticToken {
    delta_line,
    delta_start,
    length: token_len,
    token_type: type_idx,
    token_modifiers_bitset: 0,
});
```

---

## Lexer の機能選択

| 機能 | デフォルト | `with_comments(true)` |
|------|-----------|----------------------|
| キーワード | 〇 | 〇 |
| 識別子 | 〇 | 〇 |
| リテラル | 〇 | 〇 |
| 行コメント (`--`) | 〇 | 〇 |
| **ブロックコメント (`/* */`)** | **✗** | **〇** |
| 変数 (`@`, `@@`) | 〇 | 〇 |

**フォールディングやホバー等でブロックコメントを扱う場合は `with_comments(true)` が必須。**

---

## テスト実行

```bash
# ase-ls-core のテスト
cargo test -p ase-ls-core

# ase-ls のテスト
cargo test -p ase-ls

# 全体テスト
cargo test --all

# Clippy
cargo clippy --all-targets -- -D warnings
```

---

## DB Language Server のデファクト機能一覧

### 基本機能（実装済み）

- [x] Diagnostics（構文エラー表示）
- [x] Semantic Tokens（シンタックスハイライト）
- [x] Document Symbols（アウトライン表示）
- [x] Folding Ranges（コード折りたたみ）
- [x] Completion（キーワード・データ型・関数補完）

### Phase 2 機能（実装済み）

- [x] Hover（型情報・スキーマ情報表示）
- [x] Document Formatting（SQLフォーマッティング）
- [x] Signature Help（関数シグネチャ表示）

### Phase 3 機能（実装済み）

- [x] Go to Definition（テーブル・プロシージャ・変数定義へジャンプ）
- [x] Find References（テーブル・変数参照箇所の検索）
- [x] Symbol Table Builder（ASTからのシンボル抽出基盤）

### Phase 4 機能（実装済み）

- [x] Workspace Symbols（ワークスペース横断シンボル検索）
- [x] Code Actions（SELECT *展開、INSERT骨組み、TRY...CATCHラッパー）
- [x] Rename（変数・テーブル・プロシージャの一括リネーム）

### 将来機能（Phase 5+）

- [ ] Document Links（URLリンク認識）
- [ ] Code Lens（参照カウント表示等）
- [ ] Inlay Hints（型注釈のインライン表示）
- [ ] Incremental Sync（差分ベースのドキュメント同期）
- [ ] Configuration（ユーザー設定の変更）
- [ ] Multi-root Workspace（複数ルート対応）

---

## Phase 2 実装パターン

### Hover 実装パターン

```rust
// hover.rs - カーソル位置のトークンを特定してドキュメントを返す

// 1. Position → バイトオフセットの変換が必要
fn position_to_offset(source: &str, position: Position) -> usize { /* ... */ }

// 2. Lexer でトークンをスキャン（Result<Token, LexError> を処理）
pub fn hover(source: &str, position: Position) -> Option<Hover> {
    let offset = position_to_offset(source, position);
    let lexer = Lexer::new(source);

    for token_result in lexer {
        let token = match token_result {
            Ok(t) => t,
            Err(_) => continue,
        };
        // カーソルがトークン範囲内かチェック
        if offset >= token.span.start as usize && offset < token.span.end as usize {
            // ドキュメント生成
            return Some(Hover { /* ... */ });
        }
    }
    None
}

// 3. 静的ドキュメントは Lazy<HashMap> で定義
static KEYWORD_DOCS: Lazy<HashMap<&str, (&str, &str)>> = Lazy::new(|| { /* ... */ });
```

### Formatting 実装パターン

```rust
// formatting.rs - トークンストリームを再構築してフォーマット

pub fn format(source: &str) -> Vec<TextEdit> {
    let formatted = format_sql(source);
    if formatted == source { return Vec::new(); }
    // 全体を一括置換する TextEdit を返す
    vec![TextEdit { range: full_range, new_text: formatted }]
}

fn format_sql(source: &str) -> String {
    // ⚠️ with_comments(true) が必須（デフォルトではコメントがスキップされる）
    let lexer = Lexer::new(source).with_comments(true);
    let tokens: Vec<_> = lexer.filter_map(Result::ok).collect();
    // トークンを再構築しながらフォーマット
}
```

### Signature Help 実装パターン

```rust
// signature_help.rs - 関数呼び出しの引数位置を追跡

pub fn signature_help(source: &str, position: Position) -> Option<SignatureHelp> {
    // 1. '(' の後のみ有効 - found_open_paren チェックが必須
    // 2. カンマで active_param をインクリメント
    // 3. ネストした括弧は paren_depth で追跡
    // 4. found_open_paren が false の場合は None を返す
    // ⚠️ lsp-types 0.94 では SignatureInformation に active_parameter フィールドが必須
}
```

### 重要な注意事項

1. **Lexer は `Result<Token, LexError>` を返す**: `filter_map(Result::ok)` で処理
2. **ブロックコメント**: `Lexer::new(source).with_comments(true)` が必要
3. **lsp-types 0.94**: `SignatureInformation.active_parameter` フィールドが必須
4. **ServerCapabilities**: 各プロバイダーの型は lsp-types 0.94 の定義に合わせる

---

## Phase 4 実装パターン

### Workspace Symbols 実装パターン

```rust
// workspace_symbols.rs - シンボルテーブルからクエリでフィルタリング

pub fn workspace_symbols(source: &str, query: &str, uri: &Url) -> Vec<SymbolInformation> {
    let table = SymbolTableBuilder::build_tolerant(source);
    // 大文字小文字区別なしの部分一致
    let query_upper = query.to_uppercase();

    // 各シンボル種別（tables, procedures, views, indexes, variables）をスキャン
    // #[allow(deprecated)] が必要（SymbolInformation.deprecated フィールド）
}

// tower-lsp 0.20 での戻り値型:
// Result<Option<Vec<SymbolInformation>>>
// （NOT WorkspaceSymbolResponse）
```

### Code Actions 実装パターン

```rust
// code_actions.rs - コンテキスト感知のクイックフィックス

pub fn code_actions(source: &str, range: Range, uri: &Url) -> Vec<CodeActionOrCommand> {
    // 1. カーソル行のテキストを取得
    // 2. レジリエントシンボルテーブルを構築（不完全SQL対応）
    // 3. パターンマッチでアクションを生成:
    //    - "SELECT * FROM table" → カラム展開 (QUICKFIX)
    //    - "INSERT INTO table" → VALUES骨組み (QUICKFIX)
    //    - "BEGIN" → TRY...CATCH ラッパー (REFACTOR)
}

// レジリエントパース: 完全パース失敗時は前方から徐々に短くして再試行
fn build_resilient_symbol_table(source: &str) -> SymbolTable {
    let table = SymbolTableBuilder::build_tolerant(source);
    if !table.tables.is_empty() { return table; }
    // フォールバック: 行数を減らして再パース
}
```

### Rename 実装パターン

```rust
// rename.rs - トークンレベルの全参照箇所をリネーム

pub fn rename(source: &str, position: Position, new_name: &str, uri: &Url) -> Option<WorkspaceEdit> {
    // 1. カーソル位置のトークンを特定
    // 2. 大文字小文字区別なしで全トークンをスキャン
    // 3. マッチした全箇所に TextEdit を生成
    // 4. 重複除去（同じ位置の複数editを防止）
}

// バリデーション:
// - 変数(@var)のリネーム: new_name は @ プレフィクス必須
// - 空文字の new_name は拒否
// - 空白位置でのリネームは None

// lsp-types 0.94 の注意:
// RenameParams.text_document_position （NOT text_document_position_params）
```

---

## 新機能追加時のプロセス

### 実装前チェック（必須）

1. `.claude/rules/pre-implementation-checklist.md` に従う
2. `.claude/rules/lsp-use-case-template.md` でシナリオを定義
3. Parser対応状況を `.claude/rules/project-ast-types.md` の「未対応構文」で確認
4. lsp-types 0.94 のAPI差異を `.claude/rules/dependency-version-compatibility.md` で確認

### コミット粒度

- 1機能 = 1コミット（モノリシックコミット禁止）
- `.claude/rules/git-branch-strategy.md` に従う

### テスト要件

- 正常系2件以上 + エッジケース1件以上
- テストモジュールに標準 #[allow] 3つを追加
- server.rs のハンドラーにも統合テストを追加（将来）
