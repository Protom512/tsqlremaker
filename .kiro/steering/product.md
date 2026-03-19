# T-SQL Remaker - Product Context

## プロダクトの目的

SAP ASE T-SQL で書かれたストアドプロシージャを、MySQL で実行可能な SQL スクリプトに変換するツール。

## 対象ユーザー

- SAP ASE から MySQL への移行を検討している開発者
- レガシーな T-SQL コードのメンテナンスを行う DBA
- 異なるデータベース間の SQL 移行ツールを必要としているチーム

## コア機能

1. **T-SQL Lexing**: SAP ASE T-SQL の字句解析
2. **T-SQL Parsing**: 構文解析
3. **Common SQL AST**: 方言非依存の中間表現
4. **MySQL Emission**: MySQL SQL コード生成

## 将来の機能

- PostgreSQL Emitter
- SQL Server Emitter
- Oracle Emitter
- その他の方言対応

## 品質目標

- **正確性**: 意味を保持した変換
- **可読性**: 出力される SQL は人間が読みやすく、保守可能であること
- **保守性**: 新しい方言の追加が容易であること
- **テスト可能性**: 各コンポーネントが独立してテスト可能であること
