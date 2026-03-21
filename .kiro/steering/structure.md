# T-SQL Remaker - Project Structure

## ワークスペース構成

```
tsqlremaker/
├── Cargo.toml                 # ワークスペースルート
├── .kiro/                     # Spec Driven Development
│   ├── steering/              # プロジェクト全体のガイドライン
│   └── specs/                 # 個別機能の仕様
├── crates/
│   ├── tsql-lexer/            # SAP ASE T-SQL 字句解析器
│   ├── tsql-parser/           # T-SQL 構文解析器
│   ├── common-sql/            # 方言非依存 AST
│   ├── mysql-emitter/         # MySQL コード生成
│   └── tsql-remaker/          # CLI アプリケーション
└── tests/                     # 統合テスト
```

## クレートの境界

### tsql-lexer
- **責務**: 文字列 → トークンストリーム
- **依存先**: なし
- **公開型**: `Token`, `TokenKind`, `Lexer`

### tsql-parser
- **責務**: トークンストリーム → Common SQL AST
- **依存先**: `tsql-lexer`, `common-sql`
- **公開型**: `Parser`, `ParseError`

### common-sql
- **責務**: 方言非依存の共通 AST
- **依存先**: なし（他から依存される）
- **公開型**: `Statement`, `Expression`, `Visitor`, `DataType`

### mysql-emitter
- **責務**: Common SQL AST → MySQL SQL
- **依存先**: `common-sql`
- **公開型**: `MySqlEmitter`

### tsql-remaker
- **責務**: CLI エントリーポイント
- **依存先**: 全クレート

## 依存方向のルール

```
┌─────────────────┐
│   MySQL Emitter │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  Common SQL AST │ (中間層)
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│     Parser      │
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│     Lexer       │
└─────────────────┘
```

**重要**: 下位が上位に依存してはならない

## ディレクトリ構成規約

```
crates/{crate-name}/
├── Cargo.toml
├── src/
│   ├── lib.rs              # 公開API
│   ├── error.rs            # エラー型（必要な場合）
│   └── (module files)
└── tests/                  # クレート固有の統合テスト
    ├── (integration tests)
    └── fixtures/
        └── (test data)
```

## ファイル命名規約

- モジュール: `snake_case.rs`
- 統合テスト: `{module}_tests.rs`
- フィクスチャ: 意味のわかる名前で`.sql`拡張子
