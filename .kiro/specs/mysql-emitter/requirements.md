# Requirements Document

## Introduction

MySQL Emitter は、Common SQL AST を入力として受け取り、MySQL 方言の SQL 文字列を出力するライブラリである。SAP ASE T-SQL で記述されたストアドプロシージャを MySQL で実行可能な SQL スクリプトに変換する T-SQL Remaker プロジェクトの中核コンポーネントの一つである。

### 対象ユーザー

- SAP ASE から MySQL への移行を検討している開発者
- レガシーな T-SQL コードのメンテナンスを行う DBA
- 異なるデータベース間の SQL 移行ツールを必要としているチーム

### スコープ

本仕様の範囲は以下の通り：

- **含む**: Common SQL AST から MySQL SQL のコード生成
- **含む**: T-SQL 固有の構文を MySQL 相当の構文へ変換
- **含む**: データ型、関数、一時テーブルの変換
- **除く**: Common SQL AST の構築（Parser の責務）
- **除く**: T-SQL の字句解析・構文解析（Lexer/Parser の責務）

## Requirements

### Requirement 1: AST トラバーサル

**Objective:** Common SQL AST を巡回し、各ノードを MySQL SQL に変換する機能を提供する

As a 移行ツール開発者, I want Visitor パターンで AST を巡回して SQL を生成したい, so that 拡張可能なアーキテクチャを維持できる

#### Acceptance Criteria

1. When Common SQL AST の `Statement` ノードが入力される, the MySQL Emitter shall 対応する MySQL SQL 文を生成する
2. When Common SQL AST の `Expression` ノードが入力される, the MySQL Emitter shall 対応する MySQL 式を生成する
3. When Common SQL AST の `DataType` ノードが入力される, the MySQL Emitter shall 対応する MySQL データ型文字列を生成する
4. When 不明な AST ノードタイプが入力される, the MySQL Emitter shall エラーを返すのではなく、コメントまたはプレースホルダーを出力する
5. The MySQL Emitter shall `Visitor` trait を実装する

### Requirement 2: データ型変換

**Objective:** T-SQL のデータ型を MySQL 相当のデータ型に変換する

As a 移行ツールユーザー, I want T-SQL データ型が正しく MySQL データ型に変換される, so that 移行後のスキーマが正しく動作する

#### Acceptance Criteria

1. When `BIGINT` データ型が入力される, the MySQL Emitter shall `BIGINT` を出力する
2. When `VARCHAR(n)` データ型が入力される, the MySQL Emitter shall `VARCHAR(n)` を出力する
3. When `DATETIME` データ型が入力される, the MySQL Emitter shall `DATETIME` を出力する
4. When `TEXT` データ型が入力される, the MySQL Emitter shall `TEXT` を出力する
5. When `INT` データ型が入力される, the MySQL Emitter shall `INT` を出力する
6. When `SMALLINT` データ型が入力される, the MySQL Emitter shall `SMALLINT` を出力する
7. When `TINYINT` データ型が入力される, the MySQL Emitter shall `TINYINT` を出力する
8. When `DECIMAL(p,s)` データ型が入力される, the MySQL Emitter shall `DECIMAL(p,s)` を出力する
9. When `NUMERIC(p,s)` データ型が入力される, the MySQL Emitter shall `DECIMAL(p,s)` を出力する（MySQL は NUMERIC を DECIMAL の別名として扱う）
10. When `FLOAT` データ型が入力される, the MySQL Emitter shall `FLOAT` を出力する
11. When `REAL` データ型が入力される, the MySQL Emitter shall `DOUBLE` を出力する
12. When `DOUBLE` データ型が入力される, the MySQL Emitter shall `DOUBLE` を出力する
13. When `CHAR(n)` データ型が入力される, the MySQL Emitter shall `CHAR(n)` を出力する
14. When `NCHAR(n)` データ型が入力される, the MySQL Emitter shall `CHAR(n)` を出力する（MySQL は NATIONAL CHAR をサポート）
15. When `NVARCHAR(n)` データ型が入力される, the MySQL Emitter shall `VARCHAR(n)` を出力する
16. When `DATE` データ型が入力される, the MySQL Emitter shall `DATE` を出力する
17. When `TIME` データ型が入力される, the MySQL Emitter shall `TIME` を出力する
18. When `TIMESTAMP` データ型が入力される, the MySQL Emitter shall `TIMESTAMP` を出力する
19. When `BIT` データ型が入力される, the MySQL Emitter shall `TINYINT(1)` を出力する（MySQL の BOOL 相当）
20. When `MONEY` データ型が入力される, the MySQL Emitter shall `DECIMAL(19,4)` を出力する
21. When `UNIQUEIDENTIFIER` データ型が入力される, the MySQL Emitter shall `CHAR(36)` を出力する
22. When `BINARY(n)` データ型が入力される, the MySQL Emitter shall `BINARY(n)` を出力する
23. When `VARBINARY(n)` データ型が入力される, the MySQL Emitter shall `VARBINARY(n)` を出力する
24. When `IMAGE` データ型が入力される, the MySQL Emitter shall `LONGBLOB` を出力する

### Requirement 3: 関数変換

**Objective:** T-SQL の組込関数を MySQL 相当の関数に変換する

As a 移行ツールユーザー, I want T-SQL 関数が MySQL 関数に正しく変換される, so that 移行後のクエリが同じ結果を返す

#### Acceptance Criteria

1. When `GETDATE()` 関数が入力される, the MySQL Emitter shall `NOW()` を出力する
2. When `GETUTCDATE()` 関数が入力される, the MySQL Emitter shall `UTC_TIMESTAMP()` を出力する
3. When `DATEADD(day, n, date)` 関数が入力される, the MySQL Emitter shall `DATE_ADD(date, INTERVAL n DAY)` を出力する
4. When `DATEADD(month, n, date)` 関数が入力される, the MySQL Emitter shall `DATE_ADD(date, INTERVAL n MONTH)` を出力する
5. When `DATEADD(year, n, date)` 関数が入力される, the MySQL Emitter shall `DATE_ADD(date, INTERVAL n YEAR)` を出力する
6. When `DATEDIFF(day, start, end)` 関数が入力される, the MySQL Emitter shall `DATEDIFF(end, start)` を出力する（注: MySQL は引数順が逆）
7. When `LEN(s)` 関数が入力される, the MySQL Emitter shall `LENGTH(s)` を出力する
8. When `SUBSTRING(s, start, len)` 関数が入力される, the MySQL Emitter shall `SUBSTRING(s, start, len)` を出力する
9. When `LEFT(s, n)` 関数が入力される, the MySQL Emitter shall `LEFT(s, n)` を出力する
10. When `RIGHT(s, n)` 関数が入力される, the MySQL Emitter shall `RIGHT(s, n)` を出力する
11. When `LTRIM(s)` 関数が入力される, the MySQL Emitter shall `LTRIM(s)` を出力する
12. When `RTRIM(s)` 関数が入力される, the MySQL Emitter shall `RTRIM(s)` を出力する
13. When `CHARINDEX(s1, s2)` 関数が入力される, the MySQL Emitter shall `LOCATE(s1, s2)` を出力する
14. When `PATINDEX(pattern, s)` 関数が入力される, the MySQL Emitter shall `LOCATE(pattern, s)` を出力する（簡易変換）
15. When `REPLACE(s, old, new)` 関数が入力される, the MySQL Emitter shall `REPLACE(s, old, new)` を出力する
16. When `REPLICATE(s, n)` 関数が入力される, the MySQL Emitter shall `REPEAT(s, n)` を出力する
17. When `STUFF(s, start, len, insert)` 関数が入力される, the MySQL Emitter shall `INSERT(STRING, s, start, len, insert)` を出力する（警告: MySQL 8.0+ のみ）
18. When `ISNULL(expr, default)` 関数が入力される, the MySQL Emitter shall `IFNULL(expr, default)` を出力する
19. When `COALESCE(expr1, expr2, ...)` 関数が入力される, the MySQL Emitter shall `COALESCE(expr1, expr2, ...)` を出力する
20. When `NEWID()` 関数が入力される, the MySQL Emitter shall `UUID()` を出力する
21. When `RAND(seed)` 関数が入力される, the MySQL Emitter shall `RAND()` を出力する（MySQL はシード引数をサポートしない）
22. When `ABS(n)` 関数が入力される, the MySQL Emitter shall `ABS(n)` を出力する
23. When `CEILING(n)` 関数が入力される, the MySQL Emitter shall `CEIL(n)` を出力する
24. When `FLOOR(n)` 関数が入力される, the MySQL Emitter shall `FLOOR(n)` を出力する
25. When `ROUND(n, d)` 関数が入力される, the MySQL Emitter shall `ROUND(n, d)` を出力する
26. When `POWER(x, y)` 関数が入力される, the MySQL Emitter shall `POW(x, y)` を出力する
27. When `SQRT(n)` 関数が入力される, the MySQL Emitter shall `SQRT(n)` を出力する

### Requirement 4: 構文変換

**Objective:** T-SQL 固有の構文を MySQL 相当の構文に変換する

As a 移行ツールユーザー, I want T-SQL 構文が MySQL 構文に変換される, so that 移行後の SQL が MySQL で実行可能になる

#### Acceptance Criteria

1. When `SELECT TOP n` クエリが入力される, the MySQL Emitter shall `SELECT ... LIMIT n` を出力する
2. When `SELECT @variable = expr` 構文が入力される, the MySQL Emitter shall `SET @variable = (SELECT expr)` を出力する
3. When `DECLARE @var type` 構文が入力される, the MySQL Emitter shall コメントを付けて警告を出力する（MySQL はストアドプロシージャ内のみ DECLARE をサポート）
4. When `#temp_table` 一時テーブルが入力される, the MySQL Emitter shall `CREATE TEMPORARY TABLE temp_table` を出力する
5. When `##global_temp` グローバル一時テーブルが入力される, the MySQL Emitter shall `CREATE TABLE global_temp` を出力する（警告: MySQL はグローバル一時テーブルを非サポート）
6. When `BEGIN TRAN` / `COMMIT TRAN` が入力される, the MySQL Emitter shall `START TRANSACTION` / `COMMIT` を出力する
7. When `ROLLBACK TRAN` が入力される, the MySQL Emitter shall `ROLLBACK` を出力する
8. When `RAISERROR` が入力される, the MySQL Emitter shall `SIGNAL SQLSTATE` を出力する
9. When `PRINT` が入力される, the MySQL Emitter shall `SELECT` を出力する（MySQL は PRINT を非サポート）
10. When `GO` バッチ区切りが入力される, the MySQL Emitter shall 空行またはセミコロンを出力する

### Requirement 5: SELECT 文生成

**Objective:** Common SQL AST の SELECT 文を MySQL SELECT 文に変換する

As a 移行ツールユーザー, I want SELECT 文が正しく変換される, so that クエリの意味が保持される

#### Acceptance Criteria

1. When 単純な SELECT 文が入力される, the MySQL Emitter shall `SELECT columns FROM table` 形式を出力する
2. When SELECT 文に WHERE 句が含まれる, the MySQL Emitter shall WHERE 条件を出力する
3. When SELECT 文に JOIN が含まれる, the MySQL Emitter shall `INNER JOIN` / `LEFT JOIN` / `RIGHT JOIN` 構文を出力する
4. When SELECT 文に GROUP BY が含まれる, the MySQL Emitter shall `GROUP BY` 句を出力する
5. When SELECT 文に HAVING が含まれる, the MySQL Emitter shall `HAVING` 句を出力する
6. When SELECT 文に ORDER BY が含まれる, the MySQL Emitter shall `ORDER BY` 句を出力する
7. When SELECT 文に DISTINCT が含まれる, the MySQL Emitter shall `SELECT DISTINCT` を出力する
8. When SELECT 文に UNION が含まれる, the MySQL Emitter shall `UNION` を出力する
9. When SELECT 文にサブクエリが含まれる, the MySQL Emitter shall サブクエリを括弧で囲んで出力する
10. When SELECT 文に LIMIT/TOP が含まれる, the MySQL Emitter shall `LIMIT n` を出力する

### Requirement 6: INSERT 文生成

**Objective:** Common SQL AST の INSERT 文を MySQL INSERT 文に変換する

As a 移行ツールユーザー, I want INSERT 文が正しく変換される, so that データ挿入が正しく動作する

#### Acceptance Criteria

1. When `INSERT INTO table VALUES (...)` が入力される, the MySQL Emitter shall 同一構文を出力する
2. When `INSERT INTO table (cols) VALUES (...)` が入力される, the MySQL Emitter shall 同一構文を出力する
3. When `INSERT INTO table SELECT ...` が入力される, the MySQL Emitter shall 同一構文を出力する
4. When `INSERT ... EXEC` が入力される, the MySQL Emitter shall コメント付き警告を出力する（MySQL は非サポート）

### Requirement 7: UPDATE 文生成

**Objective:** Common SQL AST の UPDATE 文を MySQL UPDATE 文に変換する

As a 移行ツールユーザー, I want UPDATE 文が正しく変換される, so that データ更新が正しく動作する

#### Acceptance Criteria

1. When 単純な UPDATE 文が入力される, the MySQL Emitter shall `UPDATE table SET col = val WHERE ...` 形式を出力する
2. When UPDATE 文に FROM 句が含まれる（T-SQL 拡張）, the MySQL Emitter shall JOIN を使用した形式に変換する
3. When UPDATE 文に TOP が含まれる, the MySQL Emitter shall `LIMIT` を使用した形式に変換する

### Requirement 8: DELETE 文生成

**Objective:** Common SQL AST の DELETE 文を MySQL DELETE 文に変換する

As a 移行ツールユーザー, I want DELETE 文が正しく変換される, so that データ削除が正しく動作する

#### Acceptance Criteria

1. When 単純な DELETE 文が入力される, the MySQL Emitter shall `DELETE FROM table WHERE ...` 形式を出力する
2. When DELETE 文に FROM 句が含まれる（T-SQL 拡張）, the MySQL Emitter shall JOIN を使用した形式に変換する
3. When DELETE 文に TOP が含まれる, the MySQL Emitter shall `LIMIT` を使用した形式に変換する

### Requirement 9: CREATE TABLE 文生成

**Objective:** Common SQL AST の CREATE TABLE 文を MySQL CREATE TABLE 文に変換する

As a 移行ツールユーザー, I want CREATE TABLE 文が正しく変換される, so that テーブル定義が正しく移行できる

#### Acceptance Criteria

1. When 単純な CREATE TABLE 文が入力される, the MySQL Emitter shall `CREATE TABLE table (...)` 形式を出力する
2. When カラム定義に制約が含まれる, the MySQL Emitter shall `NOT NULL` / `DEFAULT` / `PRIMARY KEY` 制約を出力する
3. When IDENTITY 制約が含まれる, the MySQL Emitter shall `AUTO_INCREMENT` を出力する
4. When UNIQUE 制約が含まれる, the MySQL Emitter shall `UNIQUE` 制約を出力する
5. When FOREIGN KEY 制約が含まれる, the MySQL Emitter shall `FOREIGN KEY` 制約を出力する
6. When CHECK 制約が含まれる, the MySQL Emitter shall `CHECK` 制約を出力する
7. When 複数カラムの主キーが定義される, the MySQL Emitter shall テーブルレベルの `PRIMARY KEY` 制約を出力する

### Requirement 10: 一時テーブル変換

**Objective:** T-SQL の一時テーブルを MySQL の一時テーブルに変換する

As a 移行ツールユーザー, I want 一時テーブルが正しく変換される, so that 一時的なデータ処理が正しく動作する

#### Acceptance Criteria

1. When `#temp_table` が参照される, the MySQL Emitter shall `temp_table` に変換する
2. When `##global_temp` が参照される, the MySQL Emitter shall `global_temp` に変換し、警告コメントを出力する
3. When 一時テーブルが作成される, the MySQL Emitter shall `CREATE TEMPORARY TABLE` を使用する
4. When 一時テーブルが削除される, the MySQL Emitter shall `DROP TEMPORARY TABLE` を使用する

### Requirement 11: 変数代入構文変換

**Objective:** T-SQL の変数代入構文を MySQL の構文に変換する

As a 移行ツールユーザー, I want 変数代入が正しく変換される, so that 変数操作が正しく動作する

#### Acceptance Criteria

1. When `SET @variable = value` が入力される, the MySQL Emitter shall 同一構文を出力する
2. When `SELECT @variable = expression` が入力される, the MySQL Emitter shall `SET @variable = (SELECT expression)` に変換する
3. When 複数の変数が SELECT で代入される, the MySQL Emitter shall 個別の SET 文に分割する

### Requirement 12: 制御フロー構文変換

**Objective:** T-SQL の制御フロー構文を MySQL の構文に変換する

As a 移行ツールユーザー, I want 制御フローが正しく変換される, so that プログラムロジックが保持される

#### Acceptance Criteria

1. When `IF ... ELSE` 構文が入力される, the MySQL Emitter shall `IF ... THEN ... END IF` を出力する（ストアドプロシージャ内）
2. When `WHILE` ループが入力される, the MySQL Emitter shall `WHILE ... DO ... END WHILE` を出力する
3. When `BREAK` が入力される, the MySQL Emitter shall `LEAVE` を出力する
4. When `CONTINUE` が入力される, the MySQL Emitter shall `ITERATE` を出力する
5. When `BEGIN ... END` ブロックが入力される, the MySQL Emitter shall `BEGIN ... END` を出力する

### Requirement 13: エラーハンドリング

**Objective:** エラーが発生した場合に適切に処理する

As a 移行ツールユーザー, I want エラーが明確に報告される, so that 問題を特定できる

#### Acceptance Criteria

1. When サポートされない AST ノードが入力される, the MySQL Emitter shall エラーを返す
2. When 変換不能な構文が検出される, the MySQL Emitter shall エラーメッセージに位置情報を含める
3. When 生成される SQL が不完全である, the MySQL Emitter shall 警告をログに記録する

### Requirement 14: パフォーマンス

**Objective:** 高速なコード生成を実現する

As a 移行ツールユーザー, I want 大きなファイルでも高速に変換される, so that 待ち時間が最小限になる

#### Acceptance Criteria

1. When 1000行のストアドプロシージャが入力される, the MySQL Emitter shall 1秒以内に処理を完了する
2. When 複雑なネストされたクエリが入力される, the MySQL Emitter shall スタックオーバーフローを起こさない
3. When 大量の変数が使用される, the MySQL Emitter shall メモリ使用量が適切に管理される

### Requirement 15: テストカバレッジ

**Objective:** 十分なテストカバレッジを確保する

As a 開発者, I want 全ての主要な機能がテストされている, so that 回帰バグを防ぐことができる

#### Acceptance Criteria

1. When テストスイートが実行される, the MySQL Emitter shall 80%以上のカバレッジを達成する
2. When クリティカルパス（データ型変換、関数変換、構文変換）がテストされる, the カバレッジ shall 90%以上である
3. When エラーケースがテストされる, the MySQL Emitter shall 全てのエラー分岐がカバーされる

### Requirement 16: 出力フォーマット

**Objective:** 読みやすく整形された SQL を出力する

As a 移行ツールユーザー, I want 整形された SQL が出力される, so that 出力を確認・修正しやすい

#### Acceptance Criteria

1. When SQL が生成される, the MySQL Emitter shall 適切なインデントを使用する
2. When 長いクエリが生成される, the MySQL Emitter shall 適切な位置で改行する
3. When 複数のステートメントが生成される, the MySQL Emitter shall セミコロンで区切る

### Requirement 17: 依存関係ルール

**Objective:** アーキテクチャの依存関係ルールを遵守する

As a アーキテクト, I want 依存関係が単一方向である, so that コンポーネント間の結合が適切に管理される

#### Acceptance Criteria

1. The MySQL Emitter shall `common-sql-ast` クレートにのみ依存する
2. The MySQL Emitter shall `tsql-parser` に直接依存しない
3. The MySQL Emitter shall `tsql-lexer` に直接依存しない
4. When `Visitor` trait が実装される, the 出力型 shall `String` である
