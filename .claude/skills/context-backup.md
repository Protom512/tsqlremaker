# Context Backup and Clear

現在のセッションコンテキストをMarkdownファイルにバックアップしてからクリアします。

## 使い方

```bash
/context-backup
```

## 実行内容

1. 現在の日時でバックアップファイルを作成
2. 以下の内容をMarkdownに書き出し
   - プロジェクト情報
   - Git状態
   - 最近のコミット
   - steeringドキュメント
   - アクティブなスペックの状態
3. セッションをクリア

## バックアップ先

`~/.claude/backups/context-{YYYY-MM-DD-HHMMSS}.md`

※ Windowsの場合: `C:\Users\{username}\.claude\backups\`

---

## 自動実行hookについて

Claude Codeには以下のhookがありますが、「コンテキストサイズを監視」する機能はありません：

### 利用可能なhook

| Hook | タイミング |
|------|-----------|
| `command-start-hook` | コマンド実行前 |
| `command-output-hook` | コマンド出力後 |
| `command-end-hook` | コマンド終了後 |

### 現実的な方法

1. **手動実行**: コンテキストが肥大化したと感じたら `/context-backup`
2. **特定コマンド前にhook**: `/commit` の前にバックアップする等

### 設定例 (.claude/settings.local.json)

```json
{
  "hooks": {
    "command-start-hook": "if [ \"$CLAUDE_CMD\" = \"/commit\" ]; then /context-backup; fi"
  }
}
```

※ 残念ながらhook内での条件分岐は制限されているため、実用的には**手動実行**を推奨します。
