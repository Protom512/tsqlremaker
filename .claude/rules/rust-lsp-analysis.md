# Rust LSP Analysis Rules

rust-analyzerのLSP機能を使用して、Rustコードの解析精度と効率を向上させるルールを定義します。

## 原則

**LSPを活用した正確なコード理解**: 静的解析ツール（Grep/Glob）のみに依存せず、rust-analyzerのLSP機能を併用して、シンボルの定義位置、参照、型情報を正確に特定する。

---

## LSP使用ガイドライン

### 1. シンボル定義の検索

**❌ 禁止: Grepのみで推測**
```bash
# 曖昧で複数ヒットする可能性
grep -r "struct Token" crates/
```

**✅ 推奨: LSP goToDefinition**
```
ファイル内のシンボル位置で LSP goToDefinition を実行
→ 正確な定義位置にジャンプ
```

### 2. 参照の検索

**❌ 禁止: Grepで文字列検索**
```bash
# 同名の異なるシンボルもヒットする
grep -r "ParseError" crates/
```

**✅ 推奨: LSP findReferences**
```
シンボル上で LSP findReferences を実行
→ そのシンボルのすべての使用箇所を取得
```

### 3. 型情報の取得

**❌ 禁止: コードを推測で読む**
```rust
// 型注釈がない場合、推測が必要
let tokens = lexer.collect();  // 何の型？
```

**✅ 推奨: LSP hover**
```
変数/式上で LSP hover を実行
→ 型情報とドキュメントを取得
```

---

## LSP操作の使用タイミング

| 操作 | 使用タイミング | 目的 |
|------|---------------|------|
| `goToDefinition` | シンボルの定義を知りたい | 正確な定義位置へ移動 |
| `findReferences` | シンボルの使用箇所を知りたい | 影響範囲分析 |
| `hover` | 型/ドキュメントを知りたい | コードの理解 |
| `documentSymbol` | ファイルの構造を把握 | モジュール/関数の一覧 |
| `workspaceSymbol` | ワークスペース全体から検索 | シンボルの所在特定 |
| `prepareCallHierarchy` | 呼び出し関係を知りたい | 呼び出し元/先の追踪 |
| `incomingCalls` | 誰がこの関数を呼んでいる | 依存関係解析 |
| `outgoingCalls` | この関数が何を呼んでいる | 振る舞い理解 |

---

## 実装パターン

### パターン1: シンボル定義の調査

```python
# 1. まず workspaceSymbol でシンボルを検索
LSP(workspaceSymbol, query="Token")

# 2. 見つかったシンボルの位置で goToDefinition
LSP(goToDefinition, file="...", line=10, character=5)

# 3. 必要に応じて hover で型情報を確認
LSP(hover, file="...", line=10, character=5)
```

### パターン2: 影響範囲の分析

```python
# 1. 変更対象のシンボルで findReferences
LSP(findReferences, file="crates/foo/src/lib.rs", line=42, character=7)

# 2. 呼び出し階層を確認
LSP(prepareCallHierarchy, file="...", line=42, character=7)
LSP(incomingCalls, ...)  # 呼び出し元を全取得
```

### パターン3: 新規実装時の構造把握

```python
# 1. ファイルのシンボル一覧を取得
LSP(documentSymbol, file="crates/parser/src/lib.rs")

# 2. 既存の関連シンボルを検索
LSP(workspaceSymbol, query="Parser")

# 3. 参照を確認して使用パターンを理解
LSP(findReferences, file="...", line=..., character=...)
```

---

## LSPと静的解析の使い分け

### LSPを使用する場合

| シナリオ | LSP操作 |
|---------|---------|
| 特定の関数/構造体の定義を見つける | `goToDefinition` |
| 変数/式の型を知る | `hover` |
| 関数のすべての呼び出し箇所を見つける | `findReferences` |
| 呼び出し関係グラフを構築する | `prepareCallHierarchy` + `incoming/outgoingCalls` |
| ファイル内のすべての関数を一覧する | `documentSymbol` |
| ワークスペース全体からトレイトを探す | `workspaceSymbol` |

### Grep/Globを使用する場合

| シナリオ | ツール |
|---------|-------|
| 文字列パターンの検索（コメント、ログ等） | `Grep` |
| ファイルパターンの検索 | `Glob` |
| 複数ファイル間の共通パターン発見 | `Grep` |
| リテラル文字列の検索 | `Grep` |

---

## LSPエラー時のフォールバック

```python
# LSPが利用できない場合のフォールバック戦略

try:
    # LSPで正確な参照検索
    refs = LSP(findReferences, ...)
except LSPError:
    # フォールバック: Grepで検索（結果をフィルタ必要）
    results = Grep(pattern="TokenKind::SELECT", glob="*.rs")
    # 結果から偽陽性を除外
```

---

## rust-analyzerの設定

### 推奨設定

```json
{
  "rust-analyzer.cargo.loadOutDirsFromCheck": true,
  "rust-analyzer.procMacro.enable": true,
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.hover.actions.enable": true,
  "rust-analyzer.lens.enable": true
}
```

---

## タスク実行時のワークフロー

### 新規機能実装時

1. **関連シンボルの特定**
   ```
   LSP(workspaceSymbol, query="<関連型名>")
   ```

2. **既存実装の調査**
   ```
   LSP(goToDefinition, ...)  → 定義へ
   LSP(findReferences, ...)  → 使用箇所を確認
   LSP(hover, ...)           → 型情報を確認
   ```

3. **呼び出し関係の理解**
   ```
   LSP(prepareCallHierarchy, ...)
   LSP(incomingCalls, ...)  # 上流依存を確認
   LSP(outgoingCalls, ...)  # 下流依存を確認
   ```

### リファクタリング時

1. **影響範囲の特定**
   ```
   LSP(findReferences, file=..., line=..., character=...)
   ```

2. **呼び出し階層の確認**
   ```
   LSP(incomingCalls, ...)  # 全呼び出し元
   ```

3. **型の整合性確認**
   ```
   LSP(hover, ...)  # 各使用箇所で型確認
   ```

---

## 注意事項

### LSPの制約

1. **ビルドが必要**: rust-analyzerが正しく動作するには、プロジェクトがビルド可能である必要がある
2. **インデックス作成時間**: 初回起動時にインデックス作成が完了するまで待つ
3. **マクロ展開**: プロシージャルマクロを含む場合は `procMacro.enable` を有効にする

### LSPが利用できない状況

- ビルドが壊れている場合
- 新規クレートでまだビルドしていない場合
- 依存関係が解決されていない場合

これらの場合は、Grep/Globをフォールバックとして使用する。

---

## チェックリスト

コード解析時に以下を確認してください：

- [ ] シンボル定義の調査で `LSP(goToDefinition)` を使用した
- [ ] 型情報の取得で `LSP(hover)` を使用した
- [ ] 参照検索で `LSP(findReferences)` を使用した
- [ ] 呼び出し関係で `LSP(prepareCallHierarchy)` を使用した
- [ ] LSPが利用できない場合のみ Grep をフォールバックとして使用した

---

## 例: LSPを使用した調査手順

### シナリオ: `ParseError` の使用箇所をすべて調査

1. **シンボルの定義を確認**
   ```
   LSP(workspaceSymbol, query="ParseError")
   → 定義位置: crates/tsql-parser/src/error.rs:15
   ```

2. **すべての参照を取得**
   ```
   LSP(findReferences, file="crates/tsql-parser/src/error.rs", line=15, character=6)
   → 45箇所で使用されている
   ```

3. **主要な使用箇所を確認**
   ```
   LSP(hover, file="crates/tsql-parser/src/parser.rs", line=100, character=15)
   → 型: ParseError
   ```

4. **呼び出し階層を確認**
   ```
   LSP(prepareCallHierarchy, file="...", line=..., character=...)
   LSP(incomingCalls, ...)
   → parse_statement(), parse_expression() などから呼ばれている
   ```

---

## 関連ルール

- `.claude/rules/rust-style.md` - Rustコーディングスタイル
- `.claude/rules/rust-anti-patterns.md` - Rustアンチパターン
- `.claude/rules/architecture-coupling-balance.md` - アーキテクチャルール
