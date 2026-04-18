# ADR-003: db_docs.rs にASE定義データを集約した理由

## 決定

キーワード、データ型、組み込み関数のドキュメントデータを
単一モジュール `db_docs.rs` に集約する。

## 理由

Phase 1-4の実装では、同じASEドメインデータが3つのモジュールで
**別の構造** で重複定義されていた:

| モジュール | 構造 | 例 |
|-----------|------|-----|
| hover.rs | `HashMap<&str, (&str, &str)>` | (説明, 構文) |
| signature_help.rs | `HashMap<&str, FunctionSignature>` | (label, doc, params) |
| completion.rs | `Vec<CompletionItem>` | (label, kind, detail) |

これにより以下の問題があった:

1. **SUBSTRINGの説明を変える時、3ファイルを修正する必要がある**
2. **hover.rs に897行、signature_help.rs に365行、completion.rs に339行**
   のうち約840行がデータ定義で、ロジックを見つけるのにスクロールが必要
3. **関数リストの不一致リスク**: hoverにはあるがsignature_helpにない関数が
   出る可能性

## 却下した代替案

- **3つの `*_data.rs` に分散**: 同じドメイン知識が3箇所に散らばり、
  整合性を保つコストが増す。「大きな岩から始める」原則に反する。

- **外部JSON/YAMLファイル**: Rustのコンパイル時チェックが効かない。
  型安全性とパフォーマンス（HashMap lookupがO(1)）を優先。

- **マクロベースの定義**: 柔軟だが可読性が下がる。
  `DocEntry` 構造体の明示的な定義の方が理解しやすい。

## 影響

- 新しいASE関数を追加する時は `db_docs.rs` の `FUNCTION_ENTRIES` に1行追加するだけで、
  hover/signature_help/completion の全てに自動的に反映される
- `DocEntry` 構造体がデータの唯一のスキーマ
- 重複名（IDENTITY がキーワードと関数で重複）は
  `FUNCTION_LOOKUP` / `OTHER_LOOKUP` の2段HashMapで解決

## 効果

| ファイル | Before | After | 変化 |
|---------|--------|-------|------|
| hover.rs | 897行 | 386行 | -57% |
| signature_help.rs | 365行 | 243行 | -33% |
| completion.rs | 339行 | 156行 | -54% |
| db_docs.rs (新規) | — | ~1200行 | 単一ソース |

## 参考

- `crates/ase-ls-core/src/db_docs.rs` — 実装
- 認識負債監査（2026-04-18）— P0として実施
