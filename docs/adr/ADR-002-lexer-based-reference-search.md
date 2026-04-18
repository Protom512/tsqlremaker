# ADR-002: Lexer ベースの参照検索を選択した理由

## 決定

Find References、Rename、Definition の各機能で、
Parser ASTではなくLexer トークンストリームをベースに参照を検索する。

## 理由

### 完全SQLのみを扱うならASTが正解

ASTは構造を理解できるため、正確な参照検索が可能。
しかし、実際の開発現場では:

```sql
SELECT * FROM users  -- ← ここだけを編集中
INSERT INTO users    -- ← 後に書く予定
```

このような**不完全SQL**はParserがエラーを返し、ASTが構築できない。
LSPでは「編集中のSQL」を扱うため、不完全SQLへの耐性が必須。

### Lexerの利点

- **不完全SQLに強い**: `SELECT * FROM us` まで書いた時点でもトークンは取得可能
- **パースエラーで停止しない**: Parserが失敗してもLexerは有効なトークンを返す
- **シンプル**: トークンストリームの線形スキャンで実装可能

### トレードオフ

- **精度が低い**: `users` という文字列がテーブル名か変数名か文脈で判断できない
  → `token_matches_symbol` でキーワードを明示的に列挙して補完（lib.rs:116-143）
- **構造認識不可**: JOIN関係、サブクエリのネスト等は追跡できない
  → Phase 5で tolerant parse による改善を検討

## 却下した代替案

- **Parser ASTベース**: 完全SQLには最適だが、編集中の不完全SQLで機能しない。
  `parse_with_errors()` も実際には `Err` を返す（部分結果は返さない）。
- **Tree-sitter**: インクリメンタルパースが可能だが、T-SQL/Sybase ASE の文法が
  共通ではない。導入コストが高い。
- **正規表現**: 実装が最も簡単だが、文字列リテラル内の誤検出を防げない。

## 影響

- `references.rs`, `rename.rs`, `definition.rs` はすべてLexerベース
- `token_matches_symbol`（lib.rs）がキーワードのマッチングロジックの中心
- Parserが対応していない構文（`EXEC`, `ALTER TABLE` 等）でもトークンレベルで
  参照を検出できる

## 将来の改善

Phase 5で以下を検討:

1. **Symbol Table キャッシング**: `build_tolerant()` の結果をキャッシュし、
   複数機能で再利用（パース1回 → 全機能で参照）
2. **tolerant parse の改善**: 部分的なAST構築で構造認識を強化
3. **`parse_with_errors()` の修正**: 部分結果を返すようにParserを改善
