# LSP Feature Use Case Template

LSP機能追加時に使用するユースケース定義テンプレート。
機能実装の前に、このテンプレートに沿ってシナリオを定義すること。

---

## テンプレート

```markdown
# 機能: [LSP機能名]

## LSP メソッド
`textDocument/[methodName]` または `workspace/[methodName]`

## ペルソナ
[誰が使うか。例: "ASE開発者、500行以上のストアドプロシージャを日常的に書く"]

## 動機
[なぜこの機能が必要か。例: "ASE開発者は長大なプロシージャ内で変数の定義元を頻繁に確認する"]

## ユースケース

### UC-1: [シナリオ名]
**入力:**
```sql
[カーソル位置を | で示すSQL]
```
**期待出力:**
[具体的なLSPレスポンス。例: "CREATE TABLE users (id INT, name VARCHAR(100)) の位置にジャンプ"]

### UC-2: [シナリオ名]
**入力:**
```sql
[SQL]
```
**期待出力:**
[出力]

### UC-3: エッジケース - [ケース名]
**入力:**
```sql
[SQL]
```
**期待出力:**
[None / 空 / gracefulなフォールバック]

## 非スコープ
- [対象外とするケース]
- [将来対応予定のケース]

## 依存
- [前提となる機能やデータ構造]
```

---

## 適用例: Rename

```markdown
# 機能: Rename

## LSP メソッド
`textDocument/rename`, `textDocument/prepareRename`

## ペルソナ
ASE開発者。ストアドプロシージャ内の変数名やテーブル名を一括変更したい。

## ユースケース

### UC-1: ローカル変数のリネーム
**入力:**
```sql
DECLARE @count INT
SET @count = 1
SELECT @count
-- カーソルは2行目 @count 上
```
**期待出力:**
WorkspaceEdit with 3 TextEdits:
- DECLARE @total INT (was @count)
- SET @total = 1 (was @count)
- SELECT @total (was @count)

### UC-2: テーブル名のリネーム（大文字小文字混在）
**入力:**
```sql
CREATE TABLE Users (id INT)
SELECT * FROM users
INSERT INTO USERS (id) VALUES (1)
-- カーソルは1行目 Users 上
```
**期待出力:**
WorkspaceEdit with 3+ TextEdits (Users, users, USERS → 全て customers)

### UC-3: 変数リネームで@プレフィクスなしは拒否
**入力:**
```sql
DECLARE @count INT
-- カーソルは @count 上、new_name = "total" (@なし)
```
**期待出力:**
None (リジェクト)

## 非スコープ
- 複数ファイルにまたがるリネーム（現在は単一ファイルのみ）
- テーブル名がスキーマ名を含む場合 (dbo.users)

## 依存
- find_token_at（トークン特定）
- Lexer（全トークンスキャン）
```

---

## 振り返りからの教訓

### 過去の失敗パターン

| 失敗 | 原因 | ユースケースで防げるか |
|------|------|---------------------|
| Hover で何を表示するか不明 | 表示内容が未定義 | UC-1 で期待出力を明記すれば防げた |
| parse_with_errors の誤解 | APIの挙動を推測 | UC-3 でエッジケースとして定義すれば防げた |
| CREATE UNIQUE INDEX でクラッシュ | Parser未対応を考慮せず | 非スコープに明記すれば防げた |
| lsp-types 0.94 と 0.97 の混乱 | バージョン固有APIを確認せず | 依存セクションに明記すれば防げた |

### 効果的なユースケースの書き方

1. **具体的なSQLを書く** - 抽象的な説明ではなく実際のコード
2. **カーソル位置を明示** - `|` や `-- カーソルここ` で示す
3. **期待出力を具体的に** - 「ジャンプする」ではなく「位置 (line, character) にジャンプ」
4. **エッジケースを含める** - 不完全SQL、空入力、Parser未対応構文
5. **非スコープを明記** - やらないことも明文化する

---

## チェックリスト

機能実装前に以下を完了すること:

- [ ] ペルソナと動機を定義した
- [ ] 正常系ユースケースを2件以上定義した
- [ ] エッジケースユースケースを1件以上定義した
- [ ] 非スコープを明記した
- [ ] 依存関係（前提機能）を特定した
- [ ] ユースケースがテスト可能である（入力→出力が明確）
