# 🖊️ WHITEBOARD

> **各エージェントへ**: 作業前に必ずこのファイルを読むこと。

**最終更新:** 2026-06-06 / Session 22 (symbol lookup unification + must_use + tests)

---

## 📊 現在の状態

| 項目 | 状態 |
|------|------|
| **テスト** | 1083 passed, 2 skipped (branch refactor/session-21-code-quality) |
| **Clippy** | clean (`-D warnings`) |
| **Fmt** | clean |
| **Open Issues** | 12 |
| **Open PRs** | 2 (#123 INSERT column list, #124 code quality) |
| **ブランチ** | master + feat/insert-column-list-v2 (#123) + refactor/session-21-code-quality (#124) |

---

## 🔄 Session 22 成果

### コミット（PR #124 ブランチ）
| コミット | 内容 |
|---------|------|
| `e08845e` | refactor(core): add #[must_use] to all remaining pure LSP handler functions |
| `b7416f4` | test(core): add tests for new find_* helpers and resolve_semantic_type |
| `b7b8015` | refactor(core): unify symbol lookups, eliminate to_uppercase() in definition/hover/semantic_tokens |

### コミット（PR #123 ブランチ）
| コミット | 内容 |
|---------|------|
| `0cd82a1` | refactor(code_actions): use make_quickfix helper for INSERT column list action |

### 変更内容
- **symbol_table/mod.rs**: `find_view`, `find_index`, `find_trigger` helpers, `SymbolTable::resolve_semantic_type` method
- **definition.rs**: Replace direct HashMap::get with case-insensitive find_* helpers, eliminate `.to_uppercase()`, add `#[must_use]`
- **hover.rs**: `collect_table_names` returns `Vec<&str>` (zero-alloc borrows), use `find_*` + `eq_ignore_ascii_case` throughout, add `#[must_use]`
- **semantic_tokens.rs**: Delegate to `SymbolTable::resolve_semantic_type` (single allocation), add `#[must_use]`
- **references.rs, rename.rs, folding.rs, symbols.rs, workspace_symbols.rs, signature_help.rs, code_actions.rs**: `#[must_use]` on all public pure functions
- **db_docs/mod.rs**: `#[must_use]` on all 6 lookup/accessor functions
- **code_actions.rs (PR #123)**: Use `make_quickfix` helper for INSERT column list action (CodeRabbit review feedback)

### 削減効果
- 5箇所の `.to_uppercase()` アロケーションを除去（definition + hover）
- `collect_table_names` の `Vec<String>` → `Vec<&str>` でテーブル名ごとのアロケーション除去
- 全28の公開純粋関数に `#[must_use]` 追加完了
- +14テスト（find_view/index/trigger 5件, resolve_semantic_type 6件, definition 4件, CodeRabbit指摘対応既存）

---

## 🔄 Session 21 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `f077159` | refactor(core): extract constants, add #[must_use], remove allocations, add tests |
| `d3ece94` | perf(core): replace to_uppercase() with zero-allocation case-insensitive search |
| `dbaa091` | perf(references): replace to_uppercase() with zero-alloc ends_with_ignore_ascii_case |

### 変更内容
- **code_actions.rs**: `TRY_CATCH_LABEL` const, `find_ignore_ascii_case`/`contains_ignore_ascii_case` utilities, removed 2x `get_line().to_string()` allocations
- **completion.rs**: `KEYWORD_DETAIL` const for repeated "T-SQL Keyword" literal
- **workspace_symbols.rs**: Use `CaseInsensitiveKey.as_str()` instead of per-symbol `name.to_uppercase().contains()`. Macro-based iteration
- **symbol_table/mod.rs**: `as_str()` accessor on `CaseInsensitiveKey`
- **references.rs**: `ends_with_ignore_ascii_case()` replaces `trimmed.to_uppercase()` allocation
- **line_index.rs**: `#[must_use]` on 5 methods
- **diagnostics.rs**: `#[must_use]` on `diagnose()`
- **formatting.rs**: `#[must_use]` on `format()`
- **semantic_tokens.rs**: `#[must_use]` on 2 functions
- **folding.rs/hover.rs**: doc comments on private functions
- **+7 tests**: definition (INDEX, variable in WHILE), rename (case-insensitive table, multi-reference), signature_help (CONVERT, GETDATE, SUBSTRING)

### 削減効果
- 6箇所の不要な `.to_uppercase()` / `.to_string()` アロケーションを除去
- 12箇所の `#[must_use]` 追加で意図しない戻り値破棄を防止
- 1050 tests (+7 from 1043)

---

## 🔄 Session 20 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `b0c8e2c` | refactor(core): remove legacy definition_ranges and folding_ranges functions |
| `6611168` | refactor(core): remove legacy document_symbols and diagnose_source functions |
| `d3144fe` | perf(core): replace .to_uppercase() comparisons with eq_ignore_ascii_case |

### 変更内容
- **definition.rs**: `definition_ranges()` + 14 legacy tests removed (all migrated to `_with_analysis`)
- **folding.rs**: `folding_ranges()` + `fold_begin_end()` + 8 legacy tests removed
- **symbols.rs**: `document_symbols()` removed → `document_symbols_with_analysis()`. 13 tests migrated
- **diagnostics.rs**: `diagnose_source()` + `parse_errors_to_diagnostics()` removed. 10 tests migrated. `DIAGNOSTIC_SOURCE` const
- **server.rs**: Updated to use `document_symbols_with_analysis()`
- **hover.rs**: 6 `.to_uppercase()` == comparisons → `eq_ignore_ascii_case()` (hot path optimization)
- **definition.rs**: 2 `.to_uppercase()` == comparisons → `eq_ignore_ascii_case()`

### PR #123 リベース
- masterの10コミット分をリベース（コンフリクトなし）
- テスト修正: 6テストを旧API(`code_actions`, `get_line_at`, `build_fallback_symbol_table`)から新APIに移行
- 1077 tests passed, clippy clean

### レガシー関数削除完了状況
全モジュールの再パース/再トークナイズ関数を除去完了:
- ✅ completion.rs (Session 19)
- ✅ definition.rs (Session 20)
- ✅ diagnostics.rs (Session 20)
- ✅ folding.rs (Session 20)
- ✅ hover.rs (Session 17-19)
- ✅ references.rs (Session 19)
- ✅ rename.rs (Session 19)
- ✅ symbols.rs (Session 20)
- ✅ semantic_tokens.rs — 常にAnalysis版のみ
- ✅ signature_help.rs — 常にAnalysis版のみ
- 🔒 formatting.rs — 再トークナイズが必要（with_comments(true)＋ゼロコピーToken利用）
- 🔒 folding.rs fold_comments — 再トークナイズが必要（with_comments(true)）

---

## 🔄 Session 19 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `b38c32a` | perf(formatting): return Cow<str> from format_token to avoid allocation |

### 変更内容
- **formatting.rs**: `format_token` の戻り値を `String` → `Cow<'_, str>` に変更
- 識別子・演算子・数字・句読点は `Cow::Borrowed` でゼロアロケーション
- キーワード大文字化・文字列・コメントのみ `Cow::Owned` でアロケーション

---

## 🔄 Session 18 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `bd45619` | refactor(core): use offset_to_range in symbols.rs and code_actions.rs |

### 変更内容
- **symbols.rs**: `span_to_lsp_range` を1行の委譲に簡素化（-10行）
- **code_actions.rs**: SELECT * 展開とINSERT骨組み生成で `offset_to_range` 利用（-25行）

### offset_to_range マイグレーション完了状況
全7モジュールのRange構築を統一完了:
- ✅ diagnostics.rs (2箇所)
- ✅ hover.rs (2箇所)
- ✅ references.rs (2箇所)
- ✅ rename.rs (3箇所)
- ✅ symbol_table/mod.rs (1箇所)
- ✅ symbols.rs (1箇所)
- ✅ code_actions.rs (2箇所)
- 🔒 semantic_tokens.rs — delta計算に個別positionを使うため移行不可
- 🔒 folding.rs — FoldingRange型でRangeではないため移行不可

## 🔄 Session 17 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `124060e` | refactor(core): extract offset_to_range utility, reduce Position/Range boilerplate |

### 変更内容
- **line_index.rs**: `offset_to_range(start, end) -> Range` メソッド追加
- **diagnostics.rs**: `make_star_diagnostic` と `error_range` で利用（2箇所）
- **hover.rs**: 2つのhover関数で利用。`Range` import除去
- **references.rs**: analysis ループ + `token_span_to_range` で利用（2箇所）
- **rename.rs**: 3箇所のTextEdit/PrepareRename で利用。`Range` import除去
- **symbol_table/mod.rs**: `span_to_range` を1行に簡素化

### 削減効果
- **97行削除**（Position/Range構築ボイラープレート除去）
- 5モジュールのRange構築パターンを統一

---

## 🔄 Session 16 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `3c05ed4` | refactor: replace once_cell::sync::Lazy with std::sync::LazyLock |
| `97fe528` | style(core): dedup diagnostics tests, centralize source string, clarify deprecated allow |
| `7fedde2` | docs(lsp): add crate-level and constructor doc comments for ase-ls |

### 変更内容
- **全クレート**: `once_cell::sync::Lazy` → `std::sync::LazyLock` に移行。once_cell依存を完全除去（Cargo.toml workspace + 4 crate）
- **diagnostics.rs**: 重複テスト `test_insert_select_star_warns` を削除。`diagnostic_source()` ヘルパーで "ase-ls" リテラルを一元化
- **workspace_symbols.rs**: `#[allow(deprecated)]` に理由コメント追加（lsp-types 0.94 / tower-lsp 0.20 制約）
- **ase-ls/lib.rs**: クレートレベルdoc追加。`AseLanguageServer::new()` にdoc comment追加。全クレートでmissing_docs警告ゼロ達成

### 品質状況
- **依存削減**: once_cell (1.21) をworkspace依存から完全除去
- **ドキュメント**: `RUSTFLAGS="-W missing_docs"` で全クレート警告ゼロ
- **テスト**: 1146 passed（重複テスト1件削除で-1）

---

## 🔄 Session 15 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `acb5f02` | refactor(core): eliminate allocations, add Debug derives, dedup DocumentStore |

### 変更内容
- **lib.rs**: `token_matches_symbol` で全トークンの `.to_uppercase()` アロケーションを `eq_ignore_ascii_case()` に置換（references/rename/definitionのホットパス改善）
- **analysis.rs**: `DocumentAnalysis` に `Debug` derive追加。`owned_source` 変数をパラメータシャドウイングに簡素化
- **line_index.rs**: `LineIndex` に `Debug` derive追加
- **server.rs**: `DocumentStore::open()`/`update()` の同一メソッドを `upsert()` に統合
- **completion.rs**: `complete_keywords()` の戻り値を `&'static CompletionResponse` に変更（clone回避）。静的参照テスト追加
- **hover.rs**: LocalVarの冗長な@剥がし→再付与を `text` 直接使用に簡素化
- **code_actions.rs**: fmt差分のみ（既存コードのフォーマット修正）

### PR #123 レビュー確認
- 3件のレビュー指摘（HIGH×2, MEDIUM×1, NITPICK×1）は全て既に適用済みを確認
- テスト1176 passed, 2 skipped（PR ブランチ）

### 監査結果（20件の改善候補を特定）
| 優先度 | 件数 | 対応 |
|--------|------|------|
| P1 | 2件 | 1件完了（complete_keywords最適化）、1件保留（range formatting: #60既知の制限） |
| P2 | 7件 | 4件完了（Debug derive, DocumentStore統合, token最適化, hover簡素化）、3件保留（legacy関数削除, Range構築ヘルパー共通化, テストカバレッジ） |
| P3 | 11件 | 未着手（美化的改善、once_cell→LazyLock移行等） |

---

## 🔄 Session 14 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `45feddc` | refactor(analysis): replace O(n²) fallback with O(n) token scan |
| `04378e1` | refactor(core): derive Clone, reduce allocations, add #[must_use] |
| `18e749a` | style(core,server): minor idiomatic cleanups |

### 変更内容
- **analysis.rs**: フォールバックパースをO(n²)の行ごとバイナリサーチからO(n)トークンスキャンに変更。`find_create_table_end()`でCREATE TABLE定義の閉じ括弧を特定し、1回のパースで抽出。`to_ascii_uppercase()`の不要なStringアロケーションも`eq_ignore_ascii_case()`に置換
- **analysis.rs**: 手動`Clone` implを`#[derive(Clone)]`に置換
- **symbol_table/mod.rs**: `#![allow(missing_docs)]`を削除し、全公開struct field・enum variantにdoc comment追加（40件の警告を解消）
- **symbols.rs**: 6箇所のカンマ後空白不足を修正
- **code_actions.rs**: `get_line_at()`の返却型を`String`→`&str`に変更し不要なアロケーションを削除。`#[must_use]`を`find_token_at()`, `get_line()`に追加
- **symbol_table/mod.rs**: `find_table()`, `find_procedure()`, `find_variable()`に`#[must_use]`追加
- **formatting.rs**: インデント文字列を`const INDENT`に抽出
- **server.rs**: リテラル文字列の`.to_string()`を`String::from()`に置換

### 残る改善候補
- **P1**: レガシー関数群の削除（テストの *_with_analysis 移行が必要、50+件）
- **P2**: DocumentAnalysis::get_line() → Option<&str>（保留: 10+呼び出し元に影響、利益小）
- **P2**: WASMクレートのテスト追加
- **P3**: emitterの #[allow(dead_code)] インデントメソッド群（将来フォーマット機能用として意図的）

---

## 🔄 Session 13 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `1f49edc` | perf(emitter): replace buffer.clone() with mem::take in all emitters |
| `c937eab` | refactor(completion): return static reference from complete_all() |
| `fb0d53a` | refactor(server): remove unused Arc<str> from DocumentStore |
| `c9b0125` | refactor(parser): return &[ParseError] instead of Vec from errors() |
| `5cea77a` | perf(symbols): build LineIndex once instead of per-statement |
| `c417592` | refactor(token): simplify token_matches_symbol using is_keyword() |

### PR #123 ブランチ
- リベース完了（masterのTrigger対応を統合、コンフリクト解消）
- レビュー指摘（multi-INSERT回帰テスト、fallback直接テスト）は既に対応済み
- テスト 1176 passed, 2 skipped（master 1142 + PR追加 34）

### 変更内容
- **3エミッター** (sqlite/mysql/postgresql): `self.buffer.clone()` → `std::mem::take(&mut self.buffer)` でヒープコピー回避
- **completion.rs**: `complete_all()` が `&'static CompletionResponse` を返すよう変更。clone箇所を呼び出し元(server.rs)に明示化
- **server.rs**: `DocumentStore` から不要な `Arc<str>` を削除。`HashMap<String, DocumentAnalysis>` に簡略化
- **parser.rs**: `errors()` が `&[ParseError]` を返すよう変更（Lexer APIと統一）。不要な Vec clone を除去
- **symbols.rs**: `span_to_lsp_range()` が毎回 `LineIndex::new()` していたのを、`document_symbols()` で一度だけ構築して参照渡しに変更
- **token/kind.rs**: `Exec`, `Execute` を `is_keyword()` に追加
- **lib.rs**: `token_matches_symbol()` の15行 `matches!` マクロを `is_keyword()` 呼び出しに簡素化

### 監査結果（25件の改善箇所を特定）
| 優先度 | 件数 | 対応 |
|--------|------|------|
| P1 | 5件 | 3件完了（emitter, completion, DocumentStore）、1件保留（legacy関数削除: 50+テスト依存）、1件保留（O(n^2)フォールバック） |
| P2 | 14件 | 3件完了（LineIndex, errors API, keyword統一）、残りはdocument/doc/テスト系 |
| P3 | 5件 | 未着手（emitter dead_code, 重複テスト, URL parse, sysvars, WASMテスト） |

### 残る改善候補
- **P1**: analysis.rs の O(n^2) フォールバックパース（バイナリサーチ的試行に変更）
- **P1**: レガシー関数群の削除（テストの *_with_analysis 移行が必要、50+件）
- **P2**: symbol_table の #![allow(missing_docs)] 解除とdoc comment追加
- **P2**: tsql-parser 公開APIのdoc comment追加
- **P2**: DocumentAnalysis::get_line() → Option<&str> 返却
- **P2**: WASMクレートのテスト追加
- **P3**: emitterの #[allow(dead_code)] インデントメソッド群

---

## 🔄 Session 12 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `583c2c7` | fix(lsp): add Trigger body recursion to folding, hover, diagnostics, code_actions |
| `b2694a7` | fix(references): detect CREATE UNIQUE INDEX and CREATE TRIGGER as definitions |
| `da35bbb` | test(lsp): add 12 integration tests for definition, references, rename, code actions, diagnostics |
| `6289865` | feat(lsp): track Trigger definitions in symbol table for navigation |
| `df7dc01` | feat(lsp): add Trigger to workspace symbol search results |

### 変更内容
- **folding.rs**: `collect_ast_folds` に Trigger ボディ再帰追加。Procedure body も再帰するよう修正（従来はトップレベルフォールドのみ）
- **hover.rs**: `resolve_column_in_statement` に Trigger ボディ再帰追加。Trigger 内のカラムに hover 表示可能に
- **diagnostics.rs**: `collect_select_star_warnings` に Trigger ボディ再帰追加。Trigger 内の SELECT * が警告対象に
- **code_actions.rs**: `find_block_at_offset` に Trigger ボディ再帰追加。Trigger 内の TRY...CATCH code action が動作するように
- **symbol_table**: `TriggerSymbol` 構造体追加。`triggers` フィールドで Trigger 定義を追跡。Go to Definition と Hover が利用可能に
- **workspace_symbols**: Trigger を EVENT kind で workspace symbol 検索結果に含めるように対応
- **references.rs**: (前セッション) CREATE UNIQUE INDEX と CREATE TRIGGER を定義として検出
- **テスト**: +5 テスト（folding 2, diagnostics 1, hover 1, code_actions 1）+ 12 統合テスト + 5 Trigger symbol tableテスト
- **ブランチ整理**: `feat/114-alter-table-parser`（古い）と `feat/code-action-insert-column-list`（旧PR）を削除。リモート旧ブランチも削除

### 調査結果
- TODO/FIXME/HACK マーカーはコード内に存在しない（テスト内アサーションのみ）
- 全 `#[allow(dead_code)]` は「将来のフォーマット機能用」として意図的に文書化済み
- 全 wildcard `_ => None` / `_ => {}` は意図的で正しいことを確認
- rename, semantic_tokens, completion, definition, signature_help, formatting はトークンレベル処理のため Trigger 再帰不要
- doc comments は全公開API関数に既に付与されていることを確認

---

## 🔄 Session 11 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `fa2bbce` | feat(parser): implement Display for DataType + fix hover output |

### コミット（PR #123 ブランチ）
| コミット | 内容 |
|---------|------|
| `5aab4d5` | test(lsp): add regression + direct unit tests for INSERT column list |

### 変更内容
- **tsql-parser/ast/ddl.rs**: `DataType` enum に `Display` trait を実装。SQL標準の形式（`INT`, `VARCHAR(100)`, `DECIMAL(10,2)`）を出力。従来の `Debug`（`Int`, `Varchar(Some(100))`）をユーザー向け出力から排除
- **hover.rs**: 全 `{:?}` を `{}` に置換し、`DataType` の Display を使用。TryCatch パターンを `chain().find_map()` に統一
- **code_actions.rs**（PR #123）: CodeRabbit レビュー指摘に対応。9テスト追加（複数INSERT回帰テスト、`resolve_insert_stmt_end`/`find_values_token_start`/`build_fallback_symbol_table` の直接テスト）

## 🔄 Session 10 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `5692413` | fix(analysis): avoid O(n) LineIndex recomputation in Clone impl |
| `9446695` | refactor(code_actions): extract make_quickfix/make_refactor helpers |
| `aa6c4ce` | refactor(workspace_symbols): deduplicate symbol iteration code |
| `680659e` | test(completion): add 3 edge case tests for completion module |
| `4e06ca7` | test(parser): add 12 to_common.rs conversion tests |
| `f71cea4` | docs(emitter): clarify dead_code indentation helpers as future use |

### 変更内容
- **analysis.rs**: `DocumentAnalysis::clone()` が `LineIndex::new()` を再計算していたバグを修正（`derive(Clone)` + 直接clone）
- **code_actions.rs**: 6箇所のCodeAction構築ボイラープレートを `make_quickfix()` / `make_refactor()` ヘルパーに抽出。`InsertSource` のワイルドカードマッチを明示的マッチに修正
- **workspace_symbols.rs**: `push_matching()` + `collect_symbols()` ヘルパーで~80行の重複コードを削除（171行→87行）
- **completion.rs**: 3テスト追加（システム変数、重複ラベル、構文エッジケース）
- **common_ast_conversion.rs**: 12テスト追加（INSERT SELECT, CASE, ORDER/GROUP/HAVING, EXISTS, unary minus, NOT BETWEEN/LIKE, hex literal, BatchSeparator, CREATE方言固有, UPDATE/DELETE FROM方言固有, サブクエリ, 制御フロー）
- **postgresql-emitter/sqlite-emitter**: `#[allow(dead_code)]` に「将来のフォーマット機能用」コメント追加

## 🔄 Session 8 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `18be817` | fix(parser): prevent infinite loop in parse_with_errors error recovery |

### 修正内容
- **根本原因**: `parse_with_errors` のエラー回復で `synchronize()` が同期ポイント（END等）で停止してもトークンを消費しないため、`END` が `parse_statement` の有効開始トークンでない場合に無限ループ発生
- **修正**: `synchronize()` 後に必ず1トークン消費して前進を保証
- **影響**: 複数行の `BEGIN TRY...END TRY BEGIN CATCH...END CATCH` 入力で `DocumentAnalysis::new` がスタックオーバーフローしていた問題が解消

---

## 🔄 Session 7 成果

### コミット（master直接）
| コミット | 内容 |
|---------|------|
| `8d08592` | perf(analysis): remove unused tokens_with_comments computation |
| `6daa25d` | test(lsp): add 24 tests for folding, rename, definition, references coverage |
| `7eddb9b` | test(lsp): add 4 diagnostics tests for empty source and multi-statement coverage |
| `8705faa` | test(lsp): add 12 tests for diagnostics, formatting, and hover coverage |

### 変更内容
- **analysis.rs**: `tokens_with_comments` の不要なLexer二重実行を削除（1パス節約）
- **folding.rs**: +7テスト（TRY...CATCH, CREATE PROCEDURE, 単一行IF, 不一致BEGIN, 複数コメント, ネストWHILE, 空ソース）
- **rename.rs**: +7テスト（Analysis版 variable/table/empty/beyond-end/reject, placeholder）
- **definition.rs**: +5テスト（Analysis版 variable/table/empty/no-token/procedure）
- **references.rs**: +5テスト（Analysis版 variable/table/empty/no-token/exclude-declaration）
- **diagnostics.rs**: +4テスト（空ソース, 複数SELECT *, 位置インデックス）
- **formatting.rs**: +5テスト（空ソース, ドット表記, 括弧, HexString, 変更なし）
- **hover.rs**: +3テスト（空ソース, GETDATE関数, 範囲外位置）

### 発見した既存バグ
- ~~DocumentAnalysis スタックオーバーフロー~~ → **Session 8 で修正**（parser infinite loop）

---

## 🔀 申し送り（次セッションへ）

### 優先度高
1. **PR #123** (INSERT column list): レビュー指摘対応済み + カバレッジ改善プッシュ済み。マージ待ち。

### 優先度中
2. **DocumentAnalysis スタックオーバーフロー**: `BEGIN TRY\n    SELECT 1\n    SELECT 2\nEND TRY\nBEGIN CATCH\n    SELECT -1\nEND CATCH` で DocumentAnalysis::new がクラッシュ。根本原因調査が必要。
3. **#82 Parser error recovery**: 現在最初のエラーで停止。build_tolerant()で部分的に対応済み。
4. **#75 SQLite converter**: function_mapperは抽出済み。コンバータパターンの一般化。

### 残りのOpen Issues (11件)
| Issue | 分類 | 難易度 |
|-------|------|--------|
| #82 | Parser error recovery | Large |
| #81 | LSP configuration | Large |
| #75 | SQLite converter | Medium |
| #70 | Cross-file definition | Large |
| #65 | Multi-file workspace | Large |
| #61 | WASM AST conversion | Large |
| #60 | Range formatting | Medium |
| #54 | Context-aware completion | Large |
| #52 | Incremental sync | Large |
| #119 | Code Lens support | Medium |
| #118 | Inlay Hints support | Medium |

---

## 🏗️ アーキテクチャノート

### 依存関係 (更新なし)
```
ase-ls (tower-lsp 0.20, lsp-types 0.94.1)
  └── ase-ls-core (lsp-types 0.94.1)
        └── tsql-parser
              └── tsql-lexer (tsql-token)
```

### 結合度分析 (2026-05-30 cargo coupling)
- **Grade C** (Score 0.88): 4 High, 40 Medium issues
- 主な問題: tsql-token (68 dependents), tsql-parser (86 dependents), parser module (171 functions)
- 改善は長期的なリファクタリングとして計画が必要

---

## 📋 過去セッション成果（要約）

| Session | コミット数 | テスト数 | 主な成果 |
|---------|-----------|---------|---------|
| 3 | 1 | — | db_docs monolith split (#71) |
| 5 | 4 | 1018 | signature help fix (#77), dead code removal |
| 6 | 4 | 1049 | 4 issues closed, coverage +2.38% |
| 7 | 4 | 1085 | dead code removal, +36 tests across 8 modules |
| 8 | 1 | 1085 | fix parser infinite loop in error recovery |
| 9 | 2 | 1097 | +12 tests for lib.rs, symbol_table, line_index |
| 10 | 6 | 1115 | bug fix (analysis Clone), dedup 2 modules, +15 tests |
| 11 | 1 | 1115 | DataType Display impl + PR #123 review tests |
| 12 | 6 | 1142 | Trigger full support (recursion + symbol table + workspace symbols), +18 tests, branch cleanup |
| 13 | 6 | 1142 | Codebase quality audit (25 findings), 6 performance/refactor commits, PR #123 rebase |
