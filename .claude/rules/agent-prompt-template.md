# Agent Prompt Template

エージェント（Agent tool）にタスクを依頼する際の標準プロンプトテンプレート。
エージェントの失敗率を下げ、出力品質を安定化する。

## 原則

**エージェントは文脈を持たない** — 必要な情報を全てプロンプトに含める。

---

## プロンプト構成要素

### 1. コンテキスト（必須）

エージェントが「なぜこの作業をするのか」「全体のどこに位置するのか」を理解するための情報。

```
## コンテキスト
- プロジェクト: SAP ASE T-SQL Language Server
- 対象モジュール: [モジュール名]
- 関連PR: #[番号]
- ベースブランチ: [ブランチ名]
```

### 2. タスク定義（必須）

何を作成・変更すべきかの具体的な指示。

```
## タスク
[ファイルパス] に [機能名] を実装してください。

### 入力
- [前提となるデータ・API]

### 期待出力
- [生成されるコード・ファイル]
```

### 3. API署名（必須）

使用する型・メソッドの**実際の署名**を含める。推測させない。

```
## 使用するAPI

### tsql-parser (crates/tsql-parser/src/ast/mod.rs)
pub enum Statement {
    Select(Box<SelectStatement>),
    Insert(Box<InsertStatement>),
    // ...
}

### Identifier (crates/tsql-parser/src/ast/expression.rs)
pub struct Identifier {
    pub name: String,    // String, &str ではない
    pub span: Span,
}
```

### 4. テスト要件（必須）

```
## テスト要件
- 正常系: [N]パターン以上
- エッジケース: [N]パターン以上
- テストモジュールに #[allow(clippy::unwrap_used)] #[allow(clippy::panic)] #[allow(clippy::expect_used)] を追加
```

### 5. 品質チェック（必須）

```
## 品質チェック
必ず以下を実行してパスすることを確認:
1. cargo fmt --all --check
2. cargo clippy --all-targets -- -D warnings
3. cargo test --all
```

---

## テンプレート

```
## コンテキスト
- プロジェクト: SAP ASE T-SQL Language Server
- 対象: [ファイルパス]
- ベースブランチ: [ブランチ名]

## タスク
[具体的な実装内容]

## 使用するAPI（ソースコード確認済み）
[型定義、メソッド署名]

## テスト要件
- 正常系: [N]件以上
- エッジケース: [N]件以上
- テストモジュールに3つの #[allow] を追加

## 品質チェック
1. cargo fmt --all
2. cargo clippy --all-targets -- -D warnings
3. cargo test --all

## 注意事項
- lsp-types 0.94 のAPIを使用（0.97 ではない）
- ライブラリコードで unwrap/expect/panic は禁止
- テストでは #[allow] を追加
```

---

## よくある失敗と対策

| 失敗 | 原因 | 対策 |
|------|------|------|
| 型の署名が違う | 推測でAPIを使った | 使用するAPIの署名をプロンプトに含める |
| コンパイルエラー | ベースブランチが違う | ベースブランチを明示 |
| fmt/clippy エラー | チェックを実行しなかった | 品質チェックを必須要素に含める |
| テストが不十分 | 何をテストすべきか不明 | テスト要件で正常系・エッジケースの数を指定 |
| Worktreeの変更が消える | コミットせずに完了した | 「必ずコミットすること」を明記 |

---

## チェックリスト

エージェントにタスクを依頼する前に:

- [ ] コンテキスト（プロジェクト、対象、ベースブランチ）を含めた
- [ ] タスクの入力と期待出力を明確にした
- [ ] 使用するAPIの署名をソースコードで確認して含めた
- [ ] テスト要件（正常系・エッジケースの数）を指定した
- [ ] 品質チェック（fmt/clippy/test）を含めた
- [ ] 注意事項（バージョン制約等）を含めた
