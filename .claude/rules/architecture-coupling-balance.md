# Architecture Rules - Balanced Coupling

Vlad Khononovの「結合の均衡（Balanced Coupling）」原則に基づき、クリーンアーキテクチャとDDDの境界を守るルールを定義します。

**原則**: 無駄に疎結合にしてオーバーエンジニアリングを招き、システム全体の複雑性が制御不能になることを防ぐ。

---

## プロジェクトの境界づけられたコンテキスト

| コンテキスト | 責務 | 変動性 | Crate |
|-------------|------|--------|-------|
| **Lexer** | T-SQL → トークンストリーム | 低 | `tsql-lexer` |
| **Parser** | トークン → 構文木 | 低 | `tsql-parser` |
| **Common SQL AST** | 方言非依存の中間表現 | 中 | `common-sql` |
| **MySQL Emitter** | AST → MySQL SQL | 高 | `mysql-emitter` |
| **SAP ASE Dialect** | ASE固有構文の解釈 | 中 | `tsql-lexer` (一部) |

---

## 依存方向（単一方向）

```
┌─────────────────┐
│   MySQL Emitter │ (最上位: 高変動)
└────────┬────────┘
         │ 依独
         ▼
┌─────────────────┐
│  Common SQL AST │ (中間: 安定)
└────────┬────────┘
         │ 依存
         ▼
┌─────────────────┐
│     Parser      │ (下位: 安定)
└────────┬────────┘
         │ 依存
         ▼
┌─────────────────┐
│     Lexer       │ (最下位: 最も安定)
└─────────────────┘
```

**ルール**: 依存は常に「上位→下位」の方向。下位が上位に依存してはならない。

---

## 1. モデルの所有権と隔離

### 1.1 内部ドメインモデルの直接依存禁止

**❌ 禁止**:
```rust
// parser/src/lib.rs
use tsql_lexer::LexerInternalState;  // 内部実装に依存
```

**✅ 推奨**:
```rust
// parser/src/lib.rs
use tsql_lexer::{Lexer, Token, TokenKind};  // 公開APIのみ
```

### 1.2 コンテキスト間のデータ交換は共通ASTのみ

**❌ 禁止**:
```rust
// Parser が MySQL Emitter のモデルを使用
fn parse_and_emit(sql: &str) -> mysql_emitter::MySqlAst { }
```

**✅ 推奨**:
```rust
// Common SQL AST を介する
fn parse_and_emit(sql: &str) -> common_sql::ast::Statement { }
```

### 1.3 コントラクト結合の強制

コンテキスト間は必ず `trait` で定義されたコントラクトを使用すること。

```rust
// common-sql/src/lib.rs - コントラクト定義
pub trait Visitable {
    fn accept<V: Visitor>(&self, visitor: &mut V) -> V::Output;
}
```

---

## 2. 内部実装へのアクセス禁止

### 2.1 プライベートフィールドへの直接アクセス禁止

**❌ 禁止**:
```rust
let lexer = Lexer::new(sql);
let cursor_pos = lexer.cursor_position;  // プライベート
```

**✅ 推奨**:
```rust
let span = lexer.current_span();  // 公開メソッド
```

### 2.2 公開APIのみ使用

すべてのコンテキスト間通信は公開APIのみ。

```rust
// 各クレートの lib.rs で公開APIを明確に
// tsql-lexer/src/lib.rs
pub use token::{Token, TokenKind};
pub use lexer::Lexer;

// 内部は非公開
mod cursor;
mod error;
```

---

## 3. 結合強度と距離のルール

### 3.1 強い結合は近距離のみ許容

| 結合強度 | 許容距離 | 例 |
|---------|----------|-----|
| `Intrusive` | 同一モジュール内 | 同一ファイル内の関数呼び出し |
| `Functional` | 同一コンテキスト内 | 同一クレート内の関数呼び出し |
| `Data` | 近接コンテキスト | 構造体の受け渡し |
| `Contract` | 遠距離コンテキスト | **境界越えは必須** |

### 3.2 高凝集化

関連するロジックは同一境界内に凝縮。

**❌ 禁止**: 関連ロジックが分散
```
tsql-lexer/src/comment.rs
tsql-parser/src/comment.rs
```

**✅ 推奨**: 関連ロジックを集約
```
tsql-lexer/src/
  ├── comment.rs
  ├── string.rs
  └── number.rs
```

### 3.3 循環依存の禁止

**❌ 禁止**:
```
Lexer ←→ Parser  (循環依存)
```

**✅ 推奨**:
```
Parser → Lexer  (単方向)
```

---

## 4. 変更の隔離（変動性管理）

### 4.1 高変動コンテキストへの対策

変動性の高いコンテキストに依存する場合、**腐敗防止層（ACL）**を設ける。

**❌ 禁止**: 外部ライブラリに直接依存
```rust
use third_party_sql_parser::Parser;
```

**✅ 推奨**: 自前の抽象化
```rust
pub trait SqlParser {
    fn parse(&self, sql: &str) -> Result<Ast, Error>;
}

mod adapter {
    use third_party_sql_parser::Parser as ThirdPartyParser;
    // 実装は非公開
}
```

### 4.2 影響範囲分析

変更時に必ず実施:
- [ ] 変更対象の依存先を特定
- [ ] 公開APIへの影響を確認
- [ ] 下流コンテキストのテストへの影響を確認
- [ ] 破壊的変更ならバージョン更新を検討

---

## 5. 各コンテキストの制約

### Lexer (`tsql-lexer`)

- **所有**: `Token`, `TokenKind`, `Position`, `Span`
- **依存先**: なし（最下位）
- **公開API**: `Iterator<Item=Token>`
- **制約**: 他クレートから依存されても動作するよう安定

### Parser (`tsql-parser`)

- **所有**: `Statement`, `Expression` (ASE固有)
- **依存先**: `Lexer` のみ
- **公開API**: `parse(sql: &str) -> Result<Statement, ParseError>`
- **出力**: `Common SQL AST` へ変換

### Common SQL AST (`common-sql`)

- **所有**: 共通 `Statement`, `Expression`, `DataType`
- **依存先**: なし（他から依存される）
- **公開API**: `Visitor` trait, 各ASTノード

### MySQL Emitter (`mysql-emitter`)

- **所有**: MySQL生成ロジック
- **依存先**: `Common SQL AST` のみ
- **公開API**: `emit(stmt: &Statement) -> String`

---

## 6. 開発時のチェックリスト

### 新機能追加時
- [ ] 変更が必要なコンテキストを特定
- [ ] 他コンテキストへの波及を確認
- [ ] 公開APIの変更が必要か検討
- [ ] 統合テストを記述

### リファクタリング時
- [ ] コンテキスト境界をまたぐか確認
- [ ] 公開APIの変更は破壊的変更として扱う
- [ ] 下流コンテキストへの影響を評価

### バグ修正時
- [ ] 問題があるコンテキストを特定
- [ ] 可能な限り同一コンテキスト内で解決
- [ ] API変更は最小限に

---

## 7. 違反の例

### 違反1: 下位が上位に依存

```rust
// ❌ 禁止: Lexer が Parser を知っている
// tsql-lexer/src/lib.rs
use tsql_parser::ParseError;  // 下位が上位に依存
```

### 違反2: 内部実装の露出

```rust
// ❌ 禁止: 内部構造を公開
// tsql-lexer/src/lib.rs
pub struct Lexer {
    pub cursor: Cursor,  // 内部状態を公開
    pub tokens: Vec<Token>,
}
```

### 違反3: 他コンテキストのモデルを直接使用

```rust
// ❌ 禁止
// mysql-emitter/src/lib.rs
fn emit(tokens: Vec<tsql_lexer::Token>) -> String { }
```

---

## 8. 用語

| 用語 | 意味 |
|------|------|
| **侵入的結合** | 内部実装に直接依存 |
| **コントラクト結合** | 公開interfaceのみ依存 |
| **腐敗防止層（ACL）** | 外部変動から内部保護 |
| **変動性** | 変更の頻度 |

---

## 参考

- Vlad Khononov, "Balanced Coupling"
- Eric Evans, "Domain-Driven Design"
- Robert C. Martin, "Clean Architecture"
