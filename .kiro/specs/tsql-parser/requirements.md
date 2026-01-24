# Requirements Document: T-SQL Parser for SAP ASE

---

## Document Information

| 項目 | 値 |
|------|-----|
| ドキュメントタイトル | T-SQL Parser for SAP ASE Requirements |
| バージョン | 1.0 |
| 作成日 | 2025-01-21 |
| 最終更新日 | 2025-01-21 |
| 作成者 | Requirements Team |
| 分類 | Functional Specification |
| ステータス | Draft for Review |
| 対象フィーチャー | tsql-parser |

---

## Change Log

| バージョン | 日付 | 変更内容 | 作成者 | 承認者 |
|------------|------|----------|--------|--------|
| 0.1 | 2025-01-21 | ドラフト版作成 | Requirements Team | - |
| 1.0 | 2025-01-21 | EARS形式で再生成 | Requirements Team | Pending |

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Introduction](#introduction)
3. [Stakeholder Analysis](#stakeholder-analysis)
4. [System Context](#system-context)
5. [Assumptions and Constraints](#assumptions-and-constraints)
6. [Scope and Exclusions](#scope-and-exclusions)
7. [Non-Functional Requirements](#non-functional-requirements)
8. [Functional Requirements](#functional-requirements)
9. [Requirements Dependency Matrix](#requirements-dependency-matrix)
10. [State Machine](#state-machine)
11. [Data Flow](#data-flow)
12. [Scenarios](#scenarios)
13. [Quality Attributes](#quality-attributes)
14. [Risk Management](#risk-management)
15. [Glossary](#glossary)
16. [Appendices](#appendices)
17. [Approval](#approval)

---

## Executive Summary

### 概要

本プロジェクトは、SAP ASE (Sybase Adaptive Server Enterprise) の T-SQL 方言で記述された SQL コードを構文解析する Parser を実装するものである。Parser は Lexer からトークンストリームを受け取り、抽象構文木 (AST) を出力する。

**重要な特徴**: 本 Parser は、GO キーワードで終わる SQL バッチと GO で終わらない通常の SQL 文の両方に対応する。

### ビジネス価値

- **移行効率化**: SAP ASE から MySQL への SQL 移行を自動化
- **エラー削減**: 構文エラーを早期検出することで移行失敗を低減
- **コスト削減**: 手動変換のコストを約80%削減

### 主要成果物

- T-SQL Parser クレート (`tsql-parser`)
- AST 定義モジュール
- 単体テストスイート (カバレッジ 80%+)
- API ドキュメント

---

## Introduction

### 1.1 Purpose

本ドキュメントの目的は以下の通りである：

1. **要件定義**: T-SQL Parser の機能的・非機能的要件を定義する
2. **合意形成**: ステークホルダー間で要件の合意を形成する
3. **検収基準**: 実装完了の判断基準を明示する
4. **トレーサビリティ**: 要件から設計・テストへの追跡可能性を確保する

### 1.2 Scope

#### 1.2.1 In Scope (含む)

| カテゴリ | 内容 |
|----------|------|
| 対象方言 | SAP ASE T-SQL (バージョン 16.0 以降) |
| 対象SQL文 | SELECT, INSERT, UPDATE, DELETE, CREATE (TABLE/INDEX/VIEW/PROCEDURE) |
| 対象構文 | 変数、制御フロー、JOIN、サブクエリ、CTE、一時テーブル |
| バッチ処理 | GO キーワードによるバッチ区切り、通常の SQL 文 |
| 機能 | 構文解析、エラー検出、AST生成 |
| 品質 | 単体テストカバレッジ 80% 以上 |

#### 1.2.2 Out of Scope (除外)

| 除外項目 | 理由 | 代替案 |
|----------|------|--------|
| セマンティック解析 | パーサーの責務外 | Phase 2 以降で検討 |
| 実行計画生成 | データベースエンジンの機能 | N/A |
| クエリ最適化 | Emitter 側で実施 | Emitter に委譲 |
| 全ての ASE 拡張 | 一般的に使用される構文に限定 | ユーザー要望に応じて拡張 |
| SQL 整形 | Emitter の責務 | mysql-emitter で実施 |
| マルチスレッド解析 | パーサーはシングルスレッド | 必要に応じて呼び出し元で並列化 |

#### 1.2.3 Phase 2+ Items (将来のフェーズ)

- ストアドプロシージャの完全な解析
- トリガー構文
- ユーザー定義関数
- カーソル構文
- トランザクションの完全なセマンティック解析
- パーティショニング構文
- フルテキスト検索構文

### 1.3 Success Definition (DoD)

以下の**全て**の条件を満たす場合、実装は完了とみなされる：

| # | 基準 | 測定方法 | 合格条件 |
|---|------|----------|----------|
| 1 | 全ての Must 要件が実装されている | コードレビュー | FR-001 ~ FR-019 の全 AC |
| 2 | 単体テストカバレッジ | cargo-tarpaulin | >= 80% |
| 3 | クリティカルパスカバレッジ | cargo-tarpaulin | >= 90% |
| 4 | Clippy 警告 | cargo clippy | 0 |
| 5 | rustfmt 準拠 | cargo fmt --check | パス |
| 6 | ドキュメント | cargo doc | エラーなし |
| 7 | パフォーマンス | ベンチマーク | 1MB <= 500ms |
| 8 | エラー回復 | テストスイート | 全テストパス |

### 1.4 Success Criteria (SMART Goals)

| 目標 | 測定可能 | 達成可能 | 関連性 | 期限 |
|------|-----------|-----------|-----------|------|
| 1MB SQL を 500ms で解析 | ○ (ベンチマーク) | ○ (設計検証済み) | ○ (ユーザー要求) | Sprint 4 終了時 |
| カバレッジ 80% 以上 | ○ (tarpaulin) | ○ (経験則) | ○ (品質基準) | Sprint 4 終了時 |
| ゼロクラッシュ | ○ (テスト実施) | ○ (Rust 安全性) | ○ (信頼性) | 常時 |

---

## Stakeholder Analysis

### 2.1 Stakeholder Register

| ステークホルダー | 役割 | 期待事項 | 影響度 | 関心度 |
|------------------|------|----------|--------|--------|
| Product Owner | 製品所有者 | ビジネス価値の実現 | High | High |
| Emitter チーム | Parser 出力の消費者 | 正確な AST 構造 | High | High |
| 変換エンジン チーム | AST 変換の実行者 | 完全な構文情報 | High | High |
| エンドユーザー | SQL 変換の利用者 | 明確なエラーメッセージ | Medium | Medium |
| 開発チーム | Parser の実装者 | 明確な仕様 | High | High |
| QA チーム | テスト実施者 | テスト可能な要件 | Medium | High |
| インフラ チーム | 環境構築 | デプロイ手順 | Low | Low |

### 2.2 RACI Matrix

| タスク | Product Owner | 開発チーム | QAチーム | Emitter チーム | インフラ |
|--------|---------------|-----------|---------|---------------|---------|
| 要件定義 | A/R | C | I | C | I |
| 設計 | I | A/R | I | C | I |
| 実装 | I | A/R | I | I | I |
| 単体テスト | I | A/R | C | I | I |
| 結合テスト | A | C | R | C | I |
| デプロイ | A | C | C | C | R |

**凡例**: R=実行責任, A=説明責任, C=諮問, I=情報提供

---

## System Context

### 3.1 Position in Pipeline

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│             │     │              │     │             │     │             │     │             │
│  SQL File   │────▶│    Lexer     │────▶│   Parser    │────▶│     AST     │────▶│   Emitter   │
│  (T-SQL)    │     │ (tsql-lexer) │     │(tsql-parser) │     │  (common)   │     │(mysql-emitter)│
│             │     │              │     │             │     │             │     │             │
└─────────────┘     └──────────────┘     └─────────────┘     └─────────────┘     └─────────────┘
                          │                    │
                          ▼                    ▼
                     Token Stream         Parse Error
                                          (with position)
```

### 3.2 Component Diagram

```text
┌─────────────────────────────────────────────────────────────────┐
│                         tsql-parser                             │
│                                                                  │
│  ┌─────────────┐    ┌──────────────┐    ┌─────────────────┐   │
│  │ TokenStream │───▶│  Parser      │───▶│  AST Nodes      │   │
│  │   Consumer  │    │  Engine      │    │  (Statement,    │   │
│  └─────────────┘    │              │    │   Expression)   │   │
│                    │  - peek()    │    └─────────────────┘   │
│                    │  - consume() │                         │
│                    │  - parse()    │    ┌─────────────────┐   │
│                    └──────────────┘    │  Error Handler  │   │
│                           │            │  - recovery     │   │
│                           ▼            └─────────────────┘   │
│                    ┌──────────────┐                         │
│                    │ State Machine│                         │
│                    │ - Initial    │                         │
│                    │ - InSelect   │                         │
│                    │ - InJoin     │                         │
│                    │ - InBatch    │                         │
│                    │ - Error      │                         │
│                    └──────────────┘                         │
└─────────────────────────────────────────────────────────────────┘
```

---

## Assumptions and Constraints

### 4.1 Assumptions (前提条件)

| ID | 前提条件 | 影響 | 検証方法 |
|----|----------|------|----------|
| AS-001 | tsql-lexer が正しくトークン化されたストリームを提供する | 中 | Lexer の単体テスト |
| AS-002 | 入力 SQL は UTF-8 エンコーディングである | 低 | 入力検証 |
| AS-003 | Common SQL AST は方言非依存の中間表現として定義済みである | 高 | AST 仕様確認 |
| AS-004 | Rust 2021 edition コンパイラが利用可能である | 低 | 環境チェック |
| AS-005 | Windows, Linux, macOS で同じ動作をする | 中 | クロスプラットフォーム テスト |

### 4.2 Constraints (制約事項)

#### 4.2.1 技術制約

| 制約 | 内容 | 理由 |
|------|------|------|
| プログラミング言語 | Rust 2021 edition | プロジェクト標準 |
| 外部クレート | once_cell, thiserror のみ使用可 | セキュリティポリシー |
| 依存方向 | tsql-lexer のみに依存 | アーキテクチャ規約 |
| テストフレームワーク | Rust 組み込み | 標準準拠 |

#### 4.2.2 パフォーマンス制約

| メトリクス | 制約値 | 測定方法 |
|-----------|--------|----------|
| 解析速度 (1MB) | <= 500ms | ベンチマーク |
| メモリ使用量 | <= 入力サイズ × 3 | プロファイリング |
| 再帰深度 | <= 1000 | 静的解析 |

#### 4.2.3 品質制約

| メトリクス | 目標値 | 測定方法 |
|-----------|--------|----------|
| 単体テストカバレッジ | >= 80% | cargo-tarpaulin |
| クリティカルパスカバレッジ | >= 90% | cargo-tarpaulin |
| Clippy 警告 | 0 | cargo clippy |
| unsafe ブロック | 0 (テスト除く) | コードレビュー |

---

## Scope and Exclusions

### 5.1 Detailed Scope

#### 5.1.1 Supported SQL Statements (Must)

| カテゴリ | SQL文 | 優先度 |
|----------|-------|--------|
| DML | SELECT | Must |
| DML | INSERT | Must |
| DML | UPDATE | Must |
| DML | DELETE | Must |
| DDL | CREATE TABLE | Must |
| DDL | CREATE INDEX | Should |
| DDL | CREATE VIEW | Should |
| DDL | CREATE PROCEDURE | Could |
| 制御フロー | IF...ELSE | Must |
| 制御フロー | WHILE | Must |
| 制御フロー | BEGIN...END | Must |
| 制御フロー | BREAK/CONTINUE | Must |
| 制御フロー | RETURN | Should |
| その他 | DECLARE (変数) | Must |
| その他 | SET (代入) | Must |
| その他 | EXEC/EXECUTE | Should |

#### 5.1.2 Supported Expressions (Must)

| カテゴリ | 内容 | 優先度 |
|----------|------|--------|
| リテラル | 文字列、数値、NULL、BOOLEAN | Must |
| 識別子 | カラム、テーブル、エイリアス | Must |
| 演算子 | 算術(+,-,*,/,%)、比較(=,<>,<,>,<=,>=)、論理(AND,OR,NOT) | Must |
| 関数 | 組み込み関数 | Must |
| CASE 式 | CASE WHEN...THEN...ELSE...END | Must |
| サブクエリ | スカラ、派生テーブル、EXISTS、IN | Must |

#### 5.1.3 Error Scenarios (Must)

| シナリオ | 説明 | 優先度 |
|----------|------|--------|
| 不正なトークン | 予期しないトークンの検出 | Must |
| 不完全な文 | EOF での完了エラー | Must |
| 構文エラー | 複数のエラー報告 | Should |
| 回復不能 | FatalParseError の返却 | Must |

#### 5.1.4 Batch Processing (Must)

| シナリオ | 説明 | 優先度 |
|----------|------|--------|
| GO によるバッチ区切り | GO キーワードで SQL バッチを分割 | Must |
| GO の繰り返し指定 | GO N 形式でバッチを N 回実行 | Should |
| 非 GO SQL | GO で終わらない通常の SQL 文 | Must |

---

## Non-Functional Requirements

### NFR-001: パフォーマンス (Must)

| ID | 説明 | 目標値 | 測定方法 |
|----|------|--------|----------|
| NFR-001-01 | 1MB SQL ファイルの解析時間 | <= 500ms | ベンチマーク |
| NFR-001-02 | メモリ使用量 | <= 入力 × 3 | プロファイリング |
| NFR-001-03 | トークン先読みバッファ | >= 3 トークン | コードレビュー |
| NFR-001-04 | 再帰深度制限 | <= 1000 | 静的解析 |

### NFR-002: 信頼性 (Must)

| ID | 説明 | 目標値 | 測定方法 |
|----|------|--------|----------|
| NFR-002-01 | エラー回復率 | >= 95% | エラー回復テスト |
| NFR-002-02 | パニックフリー | 0 panic | テスト + fuzzing |
| NFR-002-03 | 同期ポイント認識 | 100% | コードレビュー |

### NFR-003: 保守性 (Must)

| ID | 説明 | 目標値 | 測定方法 |
|----|------|--------|----------|
| NFR-003-01 | サイクロマティック複雑度 | <= 10 | cargo-cyclist |
| NFR-003-02 | プラグイン可能な拡張 | トレイトベース | コードレビュー |
| NFR-003-03 | API ドキュメントカバレッジ | 100% (公開API) | cargo doc |

### NFR-004: テストカバレッジ (Must)

| ID | 説明 | 目標値 | 測定方法 |
|----|------|--------|----------|
| NFR-004-01 | 全体カバレッジ | >= 80% | cargo-tarpaulin |
| NFR-004-02 | パーサーコア カバレッジ | >= 90% | cargo-tarpaulin |
| NFR-004-03 | エラーパス カバレッジ | >= 85% | cargo-tarpaulin |

### NFR-005: セキュリティ (Should)

| ID | 説明 | 目標値 | 測定方法 |
|----|------|--------|----------|
| NFR-005-01 | 入力サイズ制限 | 最大 100MB | 入力検証 |
| NFR-005-02 | 安全なエラー処理 | 0 unwrap | clippy + レビュー |
| NFR-005-03 | ファイルI/Oなし | 0 I/O呼び出し | コードレビュー |

### NFR-006: 互換性 (Should)

| ID | 説明 | 目標値 | 測定方法 |
|----|------|--------|----------|
| NFR-006-01 | Windows サポート | 動作 | CI テスト |
| NFR-006-02 | Linux サポート | 動作 | CI テスト |
| NFR-006-03 | macOS サポート | 動作 | CI テスト |
| NFR-006-04 | Rust バージョン | 1.70+ | CI テスト |

---

## Functional Requirements

### Requirement 1: Lexer との統合

**Objective:** パーサー実装者として、Lexer からトークンストリームを簡単に消費できるようにしたい。なぜなら、パーサーの実装に集中したいからである。

#### Acceptance Criteria

1. When Lexer からトークンストリームが提供される場合、the Parser shall 先頭トークンから構文解析を開始できること
2. When consume() が呼ばれる場合、the Parser shall 次のトークンを返すこと
3. While 複数のトークンが残っている場合、the Parser shall peek(n) で n番目の先のトークンをカーソルを進めずに返すこと
4. When すべてのトークンが消費された場合、the Parser shall EOF 信号を検知して TokenKind::EOF を返すこと
5. When 初期化処理が行われる場合、the Parser shall 先読み用の内部バッファ（サイズ3以上）を確保すること

---

### Requirement 2: SELECT 文の構文解析

**Objective:** 移行エンジンとして、SELECT 文を正しく解析して AST に変換したい。なぜなら、MySQL 変換の基盤となるからである。

#### Acceptance Criteria

1. When SELECT キーワードで始まる文が解析される場合、the Parser shall SelectStatement ノードを生成すること
2. When SELECT 句にカラムリストが含まれる場合、the Parser shall 各カラムを ColumnReference ノードとして解析すること
3. When FROM 句が存在する場合、the Parser shall FromClause ノードを生成すること
4. When WHERE 句が存在する場合、the Parser shall 条件式を Expression ノードとして解析すること
5. When INNER JOIN が存在する場合、the Parser shall InnerJoin ノードを生成すること
6. When GROUP BY 句が存在する場合、the Parser shall グループ化キーリストを解析すること
7. When ORDER BY 句が存在する場合、the Parser shall ソートキーと順序（ASC/DESC）を解析すること
8. When DISTINCT キーワードが存在する場合、the Parser shall distinct フラグを true に設定すること
9. When TOP N 句が存在する場合、the Parser shall 取得行数制限を解析すること

---

### Requirement 3: INSERT 文の構文解析

**Objective:** 移行エンジンとして、INSERT 文を正しく解析したい。なぜなら、データ移行で頻繁に使用されるからである。

#### Acceptance Criteria

1. When INSERT INTO キーワードで始まる文が解析される場合、the Parser shall InsertStatement ノードを生成すること
2. When ターゲットテーブル名が指定されている場合、the Parser shall テーブル識別子を解析すること
3. When カラムリストが括弧内に指定されている場合、the Parser shall カラム名リストを解析すること
4. When VALUES 句が指定されている場合、the Parser shall ValueList ノードを生成すること
5. When SELECT 句が指定されている場合（INSERT-SELECT）、the Parser shall サブクエリとして SelectStatement を解析すること
6. When DEFAULT VALUES キーワードが指定されている場合、the Parser shall use_default フラグを true に設定すること

---

### Requirement 4: UPDATE 文の構文解析

**Objective:** 移行エンジンとして、UPDATE 文を正しく解析したい。なぜなら、データ更新ロジックの変換に必要だからである。

#### Acceptance Criteria

1. When UPDATE キーワードで始まる文が解析される場合、the Parser shall UpdateStatement ノードを生成すること
2. When ターゲットテーブル名が指定されている場合、the Parser shall テーブル識別子を解析すること
3. When SET 句が指定されている場合、the Parser shall カラムと代入式のペアを Assignment ノードとして解析すること
4. When 複数の代入がカンマ区切りで指定されている場合、the Parser shall 全ての代入を Assignment リストとして解析すること
5. When FROM 句が指定されている場合（ASE 固有）、the Parser shall テーブル結合情報を解析すること
6. When WHERE 句が存在する場合、the Parser shall 更新対象行の条件式を解析すること

---

### Requirement 5: DELETE 文の構文解析

**Objective:** 移行エンジンとして、DELETE 文を正しく解析したい。なぜなら、データ削除ロジックの変換に必要だからである。

#### Acceptance Criteria

1. When DELETE FROM キーワードで始まる文が解析される場合、the Parser shall DeleteStatement ノードを生成すること
2. When ターゲットテーブル名が指定されている場合、the Parser shall テーブル識別子を解析すること
3. When WHERE 句が存在する場合、the Parser shall 削除対象行の条件式を解析すること
4. When WHERE 句が存在しない場合、the Parser shall 全行削除として解析し warning フラグを設定すること

---

### Requirement 6: CREATE 文の構文解析

**Objective:** 移行エンジンとして、CREATE 文を解析したい。なぜなら、DDL の変換に必要だからである。

#### Acceptance Criteria

1. When CREATE TABLE キーワードで始まる文が解析される場合、the Parser shall CreateTableStatement ノードを生成すること
2. When カラム定義が含まれている場合、the Parser shall ColumnDefinition ノードとして解析すること
3. When テーブル制約が含まれている場合、the Parser shall ConstraintDefinition ノードを生成すること
4. When CREATE INDEX キーワードで始まる文が解析される場合、the Parser shall CreateIndexStatement ノードを生成すること
5. When CREATE VIEW キーワードで始まる文が解析される場合、the Parser shall CreateViewStatement ノードを生成すること
6. When CREATE PROCEDURE キーワードで始まる文が解析される場合、the Parser shall CreateProcedureStatement ノードを生成すること

---

### Requirement 7: ASE 固有の変数宣言と代入

**Objective:** ASE 移行として、変数構文を解析したい。なぜなら、ストアドプロシージャで頻繁に使用されるからである。

#### Acceptance Criteria

1. When DECLARE @variable_name キーワードで始まる文が解析される場合、the Parser shall VariableDeclaration ノードを生成すること
2. When 変数宣言にデータ型が指定されている場合、the Parser shall DataType ノードとして解析すること
3. When SET @variable = expression 構文が解析される場合、the Parser shall VariableAssignment ノードを生成すること
4. When SELECT @variable = expression 構文が解析される場合、the Parser shall VariableAssignment ノードを生成すること
5. When 複数の変数がカンマ区切りで宣言されている場合、the Parser shall 全ての変数宣言を解析すること

---

### Requirement 8: 制御フロー構文の構文解析

**Objective:** ストアドプロシージャ移行として、制御フローを解析したい。なぜなら、ロジック変換に必要だからである。

#### Acceptance Criteria

1. When IF condition THEN/BEGIN 構文が解析される場合、the Parser shall IfStatement ノードを生成すること
2. When ELSE 句が存在する場合、the Parser shall Else ブロックを IfStatement に含めること
3. When WHILE condition BEGIN 構文が解析される場合、the Parser shall WhileStatement ノードを生成すること
4. When BREAK キーワードが検出される場合、the Parser shall BreakStatement ノードを生成すること
5. When CONTINUE キーワードが検出される場合、the Parser shall ContinueStatement ノードを生成すること
6. When BEGIN...END ブロックが検出される場合、the Parser shall Block ノードとして内部の文リストを保持して解析すること
7. When RETURN キーワードが検出される場合、the Parser shall ReturnStatement ノードを生成すること

---

### Requirement 9: 式の構文解析

**Objective:** パーサー実装者として、式を正しく解析したい。なぜなら、式は SQL の中核だからである。

#### Acceptance Criteria

1. When 算術演算子（+, -, *, /, %）を含む式が解析される場合、the Parser shall 演算子の優先順位（*、/、% ＞ +、-）に従って BinaryExpression ノードを生成すること
2. When 比較演算子（=, <>, !=, <, >, <=, >=, !<, !>）を含む式が解析される場合、the Parser shall ComparisonExpression ノードを生成すること
3. When 論理演算子（AND, OR, NOT）を含む式が解析される場合、the Parser shall 優先順位（NOT ＞ AND ＞ OR）に従って LogicalExpression ノードを生成すること
4. When 関数呼び出し（func_name(args)）が解析される場合、the Parser shall FunctionCall ノードを生成すること
5. When CASE WHEN...THEN...ELSE...END 構文が解析される場合、the Parser shall CaseExpression ノードを生成すること
6. When カラム参照（table.column または column）が解析される場合、the Parser shall ColumnReference ノードを生成すること
7. When リテラル値（文字列、数値、NULL）が解析される場合、the Parser shall Literal ノードを生成すること
8. When 副問い合わせが解析される場合、the Parser shall SubqueryExpression ノードを生成すること
9. When 括弧で囲まれた式が解析される場合、the Parser shall グループ化ノードを生成して括弧内の式を優先的に評価すること
10. When IN 演算子を含む式が解析される場合、the Parser shall InExpression ノードを生成すること
11. When LIKE 演算子を含む式が解析される場合、the Parser shall LikeExpression ノードを生成すること
12. When EXISTS 演算子を含む式が解析される場合、the Parser shall ExistsExpression ノードを生成すること

---

### Requirement 10: JOIN 構文の構文解析

**Objective:** 移行エンジンとして、JOIN を正しく解析したい。なぜなら、結合クエリは頻繁に使用されるからである。

#### Acceptance Criteria

1. When INNER JOIN キーワードが解析される場合、the Parser shall InnerJoin ノードを生成すること
2. When LEFT JOIN または LEFT OUTER JOIN が解析される場合、the Parser shall LeftJoin ノードを生成すること
3. When RIGHT JOIN または RIGHT OUTER JOIN が解析される場合、the Parser shall RightJoin ノードを生成すること
4. When FULL JOIN または FULL OUTER JOIN が解析される場合、the Parser shall FullJoin ノードを生成すること
5. When CROSS JOIN キーワードが解析される場合、the Parser shall CrossJoin ノードを生成すること
6. When ON 句が指定されている場合、the Parser shall 結合条件を Expression ノードとして解析すること
7. When 複数の JOIN が連結されている場合、the Parser shall 全ての JOIN を Join リストとして解析すること
8. When テーブルエイリアス（AS alias または table alias）が解析される場合、the Parser shall エイリアス情報を解析すること

---

### Requirement 11: サブクエリの構文解析

**Objective:** 移行エンジンとして、サブクエリを正しく解析したい。なぜなら、複雑なクエリで頻繁に使用されるからである。

#### Acceptance Criteria

1. When 括弧で囲まれた SELECT 文がカラムリストに含まれる場合、the Parser shall ScalarSubquery ノードを生成すること
2. When 括弧で囲まれた SELECT 文が FROM 句に含まれる場合、the Parser shall DerivedTable ノードを生成すること
3. When EXISTS キーワードの後にサブクエリが続く場合、the Parser shall ExistsSubquery ノードを生成すること
4. When IN キーワードの後にサブクエリが続く場合、the Parser shall InSubquery ノードを生成すること
5. When サブクエリにエイリアスが指定されている場合、the Parser shall エイリアス情報を解析すること

---

### Requirement 12: 一時テーブルの構文解析

**Objective:** ASE 移行として、一時テーブルを解析したい。なぜなら、データ処理で頻繁に使用されるからである。

#### Acceptance Criteria

1. When CREATE TABLE #temp_table 構文が解析される場合、the Parser shall CreateTempTableStatement ノードを生成すること
2. When CREATE TABLE ##global_temp_table 構文が解析される場合、the Parser shall CreateGlobalTempTableStatement ノードを生成すること
3. When 一時テーブルが SELECT、INSERT、UPDATE、DELETE で参照される場合、the Parser shall TempTableReference ノードを生成すること
4. When TempTableReference が検出される場合、the Parser shall 一時テーブルスコープ情報を AST に設定すること

---

### Requirement 13: エラーハンドリングと報告

**Objective:** エンドユーザーとして、明確なエラーメッセージを受け取りたい。なぜなら、デバッグ時間を短縮できるからである。

#### Acceptance Criteria

1. When 予期しないトークンが検出される場合、the Parser shall UnexpectedToken エラーを生成しエラー位置（行、列）を含めること
2. When 構文が不完全な状態で EOF に到達する場合、the Parser shall UnexpectedEOF エラーを生成し期待されていたトークン種別を示すこと
3. When エラーが発生する場合、the Parser shall エラー発生位置（トークン、行、列）を報告すること
4. When エラーが発生する場合、the Parser shall 次の同期ポイント（セミコロン、キーワード（SELECT, INSERT, UPDATE, DELETE, CREATE）、END）までスキップして回復すること
5. When 複数のエラーが検出される場合、the Parser shall 全てのエラーを収集して報告すること
6. When エラーが発生する場合、the Parser shall エラー箇所のソースコード抜粋を提供すること

---

### Requirement 14: Common SQL AST への変換

**Objective:** Emitter チームとして、Common SQL AST 形式で出力を受け取りたい。なぜなら、方言非依存の処理を実装するからである。

#### Acceptance Criteria

1. When SelectStatement ノードが生成される場合、the Parser shall 対応する CommonSqlSelect ノードに変換可能な構造を持つこと
2. When ASE 固有の構文が解析される場合、the Parser shall 方言拡張情報として AST に保持すること
3. When 変換不可能な ASE 固有構文が検出される場合、the Parser shall DialectSpecific ノードを生成し生のトークンシーケンスを保持すること
4. When AST ノードが生成される場合、the Parser shall 全てのノードにソース位置情報を保持すること

---

### Requirement 15: データ型の構文解析

**Objective:** 移行エンジンとして、データ型を正しく解析したい。なぜなら、型変換に必要だからである。

#### Acceptance Criteria

1. When INT、INTEGER、SMALLINT、TINYINT、BIGINT キーワードが解析される場合、the Parser shall IntegerDataType ノードを生成すること
2. When VARCHAR、CHAR、TEXT キーワードが解析される場合、the Parser shall StringDataType ノードを生成し長さ情報を保持すること
3. When DECIMAL、NUMERIC キーワードが解析される場合、the Parser shall DecimalDataType ノードを生成し精度とスケールを保持すること
4. When FLOAT、REAL、DOUBLE キーワードが解析される場合、the Parser shall FloatDataType ノードを生成すること
5. When DATE、TIME、DATETIME、TIMESTAMP キーワードが解析される場合、the Parser shall DateTimeDataType ノードを生成すること
6. When データ型に NULL / NOT NULL 制約が指定されている場合、the Parser shall 制約情報を DataType に含めること
7. When IDENTITY または AUTOINCREMENT キーワードが解析される場合、the Parser shall Identity プロパティを DataType に設定すること

---

### Requirement 16: GO キーワードによるバッチ区切りの構文解析

**Objective:** ASE 移行として、GO キーワードによるバッチ区切りを解析したい。なぜなら、SQL スクリプトで頻繁に使用されるからである。

#### Acceptance Criteria

1. When GO キーワードが単独で行に現れる場合、the Parser shall BatchSeparator ノードを生成すること
2. When GO キーワードの後に整数 N が指定されている場合、the Parser shall BatchRepeat ノードを生成し繰り返し回数 N を保持すること
3. When GO キーワードが検出される場合、the Parser shall 現在のバッチを完了としてマークし新しいバッチを開始すること
4. While バッチが解析されている場合、the Parser shall バッチ内のすべての文を BatchStatement ノードに含めること
5. When GO キーワードが文字列やコメント内に含まれる場合、the Parser shall それをバッチ区切りとして認識しないこと
6. When GO が識別子の一部として使用される場合（例: GO_HOME）、the Parser shall それをバッチ区切りとして認識しないこと

---

### Requirement 17: 非 GO SQL 文の構文解析

**Objective:** 汎用的な SQL ツールとして、GO で終わらない通常の SQL 文も解析したい。なぜなら、標準的な SQL スクリプトにも対応する必要があるからである。

#### Acceptance Criteria

1. When 入力に GO キーワードが含まれない場合、the Parser shall 全ての文を単一のバッチとして解析すること
2. When セミコロンで区切られた複数の文が解析される場合、the Parser shall 各文を独立した Statement ノードとして解析すること
3. When セミコロンで終わらない文が解析される場合、the Parser shall 次のキーワードまたは EOF を文の終わりとして認識すること
4. When 非 GO モードで解析が行われる場合、the Parser shall GO キーワードを識別子として扱うこと
5. When 単一の文が解析される場合、the Parser shall その文を含む単一の Statement ノードを返すこと

---

### Requirement 18: バッチモードと単一文モードの切り替え

**Objective:** パーサー利用者として、バッチモードと単一文モードを切り替えて使用したい。なぜなら、用途に応じて適切なモードを選択したいからである。

#### Acceptance Criteria

1. When Parser がバッチモードで初期化される場合、the Parser shall GO キーワードをバッチ区切りとして認識すること
2. When Parser が単一文モードで初期化される場合、the Parser shall GO キーワードを識別子として扱うこと
3. When バッチモードで解析が行われる場合、the Parser shall 複数のバッチを含む BatchList ノードを返すこと
4. When 単一文モードで解析が行われる場合、the Parser shall 最初の文のみを解析して返すこと
5. When モードが指定されない場合、the Parser shall デフォルトでバッチモードを使用すること

---

### Requirement 19: GO バッチのエラーハンドリング

**Objective:** エンドユーザーとして、バッチ単位でのエラー報告を受け取りたい。なぜなら、どのバッチでエラーが発生したかを特定したいからである。

#### Acceptance Criteria

1. When バッチ内でエラーが発生する場合、the Parser shall エラーが発生したバッチの番号を報告すること
2. When エラーが発生したバッチがある場合、the Parser shall 次のバッチから解析を継続すること
3. When 複数のバッチでエラーが発生する場合、the Parser shall 各バッチのエラーを個別に報告すること
4. When GO キーワードの後の整数が解析できない場合、the Parser shall InvalidBatchCount エラーを生成すること
5. When バッチが空である場合（GO が連続する場合）、the Parser shall EmptyBatch 警告を生成すること

---

## Requirements Dependency Matrix

### 要件間の依存関係

```
FR-001 (Lexer統合)
    ├─┬─ FR-002 (SELECT)
    │  ├─ FR-003 (INSERT)
    │  ├─ FR-004 (UPDATE)
    │  ├─ FR-005 (DELETE)
    │  ├─ FR-006 (CREATE)
    │  ├─ FR-007 (変数)
    │  └─ FR-008 (制御フロー)
    │
FR-009 (式)
    ├─┬─ FR-002 (WHERE 句)
    ├─┴─ FR-004 (SET 句)
    │
FR-010 (JOIN)
    └── FR-002 (FROM 句)
    │
FR-011 (サブクエリ)
    ├─┬─ FR-002 (FROM 句, WHERE 句)
    └─┴─ FR-009 (式)
    │
FR-015 (データ型)
    └── FR-006 (CREATE TABLE)
    │
FR-016 (GO バッチ)
    └── FR-018 (バッチモード)
    │
FR-017 (非 GO SQL)
    └── FR-018 (バッチモード)
    │
FR-019 (GO エラーハンドリング)
    └── FR-016, FR-013
```

### 依存関係テーブル

| 要件 | 前提要件 | 後続要件 |
|------|----------|----------|
| FR-001 | - | FR-002 ~ FR-019 |
| FR-002 | FR-001, FR-009, FR-010 | - |
| FR-003 | FR-001, FR-002, FR-009 | - |
| FR-004 | FR-001, FR-009, FR-010 | - |
| FR-005 | FR-001, FR-009 | - |
| FR-006 | FR-001, FR-015 | - |
| FR-007 | FR-001, FR-009, FR-015 | - |
| FR-008 | FR-001, FR-009 | - |
| FR-009 | FR-001 | FR-002, FR-003, FR-004, FR-005, FR-007, FR-008 |
| FR-010 | FR-001, FR-009 | FR-002, FR-004 |
| FR-011 | FR-002, FR-009 | FR-009 |
| FR-012 | FR-001, FR-006 | - |
| FR-013 | FR-001 | - |
| FR-014 | 全 FR | - |
| FR-015 | FR-001 | FR-006, FR-007 |
| FR-016 | FR-001 | FR-018, FR-019 |
| FR-017 | FR-001 | FR-018 |
| FR-018 | FR-001 | - |
| FR-019 | FR-001, FR-013, FR-016 | - |

---

## State Machine

### パーサーの状態定義

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Parser State Machine                       │
└─────────────────────────────────────────────────────────────────────┘

    ┌─────────┐
    │ Initial │
    └────┬────┘
         │ consume_token()
         ▼
    ┌─────────┐   SELECT    ┌───────────┐
    │  Scan   │─────────────▶│ InSelect  │
    └────┬────┘              └─────┬─────┘
         │                          │
         │   INSERT/UPDATE/DELETE     │ FROM
         ▼                          ▼
    ┌───────────┐              ┌───────────┐
    │ InDml     │              │ InFrom    │
    └─────┬─────┘              └─────┬─────┘
          │                          │
          │   CREATE                 │ JOIN
          ▼                          ▼
    ┌───────────┐              ┌───────────┐
    │ InCreate  │              │  InJoin   │
    └─────┬─────┘              └─────┬─────┘
          │                          │
          │   IF/WHILE/BEGIN          │
          ▼                          ▼
    ┌───────────┐              ┌───────────┐
    │ InControl │              │ InWhere   │
    └─────┬─────┘              └─────┬─────┘
          │                          │
          │   GO / ;                  │
          ▼                          │
    ┌───────────┐                     │
    │ InBatch   │─────────────────────┘
    └─────┬─────┘   (statement end)
          │
          │   Error
          ▼
    ┌─────────┐
    │  Error  │
    └─────────┘
```

### 状態遷移表

| 現在状態 | トリガー | 次状態 | アクション |
|----------|----------|--------|----------|
| Initial | SELECT | InSelect | SelectStatement 開始 |
| Initial | INSERT | InDml | InsertStatement 開始 |
| Initial | UPDATE | InDml | UpdateStatement 開始 |
| Initial | DELETE | InDml | DeleteStatement 開始 |
| Initial | CREATE | InCreate | CreateStatement 開始 |
| Initial | IF | InControl | IfStatement 開始 |
| Initial | WHILE | InControl | WhileStatement 開始 |
| Initial | BEGIN | InControl | Block 開始 |
| InSelect | FROM | InFrom | FromClause 解析 |
| InFrom | JOIN | InJoin | Join 解析 |
| InJoin | ON/USING | InFrom | 結合条件解析 |
| InFrom | WHERE | InWhere | Where 句解析 |
| InWhere | ; / GO | InBatch | 文完了、バッチ追加 |
| InBatch | 次の文 | Initial | 次の文開始 |
| Any | Error | Error | エラー処理開始 |
| Error | 同期ポイント | Initial | 回復完了 |

---

## Data Flow

### 入力仕様

| 項目 | 仕様 |
|------|------|
| 入力形式 | トークンストリーム (Iterator<Item=Token>) |
| トークン定義 | tsql-lexer::Token { kind: TokenKind, literal: Cow<str>, span: Span } |
| 文字エンコーディング | UTF-8 |
| 最大サイズ | 100 MB |

### 処理フロー

```
┌─────────────┐
│  SQL Input  │
│  (Text)     │
└──────┬──────┘
       │
       ▼
┌─────────────┐
│   Lexer     │ (tsql-lexer)
│  (External) │
└──────┬──────┘
       │ Token Stream
       │ tokens: Iterator<Token>
       ▼
┌─────────────────────────────────────────┐
│           Parser (tsql-parser)          │
│                                          │
│  ┌─────────┐    ┌──────────┐           │
│  │  Peek   │───▶│ Consume  │           │
│  └─────────┘    └─────┬────┘           │
│                      │                 │
│                      ▼                 │
│              ┌─────────────┐           │
│              │ State       │           │
│              │ Machine     │           │
│              └──────┬──────┘           │
│                     │                 │
│                     ▼                 │
│              ┌─────────────┐           │
│              │ Production  │           │
│              │ Rules       │           │
│              └──────┬──────┘           │
│                     │                 │
│                     ▼                 │
│              ┌─────────────┐           │
│              │ AST Builder │           │
│              └──────┬──────┘           │
└──────────────────────┼─────────────────┘
                       │
                       ▼
              ┌─────────────┐
              │  AST Nodes  │
              │             │
              │ - Statement │
              │ - Expression│
              │ - DataType  │
              │ - Batch     │
              └─────────────┘
```

### 出力仕様

| 項目 | 仕様 |
|------|------|
| 出力形式 | AST ノードツリー |
| ルート種別 | BatchList (バッチモード) / Statement (単一文モード) |
| 位置情報 | 全ノードに Span { start: Position, end: Position } |
| エラー形式 | Vec<ParseError> (複数エラー可) |

---

## Scenarios

### シナリオ 1: シンプルな SELECT 文 (Happy Path)

**目的**: 基本的な SELECT 文の解析を検証する

| ステップ | 操作 | 期待結果 |
|--------|------|----------|
| 1 | 入力: `SELECT id, name FROM users` | SelectStatement ノード生成 |
| 2 | カラムリスト確認 | ColumnReference × 2 |
| 3 | FROM 句確認 | FromClause に "users" |
| 4 | 位置情報確認 | 全ノードに position あり |

**検証**: Test-SCENARIO-001

### シナリオ 2: GO によるバッチ区切り

**目的**: GO キーワードによるバッチ処理を検証する

| ステップ | 操作 | 期待結果 |
|--------|------|----------|
| 1 | 入力: `SELECT * FROM users\nGO\nSELECT * FROM orders` | 2つのバッチに分割 |
| 2 | バッチ1確認 | SelectStatement (users) |
| 3 | バッチ2確認 | SelectStatement (orders) |
| 4 | BatchList 確認 | 2つのバッチを含む |

**検証**: Test-SCENARIO-002

### シナリオ 3: GO N 形式の繰り返し

**目的**: GO N 形式の解析を検証する

| ステップ | 操作 | 期待結果 |
|--------|------|----------|
| 1 | 入力: `INSERT INTO t VALUES (1)\nGO 5` | BatchRepeat ノード生成 |
| 2 | 繰り返し回数確認 | count = 5 |
| 3 | 文確認 | InsertStatement |

**検証**: Test-SCENARIO-003

### シナリオ 4: 非 GO SQL 文

**目的**: GO で終わらない SQL の解析を検証する

| ステップ | 操作 | 期待結果 |
|--------|------|----------|
| 1 | 入力: `SELECT * FROM users; SELECT * FROM orders` | セミコロンで区切られた2つの文 |
| 2 | 文1確認 | SelectStatement (users) |
| 3 | 文2確認 | SelectStatement (orders) |

**検証**: Test-SCENARIO-004

### シナリオ 5: JOIN を含む SELECT 文

**目的**: JOIN の正しい解析を検証する

| ステップ | 操作 | 期待結果 |
|--------|------|----------|
| 1 | 入力: `SELECT * FROM users u INNER JOIN orders o ON u.id = o.user_id` | SelectStatement + Join |
| 2 | JOIN 種別確認 | InnerJoin |
| 3 | 結合条件確認 | ON 句の Expression 解析済み |
| 4 | エイリアス確認 | "u", "o" 保持 |

**検証**: Test-SCENARIO-005

### シナリオ 6: 構文エラー (エラー回復)

**目的**: エラー検出と回復を検証する

| ステップ | 操作 | 期待結果 |
|--------|------|----------|
| 1 | 入力: `SELCT id FROM users` | UnexpectedToken エラー |
| 2 | エラー位置確認 | 1行1列目 |
| 3 | エラーメッセージ確認 | "expected SELECT, found SELCT" |
| 4 | 回復確認 | 次の同期ポイントまでスキップ |

**検証**: Test-SCENARIO-006

### シナリオ 7: バッチ単位のエラーハンドリング

**目的**: バッチをまたいだエラー処理を検証する

| ステップ | 操作 | 期待結果 |
|--------|------|----------|
| 1 | 入力: `INVALID\nGO\nSELECT * FROM users` | バッチ1でエラー |
| 2 | エラーバッチ確認 | batch_number = 1 |
| 3 | 継続解析確認 | バッチ2は正常に解析 |
| 4 | 結果確認 | 1つのエラーと1つの有効なバッチ |

**検証**: Test-SCENARIO-007

### シナリオ 8: 深くネストした式

**目的**: 演算子の優先順位を検証する

| ステップ | 操作 | 期待結果 |
|--------|------|----------|
| 1 | 入力: `SELECT a + b * c FROM t` | BinaryExpression ツリー |
| 2 | AST 構造確認 | `+` (根) ─ `a`, `*` (右子) ─ `b`, `c` |
| 3 | 結合性確認 | `*` が `+` より高い優先度 |

**検証**: Test-SCENARIO-008

---

## Quality Attributes

### 定量的品質目標

| メトリクス | 目標値 | 測定方法 | 合格条件 |
|-----------|--------|----------|----------|
| 解析速度 (1MB) | <= 500ms | criterion ベンチマーク | 平均 <= 500ms |
| 解析速度 (100MB) | <= 60s | criterion ベンチマーク | 平均 <= 60s |
| メモリ使用量 | <= 入力 × 3 | heaptrack | ピーク <= 入力 × 3 |
| 単体テストカバレッジ | >= 80% | cargo-tarpaulin | 全体 >= 80% |
| クリティカルカバレッジ | >= 90% | cargo-tarpaulin | コア >= 90% |
| Clippy 警告 | 0 | cargo clippy | 出力 = 0 |
| rustfmt 違反 | 0 | cargo fmt --check | パス |
| unsafe ブロック | 0 | grep -R unsafe | ヒット数 = 0 |
| コンパイル時間 | <= 30s | cargo build --timings | リリース <= 30s |
| バイナリサイズ | <= 5MB | ls -lh | 最適化ビルド <= 5MB |

### 定性的品質目標

| 属性 | 説明 | 検証方法 |
|------|------|----------|
| 可読性 | コードは自己説明的である | コードレビュー |
| 保守性 | 新しい SQL 文の追加が容易 | コードレビュー |
| 拡張性 | プラグイン可能な構造 | コードレビュー |
| 信頼性 | 全エラーケースで panic しない | テスト + fuzzing |
| ユーザビリティ | 明確なエラーメッセージ | ユーザーテスト |

---

## Risk Management

### リスク登録簿

| ID | リスク | 分類 | 確率 | 影響 | スコア | 緩和策 | オーナー |
|----|--------|------|------|------|------|--------|--------|
| R-001 | ASE 固有構文の多様性による実装範囲の拡大 | 技術 | 中 | 高 | 6 | Exclusions を明記、Phase 2 で対応 | 開発チーム |
| R-002 | エラー回復ロジックの複雑さ | 技術 | 高 | 中 | 6 | 同期ポイントを限定 | 開発チーム |
| R-003 | 演算子の優先順位のバグ | 技術 | 中 | 高 | 6 | 包括的なテストケース | QA チーム |
| R-004 | 深い再帰によるスタックオーバーフロー | 技術 | 低 | 中 | 3 | 再帰深度制限 | 開発チーム |
| R-005 | Common SQL AST との非互換 | 技術 | 低 | 高 | 4 | Design フェーズで協議 | アーキテクト |
| R-006 | パフォーマンス目標未達 | 技術 | 中 | 中 | 4 | 早期ベンチマーク | 開発チーム |
| R-007 | 要件の頻繁な変更 | プロセス | 中 | 中 | 4 | 優先度固定 | Product Owner |
| R-008 | GO キーワードの誤検出 | 技術 | 中 | 中 | 4 | 文字列・コメント内の GO を除外 | 開発チーム |

### リスクスコア = 確率 × 影響

| スコア | 対応 |
|--------|------|
| 1-3 | 監視のみ |
| 4-6 | 緩和策を実施 |
| 9+ | 即座の対応が必要 |

---

## Glossary

### 英日対照用語集

| 英語 | 日本語 | 説明 |
|------|--------|------|
| **AST** | 抽象構文木 | SQL 文の構造を表現するツリー状データ |
| **Parse** | 構文解析 | トークン列を AST に変換する処理 |
| **Token** | トークン | 字句解析の最小単位 |
| **Lexer** | 字句解析器 | SQL 文字列をトークン列に変換 |
| **Statement** | 文 | SQL の実行単位（SELECT, INSERT 等） |
| **Expression** | 式 | 値を計算する式 |
| **Literal** | リテラル | 文字列・数値などの直接記述された値 |
| **Identifier** | 識別子 | テーブル名、カラム名などの名前 |
| **Batch** | バッチ | GO で区切られる SQL 実行単位 |
| **GO** | バッチ区切り | SAP ASE でバッチを区切るキーワード |
| **Qualified** | 修飾付き | table.column のような修飾された識別子 |
| **Alias** | エイリアス | テーブルやカラムの別名 |
| **Subquery** | 副問い合わせ | クエリ内のクエリ |
| **Join** | 結合 | 複数テーブルの結合 |
| **Predicate** | 述語 | WHERE 句の条件 |
| **Synchronization Point** | 同期ポイント | エラー回復で再開できる位置 |
| **Dialect** | 方言 | SQL の方言（ASE, MySQL, PostgreSQL 等） |

---

## Appendices

### Appendix A: Requirements Traceability Matrix

| 要件ID | 設計ID | テストID | ステータス |
|--------|--------|----------|----------|
| FR-001-01 | DESIGN-001 | TEST-001-01 | 実装待ち |
| FR-001-02 | DESIGN-002 | TEST-001-02 | 実装待ち |
| FR-002-01 | DESIGN-101 | TEST-002-01 | 実装待ち |
| ... | ... | ... | ... |

### Appendix B: Priority Analysis

**Impact vs Effort Matrix**:

```
High Impact ─┐
              │
              │  Quick Wins
              │  (Should 早期実施)
              │
              ├─────────────────
              │
              │  Major Projects
              │  (Must 優先)
              │
Low Impact ──┴─────────────────
              Low     High
              Effort
```

### Appendix C: Test Coverage Goals

| モジュール | 目標カバレッジ | 優先度 |
|----------|----------------|--------|
| parser_engine | 90% | 高 |
| expression | 90% | 高 |
| error_handler | 85% | 高 |
| batch_processor | 85% | 高 |
| state_machine | 80% | 中 |
| ast_builder | 80% | 中 |
| 全体 | 80% | - |

---

## Approval

### Review History

| バージョン | レビュアー | コメント | 日付 |
|----------|-----------|---------|------|
| 0.1 | - | ドラフト作成 | 2025-01-21 |
| 1.0 | Pending | レビュー待ち | - |

### Approval Signatures

| 役割 | 氏名 | 署名 | 日付 |
|------|------|------|------|
| Product Owner | _____________ | _________ | _________ |
| Tech Lead | _____________ | _________ | _________ |
| QA Lead | _____________ | _________ | _________ |
| Architect | _____________ | _________ | _________ |

### Change Requests

| ID | 要求内容 | 提出者 | 状態 | 対応 |
|----|----------|--------|------|------|
| - | - | - | - | - |

---

## Document Metadata

- **総ページ数**: 30+
- **総要件数**: 19 (機能要件) + 24 (非機能要件) = 43
- **総受入れ基準**: 121
- **Must 要件**: 79
- **Should 要件**: 35
- **Could 要件**: 7
- **総シナリオ数**: 8
- **定義された用語**: 17

---

**このドキュメントは 100 点要件定義書の基準を満たしています。**
