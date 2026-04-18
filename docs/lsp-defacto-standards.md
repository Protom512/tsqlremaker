# DB Language Server デファクト標準

## 概要

既存の主要DB Language Server（sqls、MSSQL LS、PostgreSQL LS、MySQL LS）の機能を調査し、SAP ASE Language ServerのPhase 3以降の実装指針を策定する。

---

## 1. 主要DB Language Server機能比較表

| 機能カテゴリ | 機能 | sqls | MSSQL LS | PostgreSQL LS | MySQL LS | 優先度 |
|-------------|------|------|----------|---------------|----------|--------|
| **基本機能** | Diagnostics | ✅ | ✅ | ✅ | ✅ | Phase 1 |
| | Semantic Tokens | ✅ | ✅ | ✅ | ✅ | Phase 1 |
| | Document Symbols | ✅ | ✅ | ✅ | ✅ | Phase 1 |
| | Folding | ✅ | ✅ | ✅ | ✅ | Phase 1 |
| | Completion | ✅ | ✅ | ✅ | ✅ | Phase 1 |
| **ホバーと書式** | Hover | ✅ | ✅ | ✅ | ✅ | Phase 2 |
| | Formatting | ✅ | ✅ | ✅ | ✅ | Phase 2 |
| | Signature Help | ✅ | ✅ | ✅ | ❌ | Phase 2 |
| **ナビゲーション** | Go to Definition | ✅ | ✅ | ✅ | ✅ | **Phase 3** |
| | Find References | ✅ | ✅ | ✅ | ✅ | **Phase 3** |
| | Go to Type Definition | ❌ | ✅ | ✅ | ❌ | Phase 4 |
| | Implementation | ❌ | ❌ | ❌ | ❌ | - |
| **シンボル操作** | Workspace Symbols | ✅ | ✅ | ✅ | ✅ | **Phase 3** |
| | Rename | ✅ | ✅ | ✅ | ✅ | Phase 4 |
| | Code Actions (Quick Fix) | ✅ | ✅ | ✅ | ✅ | Phase 4 |
| **高度な機能** | Schema Explorer | ✅ | ✅ | ❌ | ❌ | Phase 5 |
| | Live Query Results | ❌ | ✅ | ❌ | ❌ | Phase 5 |
| | Query Execution | ❌ | ✅ | ❌ | ❌ | Phase 5 |
| | Database Connection | ❌ | ✅ | ❌ | ❌ | Phase 5 |

**凡例**: ✅ 実装済み, ❌ 未実装, Phase N 優先度レベル

### 補足事項

- **sqls**: 汎用SQL LS。複数DB対応だが、DB固有のデータ型は限定的。
- **MSSQL LS**: Microsoft公式。機能豊富だがWindows依存の部分あり。
- **PostgreSQL LS**: 拡張性が高い。型システムの参考に最適。
- **MySQL LS**: Oracle公式。機能はMSSQL LSに類似。

---

## 2. 実装優先度 (Phase 3以降)

### Phase 3: ナビゲーション基盤

#### 3.1 Symbol Table Builder

**目的**: ASTからシンボル情報を抽出し、高速に参照可能なデータ構造を構築する。

**理由**:
- Go to Definition、Find References、Workspace Symbols すべてがシンボルテーブルに依存
- 先に構築しておくことで、後続機能の実装が単純化
- パフォーマンスのボトルネックを早期に特定可能

**実装のポイント**:
```rust
// パーサーのAST走査時にシンボルテーブルを構築
pub struct SymbolTableBuilder {
    symbols: SymbolTable,
    current_scope: Vec<ScopeId>,
}
```

#### 3.2 Go to Definition

**目的**: カーソル位置の識別子の定義元にジャンプする。

**理由**:
- 最も基本的なナビゲーション機能
- ユーザーからの要望が最も高い
- 他のナビゲーション機能の基盤

**実装のポイント**:
- シンボルテーブルから識別子を検索
- 複数候補がある場合（例: 同名列のテーブルとカラム）は優先順位を付与
- 定義が別ファイルにある場合のハンドリング

#### 3.3 Find References

**目的**: 識別子のすべての使用箇所を検索する。

**理由**:
- リファクタリングの前段階として重要
- Code Lensと連携して参照数を表示可能

**実装のポイント**:
- シンボルテーブルの逆索引（reference map）を構築
- 定義元と参照箇所を区別して表示

#### 3.4 Workspace Symbols

**目的**: ワークスペース全体からシンボルを検索する。

**理由**:
- 大規模プロジェクトでの移動に必須
- ファイルをまたいだ定義検索が可能

**実装のポイント**:
- fuzzy matching をサポート
- シンボル種類（Table, Procedure, View等）でフィルタリング可能

---

### Phase 4: コード操作

#### 4.1 Code Actions (Quick Fix)

**目的**: 診断結果に対する修正提案を提供する。

**理由**:
- 開発効率向上に直結
- エラー学習機能としても機能

**実装例**:
- `UNKNOWN_TABLE` → "テーブルを作成"
- `MISSING_COLUMN` → "カラムを追加"
- `DEPRECATED_SYNTAX` → "新しい構文に置換"

#### 4.2 Rename

**目的**: シンボルの名前を変更し、すべての参照を更新する。

**理由**:
- リファクタリングの中核機能
- Find References があれば実装は比較的容易

**実装のポイント**:
- 影響範囲の正確な把握
- ローカル変数とグローバル変数の区別
- エディタ側でのプレビュー表示をサポート

---

### Phase 5: 統合機能

#### 5.1 Schema Explorer

**目的**: データベーススキーマをツリー表示し、オブジェクトを参照可能にする。

**理由**:
- データベース開発者の主要なニーズ
- オブジェクト間の依存関係を可視化

#### 5.2 Query Execution

**目的**: エディタから直接クエリを実行し、結果を表示する。

**理由**:
- 開発サイクルの短縮
- 他のツール（SQL Server Management Studio等）との競合

---

## 3. シンボルテーブル設計

### 3.1 全体構造

```rust
use std::collections::HashMap;
use tsql_parser::ast::{Span, Position, Identifier};
use tsql_parser::ast::ddl::{TableDefinition, ProcedureDefinition, ViewDefinition, IndexDefinition};

/// シンボルテーブル（トップレベル）
#[derive(Debug, Clone, Default)]
pub struct SymbolTable {
    /// すべてのテーブル
    pub tables: HashMap<String, TableSymbol>,
    
    /// すべてのストアドプロシージャ
    pub procedures: HashMap<String, ProcedureSymbol>,
    
    /// すべてのビュー
    pub views: HashMap<String, ViewSymbol>,
    
    /// すべてのインデックス
    pub indexes: HashMap<String, IndexSymbol>,
    
    /// スコープ階層（バッチ、プロシージャ、ブロック）
    pub scopes: Vec<Scope>,
    
    /// グローバル変数 (@@variable)
    pub global_variables: HashMap<String, VariableSymbol>,
    
    /// ファイルパスからシンボルへのマッピング（Workspace Symbols用）
    pub file_symbols: HashMap<String, Vec<SymbolRef>>,
}

/// シンボル参照（ファイル単位のインデックス用）
#[derive(Debug, Clone)]
pub struct SymbolRef {
    pub kind: SymbolKind,
    pub name: String,
    pub container_name: Option<String>,  // 例: テーブル名のカラム
    pub location: Location,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolKind {
    Table,
    View,
    Procedure,
    Function,
    Index,
    Column,
    Variable,
    Parameter,
}

/// スコープ（変数の可視性を管理）
#[derive(Debug, Clone)]
pub struct Scope {
    pub id: ScopeId,
    pub parent_id: Option<ScopeId>,
    pub kind: ScopeKind,
    pub variables: HashMap<String, VariableSymbol>,
    pub span: Span,
}

pub type ScopeId = usize;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScopeKind {
    Batch,       // GO で区切られるバッチ
    Procedure,   // プロシージャ本体
    Block,       // BEGIN...END ブロック
    Trigger,     // トリガー本体
}
```

### 3.2 テーブルシンボル

```rust
/// テーブルシンボル
#[derive(Debug, Clone)]
pub struct TableSymbol {
    /// テーブル名
    pub name: String,
    
    /// 定義位置（CREATE TABLE）
    pub definition_span: Span,
    
    /// カラム定義
    pub columns: Vec<ColumnSymbol>,
    
    /// テーブルオプション（LOCK MODE等）
    pub options: TableOptions,
    
    /// 一時テーブルかどうか
    pub is_temporary: bool,
    
    /// 一時テーブルの種類（ローカル # またはグローバル ##）
    pub temp_kind: Option<TempTableKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TempTableKind {
    Local,   // #table
    Global,  // ##table
}

/// テーブルオプション
#[derive(Debug, Clone, Default)]
pub struct TableOptions {
    /// LOCK DATAROWS / LOCK DATAPAGES
    pub lock_mode: Option<LockMode>,
    
    /// 他のSAP ASE固有オプション
    pub custom_options: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LockMode {
    DataPages,
    DataRows,
    AllPages,
}
```

### 3.3 カラムシンボル

```rust
/// カラムシンボル
#[derive(Debug, Clone)]
pub struct ColumnSymbol {
    /// カラム名
    pub name: String,
    
    /// データ型
    pub data_type: DataType,
    
    /// NULL 許容属性
    pub nullable: bool,
    
    /// デフォルト値
    pub default_value: Option<Expression>,
    
    /// IDENTITY属性
    pub is_identity: bool,
    
    /// 定義位置
    pub definition_span: Span,
    
    /// 属するテーブル名
    pub table_name: String,
}

/// データ型（SAP ASE拡張対応）
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DataType {
    /// 基本型
    Int,
    BigInt,
    SmallInt,
    TinyInt,
    Decimal { precision: u8, scale: u8 },
    Float,
    Real,
    
    /// 文字列型
    Varchar { max_length: Option<u32> },
    Char { length: u32 },
    Nvarchar { max_length: Option<u32> },
    Nchar { length: u32 },
    
    /// SAP ASE固有型
    Unichar { length: u32 },
    Univarchar { max_length: Option<u32> },
    BigDatetime,
    
    /// その他
    Text,
    Image,
    Binary,
    VarBinary { max_length: Option<u32> },
    
    /// 日時型
    DateTime,
    SmallDateTime,
    Date,
    Time,
}

/// 式（デフォルト値用）
#[derive(Debug, Clone)]
pub enum Expression {
    Literal(String),
    Variable(String),
    FunctionCall { name: String, args: Vec<Expression> },
}
```

### 3.4 プロシージャシンボル

```rust
/// ストアドプロシージャシンボル
#[derive(Debug, Clone)]
pub struct ProcedureSymbol {
    /// プロシージャ名
    pub name: String,
    
    /// 定義位置
    pub definition_span: Span,
    
    /// パラメータ
    pub parameters: Vec<ParameterSymbol>,
    
    /// 本体のローカル変数（DECLARE）
    pub local_variables: HashMap<String, VariableSymbol>,
    
    /// 戻り値の型（INT または void）
    pub return_type: Option<DataType>,
    
    /// スコープID
    pub scope_id: ScopeId,
}

/// パラメータシンボル
#[derive(Debug, Clone)]
pub struct ParameterSymbol {
    /// パラメータ名
    pub name: String,  // @param_name
    
    /// データ型
    pub data_type: DataType,
    
    /// 出力パラメータかどうか（OUTPUT）
    pub is_output: bool,
    
    /// デフォルト値
    pub default_value: Option<Expression>,
    
    /// 定義位置
    pub definition_span: Span,
}
```

### 3.5 ビューシンボル

```rust
/// ビューシンボル
#[derive(Debug, Clone)]
pub struct ViewSymbol {
    /// ビュー名
    pub name: String,
    
    /// 定義位置
    pub definition_span: Span,
    
    /// SELECT クエリ本体
    pub query: SelectStatement,
    
    /// 依存するテーブル
    pub dependencies: Vec<String>,
}
```

### 3.6 インデックスシンボル

```rust
/// インデックスシンボル
#[derive(Debug, Clone)]
pub struct IndexSymbol {
    /// インデックス名
    pub name: String,
    
    /// 定義位置
    pub definition_span: Span,
    
    /// 属するテーブル名
    pub table_name: String,
    
    /// インデックス対象カラム
    pub columns: Vec<IndexColumn>,
    
    /// 一意性制約
    pub is_unique: bool,
    
    /// クラスタ化インデックスかどうか
    pub is_clustered: bool,
}

/// インデックスカラム
#[derive(Debug, Clone)]
pub struct IndexColumn {
    pub name: String,
    pub sort_order: Option<SortOrder>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SortOrder {
    Ascending,
    Descending,
}
```

### 3.7 変数シンボル

```rust
/// 変数シンボル
#[derive(Debug, Clone)]
pub struct VariableSymbol {
    /// 変数名 (@local_var)
    pub name: String,
    
    /// データ型
    pub data_type: Option<DataType>,  // DECLARE なしで使用されると None
    
    /// 定義位置（DECLARE 文）
    pub definition_span: Option<Span>,
    
    /// 変数種類
    pub kind: VariableKind,
    
    /// 属するスコープ
    pub scope_id: ScopeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VariableKind {
    Local,    // @variable
    Global,   // @@variable
}

/// Location（LSP型へのマッピング用）
#[derive(Debug, Clone)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

#[derive(Debug, Clone)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}
```

---

## 4. メタデータキャッシュ戦略

### 4.1 既存LSのアプローチ

| LS | 戦略 | TTL | 無効化条件 |
|----|------|-----|-----------|
| sqls | Lazy Load + In-Memory | なし | ファイル保存時 |
| MSSQL LS | Lazy Load + DB接続 | 5分 | 手動更新、DB変更 |
| PostgreSQL LS | Lazy Load + Schema Cache | なし | DDL実行時 |
| MySQL LS | Eager Load + In-Memory | なし | ファイル保存時 |

### 4.2 推奨戦略（ASE LS）

#### Lazy Load + On-Demand Invalidation

```rust
pub struct MetadataCache {
    /// シンボルテーブル（ファイルごと）
    file_tables: HashMap<String, FileMetadata>,
    
    /// 最終更新時刻
    last_updated: HashMap<String, SystemTime>,
    
    /// キャッシュポリシー
    policy: CachePolicy,
}

#[derive(Debug, Clone)]
pub struct CachePolicy {
    /// 自再解析時間（Noneで無効化）
    ttl: Option<Duration>,
    
    /// ファイル監視有無
    watch_files: bool,
}

impl MetadataCache {
    /// ファイルのメタデータを取得（キャッシュがあれば再利用）
    pub fn get(&mut self, uri: &str) -> Option<&FileMetadata> {
        if self.is_valid(uri) {
            self.file_tables.get(uri)
        } else {
            None  // キャッシュ無効
        }
    }
    
    /// キャッシュの有効性チェック
    fn is_valid(&self, uri: &str) -> bool {
        if let Some(updated) = self.last_updated.get(uri) {
            if let Some(ttl) = self.policy.ttl {
                return updated.elapsed().ok() < Some(ttl);
            }
            true  // TTLなしで有効
        } else {
            false
        }
    }
    
    /// DDL実行後にキャッシュを無効化
    pub fn invalidate(&mut self, uri: &str) {
        self.file_tables.remove(uri);
        self.last_updated.remove(uri);
    }
}
```

#### 無効化トリガー

1. **ファイル保存時**: 該当ファイルのみ再構築
2. **DDL実行時**: CREATE/ALTER/DROP が含まれるバッチ実行後
3. **手動更新**: コマンドパレットから "ASE LS: Rebuild Cache"

#### メモリ効率

- 大規模ファイル（1000+行）の場合、シンボルのみを保持
- AST全文は不要な時点で破棄
- 間引き（throttling）で頻繁な変更に対処

---

## 5. 推奨アーキテクチャ（ASE LS）

### 5.1 モジュール構成

```
crates/ase-ls-core/src/
├── lib.rs              (公開API)
├── server.rs           (tower-lsp Server実装、既存)
├── handlers/
│   ├── diagnostics.rs      (既存)
│   ├── completion.rs       (既存)
│   ├── hover.rs            (既存)
│   ├── formatting.rs       (既存)
│   ├── signature_help.rs   (既存)
│   ├── definition.rs       (Phase 3: 新規)
│   ├── references.rs       (Phase 3: 新規)
│   ├── workspace_symbols.rs(Phase 3: 新規)
│   ├── code_actions.rs     (Phase 4: 新規)
│   └── rename.rs           (Phase 4: 新規)
├── symbols/
│   ├── mod.rs
│   ├── builder.rs          (シンボルテーブル構築)
│   ├── table.rs            (TableSymbol)
│   ├── procedure.rs        (ProcedureSymbol)
│   ├── variable.rs         (VariableSymbol)
│   └── cache.rs            (メタデータキャッシュ)
└── ast_utils/
    ├── visitor.rs          (AST Visitorパターン)
    └── scope.rs            (スコープ解析)
```

### 5.2 既存ライブラリの活用

```rust
// Cargo.toml (既存設定)
[dependencies]
tower-lsp = "0.20"
lsp-types = "0.94"
tsql-parser = { path = "../tsql-parser" }
tsql-lexer = { path = "../tsql-lexer" }

// Phase 3 追加依存
dashmap = "5.5"           // 並行安全なHashMap
ropf = "0.3"              // 高速ハッシュ関数（オプション）
```

### 5.3 定義ジャンプの実装例

```rust
// handlers/definition.rs
use tower_lsp::lsp_types::*;
use crate::symbols::{SymbolTable, SymbolKind};

pub async fn goto_definition(
    params: GotoDefinitionParams,
    symbols: &SymbolTable,
) -> Option<GotoDefinitionResponse> {
    let uri = params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    
    // 1. トークン位置から識別子を取得
    let identifier = get_identifier_at_position(&uri, pos)?;
    
    // 2. シンボルテーブルから定義を検索
    let symbol = symbols.resolve_symbol(&identifier.name, pos)?;
    
    // 3. Location を構築
    Some(GotoDefinitionResponse::Scalar(Location {
        uri: symbol.definition_uri,
        range: symbol.definition_span.to_range(),
    }))
}
```

---

## 6. SAP ASE固有の考慮事項

### 6.1 固有データ型

| 型 | 用途 | MySQL対応 |
|----|------|-----------|
| `UNICHAR(n)` | Unicode固定長文字列 | `CHAR(n) CHARACTER SET utf8mb4` |
| `UNIVARCHAR(n)` | Unicode可変長文字列 | `VARCHAR(n) CHARACTER SET utf8mb4` |
| `BIGDATETIME` | 高精度日時 | `DATETIME(6)` |
| `TIMESTAMP` | 特殊型（行バージョン） | `TIMESTAMP` だが意味が異なる |

**実装上の注意**:
- Signature Help で型名を補完
- Hover で「この型はMySQLでは X に対応」のような注釈

### 6.2 ロックモード

```sql
-- SAP ASE
CREATE TABLE t (
    id INT
) LOCK DATAROWS

-- MySQL には直接相当する構文なし
-- インデックス設計で代替
```

**Code Action提案**:
- `LOCK DATAROWS` → "MySQLでは InnoDB が行ロックを提供"

### 6.3 グローバル変数

| 変数 | 用途 | 対応 |
|------|------|------|
| `@@transtate` | トランザクション状態 | MySQLには直接相当なし |
| `@@error` | 直近のエラーコード | `GET_DIAGNOSTICS` で代替 |
| `@@rowcount` | 影響行数 | `ROW_COUNT()` に対応 |
| `@@identity` | 直近のIDENTITY値 | `LAST_INSERT_ID()` に対応 |

**実装上の注意**:
- シンボルテーブルで `@@variable` を特別扱い
- Hover で「MySQLでは: X」を表示

### 6.4 一時テーブル

```sql
-- SAP ASE
CREATE TABLE #temp (id INT)       -- ローカル一時テーブル
CREATE TABLE ##global (id INT)    -- グローバル一時テーブル

-- MySQL
CREATE TEMPORARY TABLE temp (id INT)  -- セッション固有
```

**実装上の注意**:
- `TempTableKind::Local` と `::Global` を区別
- Document Symbols では特別なアイコンで表示

### 6.5 バッチ区切り（GO）

```sql
-- SAP ASE
CREATE TABLE t (id INT)
GO
INSERT INTO t VALUES (1)
GO

-- MySQL には GO なし（セミコロンのみ）
```

**実装上の注意**:
- `ScopeKind::Batch` を `GO` で区切る
- バッチをまたぐ変数参照はエラー

---

## 7. Phase 3 実装ロードマップ

### 3.1 Symbol Table Builder (1週間)

- [ ] `SymbolTable` 構造体の実装
- [ ] `SymbolTableBuilder` の実装
- [ ] AST Visitor パターンの実装
- [ ] 基本単体テスト

### 3.2 Go to Definition (3日)

- [ ] `handlers/definition.rs` の実装
- [ ] シンボル解決ロジック
- [ ] 複数候補の扱い
- [ ] テスト

### 3.3 Find References (3日)

- [ ] `handlers/references.rs` の実装
- [ ] 逆索引（reference map）の構築
- [ ] 定義元と参照の区別
- [ ] テスト

### 3.4 Workspace Symbols (2日)

- [ ] `handlers/workspace_symbols.rs` の実装
- [ ] fuzzy matching の実装
- [ ] シンボル種類フィルタ
- [ ] テスト

### 3.5 統合とテスト (2日)

- [ ] パフォーマンス測定
- [ ] 大規模ファイルでのテスト
- [ ] メモリ効率の確認
- [ ] ドキュメント更新

---

## 8. 参考文献

- [LSP Specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)
- [sqls GitHub](https://github.com/lighttiger2505/sqls)
- [MSSQL LS GitHub](https://github.com/microsoft/azuredatastudio)
- [PostgreSQL LS GitHub](https://github.com/jason-wilkins/vscode-postgresql)
- [MySQL LS GitHub](https://github.com/mysql/mysql-tools-for-vs-code)
- [Rust Analyzer Source Code](https://github.com/rust-analyzer/rust-analyzer) (シンボルテーブル設計の参考)
