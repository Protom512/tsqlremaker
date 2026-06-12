# TSQL Remaker - Agent Organization

## 組織構造

このプロジェクトは**会社組織型のマルチエージェントシステム**で運用されます。
ユーザー（あなた）は**CEO**として、各エージェントに指示を出します。

```
CEO (ユーザー)
  │
  ├── Project Coordinator (NEW)
  │     バックログ管理、依存関係分析、推奨提示
  │     条件付き拒否権（依存関係逆転時）
  │
  ├── PM (プロダクトマネージャー)
  │     要件定義、優先順位管理、チケット作成
  │
  ├── CTO (最高技術責任者)
  │     技術戦略、アーキテクチャ決定、最終承認
  │     │
  │     ├── VP Engineering
  │     │     リソース管理、見積もり、スケジュール
  │     │     │
  │     │     └── Tech Lead
  │     │         実装調整、コード品質監督
  │     │         │
  │     │         └── Engineers (並列実行可能)
  │     │
  │     └── QC Manager
  │           レビュー調整、品質ゲート管理
  │           │
  │           ├── Reviewer: Architecture (= MAGI MELCHIOR)
  │           ├── Reviewer: Functional (= MAGI BALTHASAR)
  │           └── Reviewer: Maintainability (= MAGI CASPER)
  │
  └── Release Manager
        唯一のコミット・マージ権限
```

## ワークフロー

### 標準パイプライン

```
Phase 0: COORDINATE (NEW)
  CEOの要望 または "next" → Coordinator がバックログをスキャン
  → 依存関係グラフ構築 → 優先度スコアリング → 推奨提示
  → 依存関係逆転時は条件付き拒否権を発動
    ↓
Phase 1: REQUEST
  PM がチケット作成 (GitHub Issue)
    ↓
Phase 2: ESTIMATE
  Tech Lead が見積もり → CTO が承認
    ↓
Phase 3: IMPLEMENT
  Tech Lead → Engineers が TDD 実装（並列可能）
    ↓
Phase 4: COMMIT GATE
  QC Manager → 6レビュアー並列レビュー + Consensus Judge
    ↓
Phase 5: RELEASE
  Release Manager → 最終チェック → コミット → マージ
```

### レビューゲート

レビューは**必ず3名のレビュアーが並列**で実施:
- **Architecture**: コンパイル、Lint、構造、美的品質
- **Functional**: テスト、カバレッジ、要件適合性
- **Maintainability**: ドキュメント、イディオマティック、保守性

**全員GO→GO、1人でもNO-GO→NO-GO（単独拒否権）**

### 修正ループ

NO-GOの場合:
```
QC Manager → Tech Lead に修正指示
  → Engineer が修正
  → QC Manager が再レビュー
  → GO になるまで繰り返し
```

## 使用方法

### 新機能を依頼する

```
CEO: 「Hover機能が欲しい。変数にカーソルを合わせたら型情報を表示して」
  ↓
Workflow: feature-pipeline が自動実行
  0. Coordinator がバックログと照合（依存関係確認）
  1. PM がチケット作成
  2. Tech Lead が見積もり
  3. CTO が承認
  4. Engineer が実装
  5. QC がレビュー（6次元 + Consensus Judge）
  6. Release Manager がコミット
```

### 次の課題を選ぶ

```
CEO: （引数なし または "next"）
  ↓
Workflow: feature-pipeline
  → Coordinator がバックログを自動スキャン
  → 依存関係グラフを構築
  → 上位3件をランキング付きで提示
  → 推奨1件を実行
```

### レビューだけ実行する

```
Workflow: review-gate
  → 現在の変更に対して3レビュアーが並列レビュー
```

### リリースを実行する

```
Workflow: release-pipeline
  → 事前チェック → QC最終レビュー → CTO承認 → コミット → 検証
```

### Spec Driven Development（既存）

```
/kiro:spec-init → /kiro:spec-requirements → /kiro:spec-design
→ /kiro:spec-tasks → /kiro:spec-impl
```

## エージェント一覧

| エージェント | 役割 | 確認事項 |
|-------------|------|---------|
| **Project Coordinator** | バックログ管理、推奨提示 | `.claude/agents/org/coordinator.md` |
| **PM** | 要件定義、チケット管理 | `.claude/agents/org/pm.md` |
| **CTO** | 技術戦略、最終承認 | `.claude/agents/org/cto.md` |
| **VP Engineering** | リソース管理、見積もり | `.claude/agents/org/vp-engineering.md` |
| **Tech Lead** | 実装調整、品質監督 | `.claude/agents/org/tech-lead.md` |
| **QC Manager** | レビュー調整、品質ゲート | `.claude/agents/review/qc-manager.md` |
| **Architecture Reviewer** | 論理・構造検証 | `.claude/agents/review/reviewer-architecture.md` |
| **Functional Reviewer** | テスト・要件検証 | `.claude/agents/review/reviewer-functional.md` |
| **Maintainability Reviewer** | 保守性・将来性検証 | `.claude/agents/review/reviewer-maintainability.md` |
| **Release Manager** | コミット・マージ権限 | `.claude/agents/release/release-manager.md` |

## ワークフロー一覧

| ワークフロー | 用途 |
|-------------|------|
| **feature-pipeline** | 新機能の全工程（調整→企画→リリース） |
| **review-gate** | 多人数レビューゲート |
| **release-pipeline** | リリースパイプライン |
| **org-retrospective** | 組織レトロスペクティブ（フィードバック→体制変更提案） |

## 重要ルール

### コミット権限

- **Release Managerのみ**がコミット・マージ権限を持つ
- Engineer、Tech Lead、QC Managerはコミットできない
- WIPコミットのみ例外的に Tech Lead も可能（`--no-verify`、ローカルのみ）

### レビュー必須

- 全てのライブラリコード変更は**QC Reviewを通過**しなければならない
- テストのみ、ドキュメントのみの変更は軽量レビューで可
- レビューなしのコミットは禁止

### 品質基準

```bash
cargo fmt --all --check          # フォーマット
cargo clippy -- -D warnings      # Lint
cargo nextest run --workspace    # テスト
```

3つすべてパスすることがコミットの前提条件。

## モデルルーティング

タスクの複雑度に応じて、モデルを使い分ける：

| エージェント | モデル | 理由 |
|-------------|--------|------|
| **Project Coordinator** | opus | 依存関係分析、影響評価、プッシュバック判断 |
| **PM** | haiku | テンプレート駆動のチケット作成 |
| **Tech Lead** | sonnet | コード分析、見積もり、タスク分解 |
| **CTO** | opus | アーキテクチャ判断、リスク評価、最終承認 |
| **Engineer** | sonnet | TDD実装 |
| **Gate Reviewers (x6)** | sonnet | 個別レビュー（信頼性・性能・拡張性・統制・安全性・統合） |
| **Consensus Judge** | opus | 6レビューの統合判断、Judgment Calls |
| **Release Manager** | sonnet | 機械的コミット作業 |
| **QC Analyze / Pre-check / Verify** | haiku | ファイル分類、チェック実行、結果確認 |
| **QC Manager (Judge)** | opus | レビュー結果の統合判断 |

## 自己改善メカニズム

### エージェントのボヤキ（フィードバック）

各エージェントはタスク実行中に気づいた問題を `.claude/org-feedback.md` に自発的に追記する。

```
[2026-06-11] [tech-lead] [bottleneck] CTO承認フェーズが直列化のボトルネックになっている
[2026-06-11] [engineer] [tooling] worktree分離がないため並列実装でコンフリクトが起きやすい
```

### 組織レトロスペクティブ

```
Workflow: org-retrospective
  Phase 1: フィードバックログを収集・分類（sonnet）
  Phase 2: 反復テーマを特定し、構造変更を提案（opus）
  Phase 3: CEOに提案を提示（承認後に実行）
```

- **手動トリガー**: `Workflow: org-retrospective`
- **自動推奨**: パイプライン5回実行毎にレトロスペクティブ実施を推奨

### 突然変異的な体制変更

レトロスペクティブは小さな改善だけでなく、大胆な組織変更も提案する：
- 役割の統合・分割・新設
- パイプラインフェーズの再順序
- モデルルーティングの変更
- フィードバックメカニズム自体の改善

## 設定ファイル

- **Permissions**: `.claude/settings.local.json`
- **MCP Servers**: `.claude/mcp.json`
- **Hooks**: `.claude/hooks/`
- **Rules**: `.claude/rules/` (品質ルール群)
- **Skills**: `.claude/skills/` (スキル群)
- **Kiro Spec**: `.claude/agents/kiro/` (Spec Driven Development)
