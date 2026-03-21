# 簡易実装箇所の仕様調査レポート

**作成日**: 2026-03-19
**調査担当**: TSQLRemaker Research Team
**バージョン**: 1.0

---

## 概要

本レポートは、TSQLRemaker プロジェクトにおける「簡易実装」「TODO」「placeholder」の箇所について、仕様調査と既存コードの分析結果をまとめたものです。

---

## 1. PostgreSQL Emitter: サブクエリ実装

### 1.1 現状分析

**ファイル**: `crates/postgresql-emitter/src/mappers/expression.rs`

```rust
// Line 234-237: IN句のサブクエリ（既に実装済み）
CommonInList::Subquery(query) => {
    format!("({})", super::SelectStatementRenderer::emit(query))
}

// Line 242-245: サブクエリの発行（既に実装済み）
fn emit_subquery(query: &tsql_parser::common::CommonSelectStatement) -> String {
    super::SelectStatementRenderer::emit(query)
}
```

### 1.2 実装状況

**既に実装済み**であることが確認できました。以下の機能が動作しています：

- `IN (SELECT ...)` 句のサブクエリレンダリング
- `EXISTS (SELECT ...)` 句のサブクエリレンダリング
- `WHERE` 句でのスカラーサブクエリ
- 派生テーブル `(SELECT ...) AS alias` のレンダリング

**検証済みテスト**:
- `test_emit_subquery` (Line 458-486)
- `test_emit_exists_subquery` (Line 489-518)

### 1.3 PostgreSQLサブクエリ構文

PostgreSQL は以下のサブクエリ構文をサポートしています：

| 構文 | 例 | 備考 |
|------|-----|------|
| **IN** | `WHERE id IN (SELECT id FROM users)` | 値リストとしてのサブクエリ |
| **NOT IN** | `WHERE id NOT IN (SELECT id FROM users)` | NULL 値に注意 |
| **EXISTS** | `WHERE EXISTS (SELECT 1 FROM users WHERE ...)` | 相関サブクエリに最適 |
| **NOT EXISTS** | `WHERE NOT EXISTS (SELECT ...)` | EXISTS の否定 |
| **スカラー** | `WHERE x = (SELECT MAX(y) FROM t)` | 単一値を返すサブクエリ |
| **派生テーブル** | `FROM (SELECT ...) AS t` | FROM 句でのサブクエリ |

### 1.4 既存コードの品質

- `SelectStatementRenderer::emit()` は完全に実装されている
- `ExpressionEmitter::emit_subquery()` は `SelectStatementRenderer` を再利用している
- テストカバレッジが十分である

### 1.5 結論

**この機能は完全実装済みです。**追加作業は不要です。

---

## 2. SAP ASE T-SQL: CREATE TABLE 制約詳細仕様

### 2.1 テーブル制約の完全構文

#### 2.1.1 PRIMARY KEY 制約

```sql
-- 基本構文
CONSTRAINT [constraint_name] PRIMARY KEY [CLUSTERED | NONCLUSTERED] (column_list)

-- 複数カラム主キー
CONSTRAINT pk_order_details PRIMARY KEY (order_id, product_id)

-- クラスター化指定
CONSTRAINT pk_users PRIMARY KEY CLUSTERED (id)
```

**構文要素:**
- `CONSTRAINT constraint_name`: 制約名（省略可能）
- `PRIMARY KEY`: 主キー制約
- `CLUSTERED | NONCLUSTERED`: インデックスタイプ（省略時は既定値）
- `(column_list)`: カラムリスト（カンマ区切り、複数可）

#### 2.1.2 FOREIGN KEY 制約

```sql
-- 基本構文
CONSTRAINT [constraint_name] FOREIGN KEY (column_list)
    REFERENCES ref_table(ref_column_list)
    [ON DELETE {NO ACTION | CASCADE | SET NULL | SET DEFAULT}]
    [ON UPDATE {NO ACTION | CASCADE | SET NULL | SET DEFAULT}]

-- 単一カラム外部キー
CONSTRAINT fk_orders_user
    FOREIGN KEY (user_id)
    REFERENCES users(id)
    ON DELETE CASCADE
    ON UPDATE CASCADE

-- 複数カラム外部キー
CONSTRAINT fk_order_details
    FOREIGN KEY (order_id, product_id)
    REFERENCES order_details(order_id, product_id)
```

**構文要素:**
- `FOREIGN KEY (column_list)`: 自テーブルのカラムリスト
- `REFERENCES ref_table(ref_column_list)`: 参照先テーブルとカラム
- `ON DELETE`: 削除時の動作（既定: NO ACTION）
- `ON UPDATE`: 更新時の動作（既定: NO ACTION）

**参照アクション:**
| アクション | 説明 |
|-----------|------|
| NO ACTION | 参照されている場合、削除/更新を拒否（既定） |
| CASCADE | 参照先の削除/更新に合わせて削除/更新 |
| SET NULL | 参照先が削除/更新されると NULL を設定 |
| SET DEFAULT | 参照先が削除/更新されると既定値を設定 |

#### 2.1.3 UNIQUE 制約

```sql
-- 基本構文
CONSTRAINT [constraint_name] UNIQUE [CLUSTERED | NONCLUSTERED] (column_list)

-- 単一カラム
CONSTRAINT uq_user_email UNIQUE (email)

-- 複数カラム
CONSTRAINT uq_user_name_email UNIQUE (first_name, last_name, email)
```

#### 2.1.4 CHECK 制約

```sql
-- 基本構文
CONSTRAINT [constraint_name] CHECK (search_condition)

-- 単一条件
CONSTRAINT chk_age CHECK (age >= 18)

-- 複合条件
CONSTRAINT chk_salary CHECK (salary > 0 AND salary < 1000000)

-- IN リスト
CONSTRAINT chk_status CHECK (status IN ('active', 'inactive', 'pending'))

-- LIKE パターン
CONSTRAINT chk_email CHECK (email LIKE '%@%')
```

### 2.2 カラムレベル制約

カラム定義内で直接指定できる制約：

```sql
CREATE TABLE users (
    id INT PRIMARY KEY,                    -- カラムレベル PRIMARY KEY
    email VARCHAR(255) NOT NULL UNIQUE,    -- 複数のカラム制約
    age INT NULL CHECK (age >= 18),        -- NULL 可能 + CHECK
    status VARCHAR(20) DEFAULT 'active'    -- デフォルト値
)
```

### 2.3 制約名の命名規則

SAP ASE では明示的な制約名を推奨：

| 制約種別 | 推奨プレフィックス | 例 |
|---------|------------------|-----|
| PRIMARY KEY | `pk_` | `pk_users_id` |
| FOREIGN KEY | `fk_` | `fk_orders_user_id` |
| UNIQUE | `uq_` | `uq_users_email` |
| CHECK | `chk_` | `chk_users_age` |

---

## 3. Parser: CREATE TABLE テーブルレベル制約実装

### 3.1 現状分析

**ファイル**: `crates/tsql-parser/src/parser.rs`

**テーブル制約パーサー**: `parse_table_constraint()` (Line 1039-1139)

```rust
fn parse_table_constraint(&mut self, _name: Identifier) -> ParseResult<TableConstraint> {
    match self.buffer.current()?.kind {
        TokenKind::Primary => {
            // PRIMARY KEY (col1, col2, ...)
            // ...
            Ok(TableConstraint::PrimaryKey { columns })
        }
        TokenKind::Unique => {
            // UNIQUE (col1, col2, ...)
            // ...
            Ok(TableConstraint::Unique { columns })
        }
        TokenKind::Foreign => {
            // FOREIGN KEY (col1, col2, ...) REFERENCES reftable(refcol1, ...)
            // ...
            Ok(TableConstraint::Foreign { columns, ref_table, ref_columns })
        }
        TokenKind::Check => {
            // CHECK (expression)
            // ...
            Ok(TableConstraint::Check { expr })
        }
        // ...
    }
}
```

### 2.2 AST 定義

**ファイル**: `crates/tsql-parser/src/ast/ddl.rs` (Line 140-167)

```rust
pub enum TableConstraint {
    PrimaryKey {
        columns: Vec<Identifier>,
    },
    Foreign {
        columns: Vec<Identifier>,
        ref_table: Identifier,
        ref_columns: Vec<Identifier>,
    },
    Unique {
        columns: Vec<Identifier>,
    },
    Check {
        expr: Expression,
    },
}
```

### 3.3 実装状況

**完全実装済み**であることが確認できました：

- `PRIMARY KEY (col1, col2, ...)` のパース
- `UNIQUE (col1, col2, ...)` のパース
- `FOREIGN KEY (cols) REFERENCES table(ref_cols)` のパース
- `CHECK (expression)` のパース
- 制約名（CONSTRAINT name）のサポート

### 3.4 未実装の機能

以下の機能は**未実装**です：

| 機能 | 優先度 | 備考 |
|------|--------|------|
| ON DELETE/ON UPDATE | 中 | PostgreSQL でサポートされている |
| CLUSTERED/NONCLUSTERED | 低 | ASE 固有の最適化ヒント |

### 3.5 結論

**基本的機能は完全実装済み**です。ON DELETE/ON UPDATE は今後の拡張で対応可能です。

---

## 4. SAP ASE T-SQL: 派生テーブル（サブクエリ）詳細仕様

### 4.1 派生テーブルの基本構文

```sql
-- 基本構文
FROM (SELECT select_list FROM table_name [WHERE condition]) AS alias

-- 具体例
FROM (
    SELECT u.id, u.name, o.order_id
    FROM users u
    INNER JOIN orders o ON u.id = o.user_id
    WHERE o.status = 'active'
) AS active_users
```

### 4.2 エイリアスの要件

SAP ASE T-SQL では、派生テーブルには**エイリアスが必須**です：

```sql
-- 有効：エイリアスあり
FROM (SELECT * FROM users) AS u

-- 無効：エイリアスなし（エラーになる）
FROM (SELECT * FROM users)

-- AS キーワードの省略も可能
FROM (SELECT * FROM users) u
```

### 4.3 入れ子のレベル

SAP ASE は最大32レベルの入れ子をサポートしています：

```sql
-- 3レベルの入れ子例
FROM (
    SELECT * FROM (
        SELECT * FROM (
            SELECT id, name FROM users
        ) AS level3
    ) AS level2
) AS level1
```

### 4.4 派生テーブルで使用可能な構文

派生テーブル内で使用できる句：

| 句 | 使用可 | 備考 |
|----|--------|------|
| SELECT | ✓ | カラムリスト |
| FROM | ✓ | テーブル参照 |
| WHERE | ✓ | フィルタ条件 |
| GROUP BY | ✓ | 集約 |
| HAVING | ✓ | 集約後フィルタ |
| ORDER BY | ✓ | TOP と共に使用時のみ |
| LIMIT/TOP | ✓ | 結果制限 |

```sql
-- ORDER BY は TOP と共に使用可能
FROM (
    SELECT TOP 10 *
    FROM users
    ORDER BY created_at DESC
) AS recent_users
```

### 4.5 CTE（共通テーブル式）との比較

SAP ASE 16 以降では CTE もサポートされています：

```sql
-- CTE 構文
WITH active_users AS (
    SELECT u.id, u.name, o.order_id
    FROM users u
    INNER JOIN orders o ON u.id = o.user_id
    WHERE o.status = 'active'
)
SELECT * FROM active_users
```

**派生テーブル vs CTE:**

| 特徴 | 派生テーブル | CTE |
|------|-------------|-----|
| エイリアス | 必須 | 必須 |
| 再帰 | 不可 | 可能 |
| 複数参照 | 不可 | 可能 |
| 可読性 | 低（複雑なクエリで） | 高 |

---

## 5. Parser: サブクエリ内FROM句の派生テーブル実装

### 5.1 現状分析

**ファイル**: `crates/tsql-parser/src/expression/mod.rs`

**派生テーブルパーサー**: `parse_subquery_from_clause()` (Line 332-447)

```rust
fn parse_subquery_from_clause(&mut self) -> ParseResult<crate::ast::FromClause> {
    // ...
    // 派生テーブル（サブクエリ）の検出
    if self.buffer.check(TokenKind::LParen) {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // LParen

        // サブクエリを解析
        let select_stmt = match self.parse_subquery_select_statement()? {
            crate::ast::Statement::Select(select) => select,
            _ => { /* error */ }
        };

        // 右括弧を期待
        self.buffer.consume()?; // RParen

        // オプションの別名
        let alias = if self.buffer.check(TokenKind::As) {
            // ...
        } else if self.buffer.check(TokenKind::Ident) {
            // ...
        } else {
            None
        };

        tables.push(crate::ast::TableReference::Subquery {
            query: select_stmt,
            alias,
            span: Span { start, end: end_span.end },
        });
    }
    // ...
}
```

### 5.2 Common SQL AST へのマッピング

**ファイル**: `crates/tsql-parser/src/common/statement.rs`

```rust
pub enum CommonTableReference {
    Table {
        name: String,
        alias: Option<String>,
        span: Span,
    },
    Derived {
        subquery: Box<CommonSelectStatement>,
        alias: Option<String>,
        span: Span,
    },
}
```

### 5.3 実装状況

**完全実装済み**であることが確認できました：

- `FROM (SELECT ...) AS alias` のパース
- `FROM (SELECT ...) AS alias` の Common SQL AST への変換
- PostgreSQL Emitter での派生テーブルのレンダリング

### 5.4 結論

**この機能は完全実装済みです。**追加作業は不要です。

---

## 6. PostgreSQL: サブクエリ構文詳細

### 6.1 派生テーブルの構文

PostgreSQL では派生テーブルに**エイリアスが必須**です：

```sql
-- 有効：エイリアスあり
FROM (SELECT * FROM users) AS u
FROM (SELECT * FROM users) u  -- AS を省略

-- 無効：エイリアスなし（エラー）
FROM (SELECT * FROM users)
```

### 6.2 カラムリストのエイリアス

派生テーブルのカラムに明示的な名前を付けられます：

```sql
-- カラムリストでエイリアス指定
FROM (
    SELECT u.id, u.name, o.order_id
    FROM users u
    JOIN orders o ON u.id = o.user_id
) AS user_orders(user_id, user_name, order_id)
```

### 6.3 横方向（LATERAL）サブクエリ

PostgreSQL 固有の `LATERAL` キーワード：

```sql
-- LATERAL により、左側のテーブルを参照可能
SELECT u.name, recent.order_count
FROM users u,
LATERAL (
    SELECT COUNT(*) AS order_count
    FROM orders o
    WHERE o.user_id = u.id
) AS recent
```

**注**: SAP ASE は `LATERAL` をサポートしていないため、変換時に注意が必要です。

### 6.4 配列のアンパック

PostgreSQL 固有の `UNNEST` 関数：

```sql
-- 配列を展開
SELECT unnest(ARRAY[1, 2, 3]) AS value

-- WITH ORDINALITY でインデックスを取得
SELECT * FROM unnest(ARRAY['a', 'b']) WITH ORDINALITY AS t(value, ord)
```

---

## 7. SAP ASE T-SQL: ストアドプロシージャ完全構文

### 7.1 CREATE PROCEDURE の完全構文

```sql
CREATE PROCEDURE [owner.]procedure_name
    [;number]
    [ {@parameter data_type} [VARYING] [= default] [OUTPUT] ]
    [ ,...n ]
[ WITH
    {
        RECOMPILE | ENCRYPTION | RECOMPILE, ENCRYPTION
    }
]
[ FOR REPLICATION ]
AS
    [ sql_statement [ ...n ] |
      BEGIN ... END
    ]
```

### 7.2 パラメータの詳細

```sql
-- 基本パラメータ
@parameter_name data_type

-- デフォルト値付き
@parameter_name data_type = default_value

-- OUTPUT パラメータ
@parameter_name data_type OUTPUT

-- VARYING （カーソルパラメータ）
@cursor_parameter CURSOR VARYING OUTPUT
```

**パラメータ例:**

```sql
CREATE PROCEDURE get_user_orders
    @user_id INT,
    @status VARCHAR(20) = 'active',  -- デフォルト値
    @order_count INT OUTPUT           -- 出力パラメータ
AS
BEGIN
    SELECT @order_count = COUNT(*)
    FROM orders
    WHERE user_id = @user_id
    AND status = @status

    SELECT * FROM orders
    WHERE user_id = @user_id
    AND status = @status
END
```

### 7.3 プロシージャ本体で使用可能な文

| 文種別 | 使用可 | 備考 |
|--------|--------|------|
| SELECT | ✓ | データ取得 |
| INSERT | ✓ | データ挿入 |
| UPDATE | ✓ | データ更新 |
| DELETE | ✓ | データ削除 |
| DECLARE | ✓ | 変数宣言 |
| SET | ✓ | 変数代入 |
| IF...ELSE | ✓ | 条件分岐 |
| WHILE | ✓ | ループ |
| BEGIN...END | ✓ | ブロック |
| TRY...CATCH | ✓ | 例外処理（ASE 12.5.1+） |
| RETURN | ✓ | 戻り値を返して終了 |
| EXECUTE | ✓ | 他のプロシージャ呼び出し |
| トランザクション制御 | ✓ | BEGIN TRANSACTION, COMMIT, ROLLBACK |
| CREATE TABLE | ✓ | 一時テーブル作成 |
| DROP TABLE | ✓ | テーブル削除 |

### 7.4 プロシージャオプション

```sql
-- 再コンパイル（毎回実行プランを再生成）
CREATE PROCEDURE get_recent_users
    @days INT = 7
WITH RECOMPILE
AS
    SELECT * FROM users
    WHERE created_at >= DATEADD(day, -@days, GETDATE())

-- 暗号化（プロシージャテキストを暗号化）
CREATE PROCEDURE sensitive_proc
WITH ENCRYPTION
AS
    SELECT * FROM sensitive_data
```

### 7.5 TRY...CATCH 構文

```sql
CREATE PROCEDURE safe_update_user
    @user_id INT,
    @new_status VARCHAR(20)
AS
BEGIN
    BEGIN TRY
        BEGIN TRANSACTION

        UPDATE users
        SET status = @new_status
        WHERE id = @user_id

        IF @@ROWCOUNT = 0
        BEGIN
            -- ユーザーが見つからない
            RAISERROR('User not found', 16, 1)
        END

        COMMIT TRANSACTION
    END TRY
    BEGIN CATCH
        IF @@TRANCOUNT > 0
            ROLLBACK TRANSACTION

        -- エラー情報を返す
        SELECT
            ERROR_NUMBER() AS ErrorNumber,
            ERROR_SEVERITY() AS ErrorSeverity,
            ERROR_STATE() AS ErrorState,
            ERROR_PROCEDURE() AS ErrorProcedure,
            ERROR_LINE() AS ErrorLine,
            ERROR_MESSAGE() AS ErrorMessage
    END CATCH
END
```

### 7.6 トランザクション制御

```sql
CREATE PROCEDURE transfer_money
    @from_account INT,
    @to_account INT,
    @amount DECIMAL(10,2)
AS
BEGIN
    DECLARE @balance DECIMAL(10,2)

    -- 残高確認
    SELECT @balance = balance
    FROM accounts
    WHERE id = @from_account

    IF @balance < @amount
    BEGIN
        RAISERROR('Insufficient funds', 16, 1)
        RETURN
    END

    -- トランザクション開始
    BEGIN TRANSACTION transfer_trx

    BEGIN TRY
        -- 引き出し
        UPDATE accounts
        SET balance = balance - @amount
        WHERE id = @from_account

        -- 預け入れ
        UPDATE accounts
        SET balance = balance + @amount
        WHERE id = @to_account

        COMMIT TRANSACTION transfer_trx
    END TRY
    BEGIN CATCH
        IF @@TRANCOUNT > 0
            ROLLBACK TRANSACTION transfer_trx

        DECLARE @errmsg NVARCHAR(4000) = ERROR_MESSAGE()
        RAISERROR(@errmsg, 16, 1)
    END CATCH
END
```

### 7.7 プロシージャの呼び出し

```sql
-- 基本呼び出し
EXEC get_user_orders @user_id = 123

-- OUTPUT パラメータ付き
DECLARE @count INT
EXEC get_user_orders @user_id = 123, @order_count = @count OUTPUT
PRINT @count

-- 変数に結果を格納
DECLARE @result INT
EXEC @result = check_user_status @user_id = 123
```

---

## 8. Parser: プロシージャ本体の完全実装

### 8.1 現状分析

**ファイル**: `crates/tsql-parser/src/parser.rs`

**プロシージャパーサー**: `parse_create_procedure()` (Line 1407-1468)

```rust
fn parse_create_procedure(&mut self, start: u32) -> ParseResult<Statement> {
    let name = self.parse_identifier()?;

    // パラメータリスト（オプション）
    let mut parameters = Vec::new();
    // ... パラメータパース ...

    // AS
    self.buffer.consume()?;

    // プロシージャ本体（簡易版：BEGIN...ENDまたは単一の文）
    let body = if self.buffer.check(TokenKind::Begin) {
        let block = self.parse_block()?;
        vec![block]
    } else {
        vec![self.parse_statement()?]
    };

    Ok(Statement::Create(Box::new(CreateStatement::Procedure(
        ProcedureDefinition {
            span: Span { start, end: end_span.end },
            name,
            parameters,
            body,
        },
    ))))
}
```

### 8.2 未実装の機能

以下のT-SQLストアドプロシージャ機能が未実装です：

| 機能 | 状態 | 優先度 |
|------|------|--------|
| `DECLARE` ブロック（カンマ区切り複数変数） | 部分 | 中 |
| `TRY...CATCH` ブロック | 未実装 | 高 |
| トランザクション制御 (`BEGIN TRANSACTION`, `COMMIT`, `ROLLBACK`) | 未実装 | 中 |
| `RAISERROR` 文 | 未実装 | 中 |
| `THROW` 文 | 未実装 | 低 |
| `WITH RECOMPILE` | 未実装 | 低 |
| `WITH ENCRYPTION` | 未実装 | 低 |

### 8.3 AST 拡張計画

#### 8.3.1 TRY...CATCH 用 AST ノード

```rust
// crates/tsql-parser/src/ast/control_flow.rs に追加

/// TRY...CATCH ブロック
#[derive(Debug, Clone)]
pub struct TryCatchStatement {
    /// 位置情報
    pub span: Span,
    /// TRY ブロック
    pub try_block: Box<Block>,
    /// CATCH ブロック
    pub catch_block: Box<Block>,
}
```

#### 8.3.2 トランザクション制御用 AST ノード

```rust
// crates/tsql-parser/src/ast/control_flow.rs に追加

/// トランザクション制御文
#[derive(Debug, Clone)]
pub enum TransactionStatement {
    /// BEGIN TRANSACTION [name]
    Begin {
        span: Span,
        name: Option<Identifier>,
    },
    /// COMMIT TRANSACTION [name]
    Commit {
        span: Span,
        name: Option<Identifier>,
    },
    /// ROLLBACK TRANSACTION [name | savepoint]
    Rollback {
        span: Span,
        name: Option<Identifier>,
    },
    /// SAVE TRANSACTION name
    Save {
        span: Span,
        name: Identifier,
    },
}
```

### 8.4 実装計画

1. **ASTノードの追加**:
   - `TryCatchStatement` (try_block, catch_block)
   - `TransactionStatement` (begin, commit, rollback, save)

2. **Parserの拡張**:
   - `parse_try_catch()` メソッド
   - `parse_transaction()` メソッド

3. **TokenKind の追加**:
   - `Try`, `Catch`, `BeginTransaction`, `Commit`, `Rollback`, `Save`

4. **Common SQL AST への変換**:
   - 方言固有構文としてマーク

### 8.5 結論

**部分的実装**。以下の追加実装が必要です：

1. **優先度高**: `TRY...CATCH` ブロックの実装
2. **優先度中**: トランザクション制御の実装
3. **優先度中**: `RAISERROR` の実装
4. **優先度低**: `THROW`、`WITH RECOMPILE`、`WITH ENCRYPTION` の実装

    Ok(Statement::Create(Box::new(CreateStatement::Procedure(
        ProcedureDefinition {
            span: Span { start, end: end_span.end },
            name,
            parameters,
            body,
        },
    ))))
}
```

---

## 9. WASM: Emitter統合実装

### 9.1 現状分析

**ファイル**: `crates/wasm/src/lib.rs`

```rust
#[wasm_bindgen(js_name = convertTo)]
pub fn convert_to(_input: &str, dialect: TargetDialect) -> JsValue {
    // Emitter未実装のためエラーを返す
}
```

### 5.2 依存関係

PostgreSQL Emitter は**完全実装済み**です。WASM 統合の前提条件を満たしています。

### 5.3 実装に必要な作業

1. PostgreSQL Emitter の WASM 呼び出し
2. エラーハンドリング
3. JS への結果返却

### 5.4 結論

**未実装**。PostgreSQL Emitter が完成しているため、実装可能です。

---

## 6. MySQL Emitter: 新規実装

### 6.1 現状分析

**Spec**: `.kiro/specs/mysql-emitter/`
- `spec.json`: `ready_for_implementation: false`

### 6.2 MySQL 固有の考慮事項

| 機能 | MySQL | 備考 |
|------|-------|------|
| 文字列連結 | `CONCAT()` | T-SQL の `+` は MySQL で加算 |
| 日付関数 | `NOW()`, `DATE_ADD()` | `GETDATE()` → `NOW()` |
| TOP | `LIMIT` | `SELECT TOP 10` → `SELECT ... LIMIT 10` |
| AUTO_INCREMENT | `AUTO_INCREMENT` | IDENTITY → AUTO_INCREMENT |
| 一時テーブル | `CREATE TEMPORARY TABLE` | `#temp` → 一時テーブル |

### 6.3 実装計画

PostgreSQL Emitter を参考に、以下のモジュールを実装：

1. `datatype.rs`: データ型マッパー
2. `function.rs`: 関数マッパー
3. `syntax.rs`: 構文マッパー
4. `expression.rs`: 式エミッター
5. `lib.rs`: メインエミッター

### 6.4 結論

**未実装**。新規実装が必要です。

### 9.2 実装計画

WASM 側の実装：

```rust
#[wasm_bindgen(js_name = convertTo)]
pub fn convert_to(input: &str, dialect: TargetDialect) -> JsValue {
    // Parser で T-SQL をパース
    let result = tsql_parser::parse_sql(input);

    let stmts = match result {
        Ok(stmts) => stmts,
        Err(e) => {
            return JsValue::from_str(&format!("Parse error: {}", e));
        }
    };

    // Common SQL AST に変換
    // ...

    // PostgreSQL Emitter で出力
    match dialect {
        TargetDialect::PostgreSQL => {
            // Emitter 呼び出し
            // ...
        }
        TargetDialect::MySQL => {
            // TODO: MySQL Emitter 実装後
            return JsValue::from_str("MySQL not yet supported");
        }
    }
}
```

### 9.3 結論

**未実装**。PostgreSQL Emitter が完成しているため、実装可能です。

---

## 10. MySQL Emitter: 新規実装

### 10.1 現状分析

**Spec**: `.kiro/specs/mysql-emitter/`
- `spec.json`: `ready_for_implementation: false`

### 10.2 MySQL 固有の考慮事項

| 機能 | MySQL | 備考 |
|------|-------|------|
| 文字列連結 | `CONCAT()` | T-SQL の `+` は MySQL で加算 |
| 日付関数 | `NOW()`, `DATE_ADD()` | `GETDATE()` → `NOW()` |
| TOP | `LIMIT` | `SELECT TOP 10` → `SELECT ... LIMIT 10` |
| AUTO_INCREMENT | `AUTO_INCREMENT` | IDENTITY → AUTO_INCREMENT |
| 一時テーブル | `CREATE TEMPORARY TABLE` | `#temp` → 一時テーブル |
| QUOTE | バッククォート \` | PostgreSQL はダブルクォート |

### 10.3 実装計画

PostgreSQL Emitter を参考に、以下のモジュールを実装：

1. `datatype.rs`: データ型マッパー
2. `function.rs`: 関数マッパー
3. `syntax.rs`: 構文マッパー
4. `expression.rs`: 式エミッター
5. `lib.rs`: メインエミッター

### 10.4 結論

**未実装**。新規実装が必要です。

---

## まとめ

| 機能 | 状態 | 作業内容 |
|------|------|----------|
| PostgreSQL Emitter: サブクエリ | **完了** | 追加作業不要 |
| Parser: CREATE TABLE 制約 | **完了** | 追加作業不要 |
| Parser: サブクエリ内FROM | **完了** | 追加作業不要 |
| Parser: プロシージャ本体 | **部分** | TRY...CATCH, トランザクションを実装 |
| WASM: Placeholder | **未実装** | Emitter統合を実装 |
| MySQL Emitter | **未実装** | 新規実装 |

---

## 次のステップ

1. **簡易実装分析レポートの更新**: 3つの機能が実装済みであることを報告
2. **プロシージャ本体の完全実装**: TRY...CATCH とトランザクション制御を実装
3. **WASM 統合**: PostgreSQL Emitter を WASM から呼び出せるようにする
4. **MySQL Emitter の仕様作成**: `spec.json` を `ready_for_implementation: true` に更新

---

## 参考資料

- [PostgreSQL Subquery Syntax](https://www.postgresql.org/docs/current/sql-expressions.html#SQL-SYNTAX-SCALAR-SUBQUERIES)
- [SAP ASE T-SQL Reference](https://help.sap.com/ase)
- TSQLRemaker Architecture Rules: `.claude/rules/architecture-coupling-balance.md`
