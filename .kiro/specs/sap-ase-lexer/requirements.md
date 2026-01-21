# Requirements Document

## Introduction

本プロジェクトは、SAP ASE (Sybase Adaptive Server Enterprise) の T-SQL 方言で記述された SQL コードを字句解析するための Lexer (字句解析器) を実装するものです。本 Lexer は、tsqlremaker プロジェクトの変換パイプラインの最初の工程として位置づけられ、ソース SQL コードをトークンストリームに変換します。

### 対象範囲

- **対象方言**: SAP ASE T-SQL (Sybase Adaptive Server Enterprise)
- **変換先**: 共通 SQL AST (方言非依存の中間表現)
- **最終目標**: MySQL 互換の SQL への変換

### ステークホルダー

| ステークホルダー | 役割 | 期待事項 |
|------------------|------|----------|
| Parser (構文解析器) | Lexer 出力の消費者 | 正確なトークン情報と位置情報 |
| 変換エンジン | AST 変換の実行者 | 構文の正確な識別 |
| エンドユーザー | SQL 変換の利用者 | 明確なエラーメッセージと警告 |

### 成功基準

- SAP ASE T-SQL の基本構文を正しくトークン化できること
- ASE 固有の構文（ネストされたコメント、変数、一時テーブル）を正しく処理すること
- エラー箇所を明確に示すエラーメッセージを出力できること
- 大規模な SQL ファイルを高速に処理できること

---

## Requirements

### Requirement 1: 基本的なトークン化

**Objective:** Parser が正しく構文解析を行えるように、Lexer は SAP ASE T-SQL のすべての予約語、識別子、演算子、リテラルを正しくトークン化すること。

#### Acceptance Criteria

1. When キーワードが入力された場合、the Lexer shall 対応する TokenKind を生成すること
2. When 識別子が入力された場合、the Lexer shall Ident トークンを生成すること
3. When 演算子が入力された場合、the Lexer shall 対応する演算子トークンを生成すること
4. When 区切り文字（括弧、カンマ、セミコロン等）が入力された場合、the Lexer shall 対応する区切り文字トークンを生成すること
5. The Lexer shall recognize keywords case-insensitively (大文字小文字を区別せずに認識すること)

### Requirement 2: コメント処理

**Objective:** SAP ASE T-SQL のコメント構文（ブロックコメント、ラインコメント、ネストされたコメント）を正しく処理すること。

#### Acceptance Criteria

1. When ブロックコメント `/* */` が入力された場合、the Lexer shall BlockComment トークンを生成すること
2. When ラインコメント `--` が入力された場合、the Lexer shall LineComment トークンを生成すること
3. While ブロックコメント内に別のブロックコメントがネストされている場合、the Lexer shall ネストの深さを追跡して正しくコメントの終了を判定すること
4. If コメントが終了していない状態で EOF に到達した場合、the Lexer shall UnterminatedComment エラーを返すこと
5. Where コメントを保持するオプションが有効な場合、the Lexer shall コメントの内容をトークンに含めること

### Requirement 3: 変数トークン

**Objective:** SAP ASE T-SQL のローカル変数、グローバル変数、一時テーブル識別子のプレフィックス構文を正しくトークン化すること。

#### Acceptance Criteria

1. When `@` プレフィックスを持つ識別子が入力された場合、the Lexer shall LocalVar トークンを生成すること
2. When `@@` プレフィックスを持つ識別子が入力された場合、the Lexer shall GlobalVar トークンを生成すること
3. When `#` プレフィックスを持つ識別子が入力された場合、the Lexer shall TempTable トークンを生成すること
4. When `##` プレフィックスを持つ識別子が入力された場合、the Lexer shall GlobalTempTable トークンを生成すること
5. The Lexer shall preserve the variable name part as the token value (変数名の部分を値として保持すること)

### Requirement 4: 文字列リテラル

**Objective:** SAP ASE T-SQL の文字列リテラル（通常文字列、Unicode 文字列、エスケープシーケンス）を正しくトークン化すること。

#### Acceptance Criteria

1. When シングルクォートで囲まれた文字列が入力された場合、the Lexer shall String トークンを生成すること
2. When `N'...'` 形式の Unicode 文字列が入力された場合、the Lexer shall NString トークンを生成すること
3. When `U&'...'` 形式の Unicode エスケープ文字列が入力された場合、the Lexer shall UnicodeString トークンを生成すること
4. While 文字列内にエスケープされたクォート（`''` または `""`）が存在する場合、the Lexer shall それらを文字列の一部として処理すること
5. If 文字列リテラルが終了していない状態で EOF に到達した場合、the Lexer shall UnterminatedString エラーを返すこと

### Requirement 5: 数値リテラル

**Objective:** 整数、浮動小数点数、科学表記法、16進数リテラルを正しくトークン化すること。

#### Acceptance Criteria

1. When 整数リテラルが入力された場合、the Lexer shall Number トークンを生成すること
2. When 小数点を含む数値が入力された場合、the Lexer shall Float トークンを生成すること
3. When 科学表記法（`1.5e10` など）の数値が入力された場合、the Lexer shall Float トークンを生成すること
4. When `0x` プレフィックスを持つ16進数が入力された場合、the Lexer shall HexString トークンを生成すること
5. The Lexer shall preserve the original string representation of numeric literals (数値リテラルの元の文字列表現を値として保持すること)

### Requirement 6: 演算子の優先度と結合性

**Objective:** 算術演算子、比較演算子、論理演算子、ビット演算子の構文を正しく認識し、Parser が優先順位を判断できる情報を提供すること。

#### Acceptance Criteria

1. When 算術演算子（`+`, `-`, `*`, `/`, `%`）が入力された場合、the Lexer shall 対応する演算子トークンを生成すること
2. When 比較演算子（`=`, `<>`, `!=`, `<`, `>`, `<=`, `>=`, `!<`, `!>`）が入力された場合、the Lexer shall 対応する演算子トークンを生成すること
3. When 論理演算子（`AND`, `OR`, `NOT`）が入力された場合、the Lexer shall 対応するキーワードトークンを生成すること
4. When ビット演算子（`&`, `|`, `^`, `~`）が入力された場合、the Lexer shall 対応する演算子トークンを生成すること
5. When 文字列連結演算子（`||`）が入力された場合、the Lexer shall Concat トークンを生成すること

### Requirement 7: 位置情報の追跡

**Objective:** 各トークンにソースコード上の正確な位置情報（行、列、バイトオフセット）を付与すること。

#### Acceptance Criteria

1. When トークンが生成される場合、the Lexer shall トークンに開始位置と終了位置の情報を含めること
2. When エラーが発生する場合、the Lexer shall エラーの発生位置を行番号と列番号で示すこと
3. The Lexer shall provide position information starting from line 1, column 1 (1行目、1列目から開始される位置情報を提供すること)
4. When 複数行にわたるトークンが生成される場合、the Lexer shall 正しい開始位置と終了位置を計算すること
5. The Lexer shall provide a source code excerpt of the error line for error messages (最大80文字までエラー行のソースコード抜粋を提供すること)

### Requirement 8: エラーハンドリング

**Objective:** 不正な文字、終了していないリテラル、予期しない EOF などのエラー状態を検出し、適切なエラー情報を報告すること。

#### Acceptance Criteria

1. When トークンとして認識できない不正な文字が入力された場合、the Lexer shall Unknown トークンを生成し、エラーを報告すること
2. When 文字列リテラルが終了していない場合、the Lexer shall UnterminatedString エラーを返すこと
3. When ブロックコメントが終了していない場合、the Lexer shall UnterminatedComment エラーを返すこと
4. When エラーが発生した場合、the Lexer shall エラーの種類と位置を含めること
5. While エラーが発生した場合、the Lexer shall 次の同期ポイント（次のセミコロンまたはキーワード）まで読み取りを続けて処理を継続すること（エラーリカバリ）

### Requirement 9: 予約語の認識

**Objective:** SAP ASE T-SQL のすべての予約語（DML、DDL、制御フロー、データ型等）を正しく識別すること。

#### Acceptance Criteria

1. When DML キーワード（SELECT, INSERT, UPDATE, DELETE, MERGE）が入力された場合、the Lexer shall 対応するキーワードトークンを生成すること
2. When DDL キーワード（CREATE, ALTER, DROP, TRUNCATE）が入力された場合、the Lexer shall 対応するキーワードトークンを生成すること
3. When 制御フローキーワード（IF, ELSE, BEGIN, END, WHILE, RETURN, BREAK, CONTINUE）が入力された場合、the Lexer shall 対応するキーワードトークンを生成すること
4. When データ型キーワード（INT, VARCHAR, DATETIME, DECIMAL 等）が入力された場合、the Lexer shall 対応するキーワードトークンを生成すること
5. When トランザクションキーワード（BEGIN TRAN, COMMIT, ROLLBACK）が入力された場合、the Lexer shall 対応するキーワードトークンを生成すること（すべて大文字小文字を区別しない）

### Requirement 10: 引用符付き識別子

**Objective:** 角括弧 `[...]` または二重引用符 `"..."` で囲まれた識別子を正しく処理すること。

#### Acceptance Criteria

1. When 角括弧 `[...]` で囲まれた識別子が入力された場合、the Lexer shall QuotedIdent トークンを生成すること
2. When 二重引用符 `"..."` で囲まれた識別子が入力された場合、the Lexer shall QuotedIdent トークンを生成すること
3. While 引用符付き識別子内にエスケープされた閉じ括弧が存在する場合、the Lexer shall それを識別子の一部として処理すること
4. If 引用符付き識別子が終了していない場合、the Lexer shall UnterminatedIdentifier エラーを返すこと
5. The Lexer shall preserve the identifier name without quotes (引用符を除いた識別子名を値として保持すること)

### Requirement 11: パフォーマンス

**Objective:** 大規模な SQL ファイルを高速に処理できる性能を提供すること。

#### Acceptance Criteria

1. The Lexer shall use a static HashMap for keyword resolution to avoid recreation overhead (キーワードの解決に静的な HashMap を使用して、再作成のオーバーヘッドを回避すること)
2. When 1 MB 以上の SQL ファイル（約10,000行相当）が入力された場合、the Lexer shall 100 ミリ秒以下でトークン化を完了すること
3. The Lexer shall use zero-copy references to source code to minimize memory consumption (ソースコードへの参照（ゼロコピー）を使用して、メモリ消費を最小限に抑えること)
4. When トークンストリームが生成される場合、the Lexer shall avoid unnecessary string allocations (不要な文字列割り当てを回避すること)
5. The Lexer shall process input containing Unicode characters correctly (Unicode 文字を含む入力を正しく処理すること)

### Requirement 12: パーサーとの統合

**Objective:** 生成されたトークンストリームが Parser で効率的に消費できる形式であること。

#### Acceptance Criteria

1. When Parser がトークンを要求した場合、the Lexer shall 次のトークンを返すこと
2. When すべてのトークンが消費された場合、the Lexer shall EOF トークンを返すこと
3. The Lexer shall provide an iterable token stream (イテレータ可能なトークンストリームを提供すること)
4. The Lexer shall support peek functionality (先読み（peek）機能をサポートすること)
5. When Parser がエラーを通知した場合、the Lexer shall 次の同期ポイントまでスキップするための情報を提供すること

### Requirement 13: 空白と改行の処理

**Objective:** 空白文字、タブ、改行を適切に処理し、位置情報を正確に追跡すること。

#### Acceptance Criteria

1. When 空白文字が入力された場合、the Lexer shall それをスキップして次のトークンを生成すること
2. When 改行文字が入力された場合、the Lexer shall 行番号と列番号を正しく更新すること
3. Where コメント保持オプションが有効でない場合、the Lexer shall コメントをスキップして次のトークンを生成すること
4. The Lexer shall handle both CRLF (`\r\n`) and LF (`\n`) newline formats (CRLF と LF の両方の改行形式を正しく処理すること)
5. When タブ文字が入力された場合、the Lexer shall calculate column position assuming 8-space tab width (タブ幅を8スペースとして列位置を計算すること)

### Requirement 14: ASE 固有のグローバル変数

**Objective:** SAP ASE で定義済みのグローバル変数（`@@error`, `@@identity`, `@@rowcount` 等）を特別に認識できること。

#### Acceptance Criteria

1. When `@@error` が入力された場合、the Lexer shall GlobalVar トークンを生成すること
2. When `@@identity` が入力された場合、the Lexer shall GlobalVar トークンを生成すること
3. When `@@rowcount` が入力された場合、the Lexer shall GlobalVar トークンを生成すること
4. When `@@servername` などの他のグローバル変数が入力された場合、the Lexer shall GlobalVar トークンを生成すること
5. The Lexer shall recognize global variable names case-insensitively (グローバル変数名を大文字小文字を区別せずに認識すること)

### Requirement 15: 拡張演算子

**Objective:** SAP ASE T-SQL の拡張演算子（代入演算子、複合代入演算子等）を正しくトークン化すること。

#### Acceptance Criteria

1. When 代入演算子 `=` が入力された場合、the Lexer shall Assign トークンを生成すること
2. When 複合代入演算子（`+=`, `-=`, `*=`, `/=`）が入力された場合、the Lexer shall 対応する複合代入演算子トークンを生成すること
3. When 範囲演算子 `..` が入力された場合、the Lexer shall DotDot トークンを生成すること
4. When ドット `.` が入力された場合、the Lexer shall Dot トークンを生成すること
5. The Lexer shall distinguish each operator type for the Parser (各演算子の種類を Parser が区別できるように区別すること)
