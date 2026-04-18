# ADR-001: SymbolTableを共通基盤にする

## 決定

Definition / References / Hover / Code Actions / Rename の5機能が
すべて `SymbolTable` に依存する設計にする。

## 理由

各機能が独立してパース処理を行うと:

- パース処理の重複（5回同じパース）
- 機能間で認識するシンボルが不一致になるリスク
- パフォーマンスの無駄

`SymbolTable` を共通基盤にすることで、パースは1回、全機能が同じ認識を共有。

## 却下した代替案

- **各機能で独立パース**: 上記の問題に加え、Definition が認識するテーブル名と
  References が認識するテーブル名がずれる可能性がある。
  例: Definition は `Users` を見つけるが References は見つけない。

- **遅延パース（キャッシュなし）**: Phase 1-3 での実際のアプローチ。
  機能ごとに `build_tolerant()` を呼び出すため、Hover → Definition → References の
  操作で3回パースが走る。Phase 5でキャッシングを検討。

## 影響

- 新しい機能（Inlay Hints等）を追加する際は、まず `SymbolTable` に必要な情報を追加し、
  それから機能側で参照する。
- `SymbolTable` のスキーマ変更は全機能に影響するため、慎重に行う。

## 参考

- `crates/ase-ls-core/src/symbol_table/mod.rs` — 実装
- 認識負債監査（2026-04-18）— P0で `db_docs.rs` を分離する際にこの設計が前提
