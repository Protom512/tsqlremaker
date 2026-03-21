# Software Design Document (SDD)
# TSQL-to-MySQL Dialect Converter

**プロジェクト名**: tsqlremaker  
**バージョン**: 0.2.0  
**作成日**: 2026-01-19  
**最終更新**: 2026-01-19

---

## 目次

1. [概要と目標](#1-概要と目標)
2. [アーキテクチャ設計](#2-アーキテクチャ設計)
3. [詳細コンポーネント設計](#3-詳細コンポーネント設計)
4. [SAP ASE TSQL 固有の処理](#4-sap-ase-tsql-固有の処理)
5. [エラーハンドリング戦略](#5-エラーハンドリング戦略)
6. [テスト戦略](#6-テスト戦略)
7. [フェーズ別実装計画](#7-フェーズ別実装計画)
8. [将来の検討事項](#8-将来の検討事項)
9. [追加要件（v0.2.0）](#9-追加要件v020)

---

## 1. 概要と目標

### 1.1 プロジェクト概要

**tsqlremaker** は、SAP ASE (Sybase Adaptive Server Enterprise) の T-SQL 方言で記述された SQL を、MySQL 互換の SQL に変換する Rust 製トランスパイラです。

変換パイプラインは以下の3段階で構成されます：

```
┌─────────────────────┐
│  SAP ASE T-SQL      │  ソース（方言固有）
│  (Input Source)     │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  Common SQL AST     │  中間表現（方言非依存）
│  (Abstract IR)      │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│  MySQL Dialect      │  出力（ターゲット固有）
│  (Output Target)    │
└─────────────────────┘
```

### 1.2 プロジェクト目標

| 目標 | 説明 |
|------|------|
| **正確な変換** | SAP ASE T-SQL の主要構文を正確に MySQL に変換 |
| **高品質なエラーメッセージ** | 変換不可能な構文に対する明確な警告・エラー |
| **拡張可能なアーキテクチャ** | 将来の方言追加（PostgreSQL等）に対応可能な設計 |
| **高パフォーマンス** | 大規模 SQL ファイルの高速処理 |
| **クロスプラットフォーム** | Linux, macOS, 将来的に WASM/ARM 対応 |

### 1.3 成功基準

- [ ] SAP ASE T-SQL の一般的なクエリ（SELECT, INSERT, UPDATE, DELETE）を正しく変換
- [ ] ストアドプロシージャの基本構文を変換
- [ ] データ型マッピングの正確な実装
- [ ] 関数マッピング（CONVERT, DATEADD 等）の実装
- [ ] 変換不可能な構文に対する明確な警告出力

### 1.4 スコープ外（Non-Goals）

| 非目標 | 理由 |
|--------|------|
| **ランタイム実行** | SQL の実行は行わない（変換のみ） |
| **データ移行** | データベースの実データ移行は対象外 |
| **完全な互換性** | 100% の互換性は保証しない（ASE 固有機能は警告） |
| **GUI ツール** | CLI ツールとして提供、GUI は将来検討 |
| **リアルタイム変換** | バッチ処理が主、ストリーミングは将来検討 |

---

## 2. アーキテクチャ設計

### 2.1 高レベルシステムアーキテクチャ

```
                              ┌──────────────────────────────────────┐
                              │           tsqlremaker                │
                              └──────────────────────────────────────┘
                                               │
           ┌───────────────────────────────────┼───────────────────────────────────┐
           │                                   │                                   │
           ▼                                   ▼                                   ▼
┌─────────────────────┐        ┌─────────────────────┐        ┌─────────────────────┐
│     tsql-lexer      │        │    tsql-parser      │        │   mysql-emitter     │
│   (字句解析器)       │        │   (構文解析器)       │        │   (コード生成器)     │
│                     │        │                     │        │                     │
│  Input String       │        │  Token Stream       │        │  Common SQL AST     │
│       ↓             │───────▶│       ↓             │───────▶│       ↓             │
│  Token Stream       │        │  ASE AST            │        │  MySQL SQL String   │
└─────────────────────┘        └──────────┬──────────┘        └─────────────────────┘
                                          │
                                          ▼
                               ┌─────────────────────┐
                               │    common-sql       │
                               │   (共通 AST 定義)    │
                               │                     │
                               │  - Statement nodes  │
                               │  - Expression nodes │
                               │  - Type definitions │
                               └─────────────────────┘
```

### 2.2 コンポーネント責務

| Crate | 責務 | 依存関係 |
|-------|------|----------|
| `tsql-lexer` | T-SQL ソースの字句解析、Token 生成 | `tsql-token` |
| `tsql-token` | Token 型定義、キーワード解決 | なし |
| `tsql-parser` | Token Stream からの AST 構築 | `tsql-lexer`, `common-sql` |
| `common-sql` | 方言非依存の AST 定義 | なし |
| `mysql-emitter` | Common SQL AST から MySQL SQL 生成 | `common-sql` |
| `tsql-cli` | CLI インターフェース | 全 crate |

### 2.3 データフロー

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Source    │    │   Lexer     │    │   Parser    │    │ Transformer │    │   Emitter   │
│   String    │───▶│  Tokenize   │───▶│  Parse AST  │───▶│  Transform  │───▶│  Generate   │
│             │    │             │    │             │    │             │    │   MySQL     │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
                          │                  │                  │                  │
                          ▼                  ▼                  ▼                  ▼
                   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
                   │   Token     │    │   ASE AST   │    │ Common AST  │    │   MySQL     │
                   │   Stream    │    │   (typed)   │    │ (abstract)  │    │   String    │
                   └─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
```

### 2.4 Crate 組織構造

```
tsqlremaker/
├── Cargo.toml                          # Workspace root
├── crates/
│   ├── tsql-token/                     # Token 定義
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── token.rs                # TokenKind enum
│   │       ├── keyword.rs              # Keyword lookup
│   │       └── span.rs                 # Position tracking
│   │
│   ├── tsql-lexer/                     # 字句解析器
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── lexer.rs                # Main lexer
│   │       ├── cursor.rs               # Character cursor
│   │       └── error.rs                # Lexer errors
│   │
│   ├── tsql-parser/                    # 構文解析器
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── parser.rs               # Main parser
│   │       ├── expr.rs                 # Expression parsing
│   │       ├── stmt.rs                 # Statement parsing
│   │       └── error.rs                # Parser errors
│   │
│   ├── common-sql/                     # 共通 AST
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── ast/
│   │       │   ├── mod.rs
│   │       │   ├── stmt.rs             # Statement AST
│   │       │   ├── expr.rs             # Expression AST
│   │       │   ├── types.rs            # Data types
│   │       │   └── name.rs             # Identifiers
│   │       └── visitor.rs              # AST visitor trait
│   │
│   ├── mysql-emitter/                  # MySQL コード生成
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── emitter.rs              # Code generation
│   │       ├── mapping/
│   │       │   ├── mod.rs
│   │       │   ├── types.rs            # Type mapping
│   │       │   └── functions.rs        # Function mapping
│   │       └── compat.rs               # Compatibility checks
│   │
│   └── tsql-cli/                       # CLI バイナリ
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
│
├── docs/
│   └── SDD.md                          # This document
│
└── tests/
    └── fixtures/                       # Test SQL files
        ├── ase/                        # ASE input files
        └── mysql/                      # Expected MySQL output
```

---

## 3. 詳細コンポーネント設計

### 3.1 Lexer (tsql-lexer)

#### 3.1.1 TokenKind 定義

```rust
/// Token種別の列挙型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    // ==================== Keywords ====================
    // DML
    Select, Insert, Update, Delete, Merge,
    From, Where, Join, Inner, Outer, Left, Right, Full, Cross,
    On, And, Or, Not, In, Exists, Between, Like, Is, Null,
    Order, By, Asc, Desc, Group, Having, Union, Intersect, Except,
    Distinct, All, Top, Limit, Offset, Fetch, First, Next, Rows, Only,
    
    // DDL
    Create, Alter, Drop, Truncate,
    Table, Index, View, Procedure, Proc, Function, Trigger,
    Database, Schema, Constraint, Primary, Foreign, Key, References,
    Unique, Check, Default, Identity, Autoincrement,
    
    // Control Flow
    If, Else, Begin, End, While, Return, Break, Continue,
    Case, When, Then, Else_, End_,
    Try, Catch, Throw, Raiserror,
    
    // Transaction
    Commit, Rollback, Transaction, Tran, Save, Savepoint,
    
    // Types
    Int, Integer, Smallint, Tinyint, Bigint,
    Float, Real, Double, Decimal, Numeric, Money, Smallmoney,
    Char, Varchar, Text, Nchar, Nvarchar, Ntext,
    Binary, Varbinary, Image,
    Date, Time, Datetime, Smalldatetime, Timestamp,
    Bit, Uniqueidentifier,
    
    // Misc Keywords
    As, Set, Declare, Exec, Execute, Into, Values, Output,
    Cursor, Open, Close, Deallocate, Fetch,
    Grant, Revoke, Deny,
    Print, Waitfor, Goto, Label,
    
    // ==================== Literals ====================
    Ident,              // 識別子
    QuotedIdent,        // [identifier] or "identifier"
    Number,             // 整数
    Float_,             // 浮動小数点
    String_,            // 'string'
    NString,            // N'unicode string'
    HexString,          // 0xABCD
    
    // ==================== Operators ====================
    // Comparison
    Eq,                 // =
    Ne,                 // <> or !=
    Lt,                 // <
    Gt,                 // >
    Le,                 // <=
    Ge,                 // >=
    
    // Arithmetic
    Plus,               // +
    Minus,              // -
    Star,               // *
    Slash,              // /
    Percent,            // %
    
    // Bitwise
    Ampersand,          // &
    Pipe,               // |
    Caret,              // ^
    Tilde,              // ~
    
    // Assignment
    Assign,             // =
    PlusAssign,         // +=
    MinusAssign,        // -=
    StarAssign,         // *=
    SlashAssign,        // /=
    
    // String
    Concat,             // + (context-dependent) or ||
    
    // ==================== Punctuation ====================
    LParen,             // (
    RParen,             // )
    LBracket,           // [
    RBracket,           // ]
    LBrace,             // {
    RBrace,             // }
    Comma,              // ,
    Semicolon,          // ;
    Colon,              // :
    Dot,                // .
    DotDot,             // ..
    At,                 // @
    AtAt,               // @@
    Hash,               // #
    HashHash,           // ##
    Dollar,             // $
    
    // ==================== Special ====================
    Whitespace,
    Newline,
    LineComment,        // -- comment
    BlockComment,       // /* comment */
    
    Eof,
    Unknown,
}
```

#### 3.1.2 Token 構造体

```rust
/// 位置情報
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start: start as u32,
            end: end as u32,
        }
    }
    
    pub fn len(&self) -> usize {
        (self.end - self.start) as usize
    }
}

/// Zero-copy Token
#[derive(Debug, Clone)]
pub struct Token<'src> {
    pub kind: TokenKind,
    pub text: &'src str,    // ソースへの参照（コピーなし）
    pub span: Span,
}

impl<'src> Token<'src> {
    pub fn new(kind: TokenKind, text: &'src str, span: Span) -> Self {
        Self { kind, text, span }
    }
    
    pub fn eof() -> Self {
        Self {
            kind: TokenKind::Eof,
            text: "",
            span: Span::new(0, 0),
        }
    }
}
```

#### 3.1.3 Lexer 実装

```rust
/// 字句解析器
pub struct Lexer<'src> {
    input: &'src str,
    chars: std::str::CharIndices<'src>,
    current: Option<(usize, char)>,
    line: u32,
    column: u32,
}

impl<'src> Lexer<'src> {
    pub fn new(input: &'src str) -> Self {
        let mut chars = input.char_indices();
        let current = chars.next();
        Self {
            input,
            chars,
            current,
            line: 1,
            column: 1,
        }
    }
    
    /// 次のトークンを取得
    pub fn next_token(&mut self) -> LexResult<Token<'src>> {
        self.skip_whitespace_and_comments();
        
        let Some((start, ch)) = self.current else {
            return Ok(Token::eof());
        };
        
        let kind = match ch {
            // 単一文字演算子・区切り
            '(' => { self.bump(); TokenKind::LParen }
            ')' => { self.bump(); TokenKind::RParen }
            '[' => return self.lex_quoted_ident(),
            ']' => { self.bump(); TokenKind::RBracket }
            ',' => { self.bump(); TokenKind::Comma }
            ';' => { self.bump(); TokenKind::Semicolon }
            ':' => { self.bump(); TokenKind::Colon }
            '+' => self.lex_plus_or_assign(),
            '-' => self.lex_minus_or_comment(),
            '*' => self.lex_star_or_assign(),
            '/' => self.lex_slash_or_comment(),
            '%' => { self.bump(); TokenKind::Percent }
            '=' => { self.bump(); TokenKind::Eq }
            '<' => self.lex_less_than(),
            '>' => self.lex_greater_than(),
            '!' => self.lex_bang(),
            '&' => { self.bump(); TokenKind::Ampersand }
            '|' => { self.bump(); TokenKind::Pipe }
            '^' => { self.bump(); TokenKind::Caret }
            '~' => { self.bump(); TokenKind::Tilde }
            '.' => self.lex_dot(),
            '@' => self.lex_at(),
            '#' => self.lex_hash(),
            
            // 文字列リテラル
            '\'' => self.lex_string()?,
            '"' => self.lex_double_quoted()?,
            
            // 数値
            '0'..='9' => self.lex_number()?,
            
            // 識別子・キーワード
            'N' | 'n' if self.peek() == Some('\'') => self.lex_nstring()?,
            c if is_ident_start(c) => self.lex_ident_or_keyword(),
            
            _ => return Err(LexError::unexpected_char(ch, self.pos())),
        };
        
        let end = self.pos();
        Ok(Token {
            kind,
            text: &self.input[start..end],
            span: Span::new(start, end),
        })
    }
    
    // ... 以下、各 lex_* メソッドの実装
}
```

#### 3.1.4 トークン対応表

| カテゴリ | 対応するトークン | 実装優先度 |
|----------|------------------|------------|
| **基本キーワード** | SELECT, FROM, WHERE, INSERT, UPDATE, DELETE | Phase 1 |
| **JOIN** | JOIN, INNER, LEFT, RIGHT, FULL, OUTER, CROSS, ON | Phase 1 |
| **集約** | GROUP BY, HAVING, ORDER BY, DISTINCT | Phase 1 |
| **演算子** | =, <>, <, >, <=, >=, +, -, *, /, % | Phase 1 |
| **論理** | AND, OR, NOT, IN, EXISTS, BETWEEN, LIKE, IS NULL | Phase 1 |
| **リテラル** | 数値, 文字列 ('...'), N'...' | Phase 1 |
| **識別子** | 通常, [quoted], "quoted", @var, @@global, #temp, ##global_temp | Phase 1 |
| **コメント** | -- line, /* block */ | Phase 1 |
| **DDL** | CREATE, ALTER, DROP, TABLE, INDEX, VIEW, PROCEDURE | Phase 2 |
| **制御フロー** | IF, ELSE, BEGIN, END, WHILE, RETURN | Phase 2 |
| **トランザクション** | BEGIN TRAN, COMMIT, ROLLBACK | Phase 3 |
| **カーソル** | CURSOR, OPEN, FETCH, CLOSE, DEALLOCATE | Phase 4 |

---

### 3.2 Token Types (tsql-token)

#### 3.2.1 キーワード解決

```rust
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// キーワードマップ（静的初期化、1回のみ構築）
static KEYWORDS: Lazy<HashMap<&'static str, TokenKind>> = Lazy::new(|| {
    let mut m = HashMap::with_capacity(150);
    
    // DML Keywords
    m.insert("select", TokenKind::Select);
    m.insert("insert", TokenKind::Insert);
    m.insert("update", TokenKind::Update);
    m.insert("delete", TokenKind::Delete);
    m.insert("from", TokenKind::From);
    m.insert("where", TokenKind::Where);
    m.insert("join", TokenKind::Join);
    m.insert("inner", TokenKind::Inner);
    m.insert("left", TokenKind::Left);
    m.insert("right", TokenKind::Right);
    m.insert("outer", TokenKind::Outer);
    m.insert("on", TokenKind::On);
    m.insert("and", TokenKind::And);
    m.insert("or", TokenKind::Or);
    m.insert("not", TokenKind::Not);
    m.insert("in", TokenKind::In);
    m.insert("exists", TokenKind::Exists);
    m.insert("between", TokenKind::Between);
    m.insert("like", TokenKind::Like);
    m.insert("is", TokenKind::Is);
    m.insert("null", TokenKind::Null);
    m.insert("as", TokenKind::As);
    m.insert("order", TokenKind::Order);
    m.insert("by", TokenKind::By);
    m.insert("asc", TokenKind::Asc);
    m.insert("desc", TokenKind::Desc);
    m.insert("group", TokenKind::Group);
    m.insert("having", TokenKind::Having);
    m.insert("distinct", TokenKind::Distinct);
    m.insert("top", TokenKind::Top);
    m.insert("union", TokenKind::Union);
    
    // DDL Keywords
    m.insert("create", TokenKind::Create);
    m.insert("alter", TokenKind::Alter);
    m.insert("drop", TokenKind::Drop);
    m.insert("table", TokenKind::Table);
    m.insert("index", TokenKind::Index);
    m.insert("view", TokenKind::View);
    m.insert("procedure", TokenKind::Procedure);
    m.insert("proc", TokenKind::Proc);
    m.insert("function", TokenKind::Function);
    m.insert("trigger", TokenKind::Trigger);
    
    // Control Flow
    m.insert("if", TokenKind::If);
    m.insert("else", TokenKind::Else);
    m.insert("begin", TokenKind::Begin);
    m.insert("end", TokenKind::End);
    m.insert("while", TokenKind::While);
    m.insert("return", TokenKind::Return);
    m.insert("case", TokenKind::Case);
    m.insert("when", TokenKind::When);
    m.insert("then", TokenKind::Then);
    
    // Types
    m.insert("int", TokenKind::Int);
    m.insert("integer", TokenKind::Integer);
    m.insert("varchar", TokenKind::Varchar);
    m.insert("char", TokenKind::Char);
    m.insert("datetime", TokenKind::Datetime);
    m.insert("decimal", TokenKind::Decimal);
    m.insert("numeric", TokenKind::Numeric);
    m.insert("money", TokenKind::Money);
    m.insert("text", TokenKind::Text);
    m.insert("image", TokenKind::Image);
    
    // Misc
    m.insert("exec", TokenKind::Exec);
    m.insert("execute", TokenKind::Execute);
    m.insert("declare", TokenKind::Declare);
    m.insert("set", TokenKind::Set);
    m.insert("print", TokenKind::Print);
    
    m
});

impl TokenKind {
    /// 識別子からキーワードを解決（大文字小文字非区別）
    pub fn from_ident(s: &str) -> Self {
        KEYWORDS
            .get(s.to_lowercase().as_str())
            .copied()
            .unwrap_or(TokenKind::Ident)
    }
    
    pub fn is_keyword(&self) -> bool {
        !matches!(self, 
            TokenKind::Ident | 
            TokenKind::QuotedIdent | 
            TokenKind::Number | 
            TokenKind::Float_ |
            TokenKind::String_ |
            TokenKind::NString |
            TokenKind::Unknown |
            TokenKind::Eof
        )
    }
}
```

#### 3.2.2 Span/Position トラッキング

```rust
/// ソースコード上の位置情報
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub line: u32,      // 1-indexed
    pub column: u32,    // 1-indexed
    pub offset: u32,    // 0-indexed byte offset
}

/// エラーメッセージ用の詳細位置
#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub span: Span,
    pub start_loc: Location,
    pub end_loc: Location,
    pub source_line: String,    // エラー行のソースコード
}

impl SourceLocation {
    /// エラーメッセージ用のコンテキスト表示
    pub fn display_context(&self) -> String {
        let line_num = format!("{:4} | ", self.start_loc.line);
        let marker = format!(
            "{:width$}{}",
            "",
            "^".repeat((self.span.end - self.span.start) as usize),
            width = line_num.len() + self.start_loc.column as usize - 1
        );
        format!("{}{}\n{}", line_num, self.source_line, marker)
    }
}
```

---

### 3.3 Parser (tsql-parser)

#### 3.3.1 文法仕様アプローチ

**採用手法**: 手書き再帰下降パーサー + Pratt パーサー（式解析用）

**理由**:
1. SQL の文脈依存性への対応が容易
2. エラーリカバリの実装が柔軟
3. パフォーマンスが予測可能
4. デバッグが容易

#### 3.3.2 AST ノード型

```rust
// ==================== Statement AST ====================

/// SQL Statement の列挙
#[derive(Debug, Clone)]
pub enum Statement {
    // DML
    Select(SelectStatement),
    Insert(InsertStatement),
    Update(UpdateStatement),
    Delete(DeleteStatement),
    
    // DDL
    CreateTable(CreateTableStatement),
    CreateProcedure(CreateProcedureStatement),
    CreateFunction(CreateFunctionStatement),
    CreateView(CreateViewStatement),
    CreateIndex(CreateIndexStatement),
    AlterTable(AlterTableStatement),
    DropTable(DropTableStatement),
    
    // Control Flow
    If(IfStatement),
    While(WhileStatement),
    Begin(BeginStatement),
    Return(ReturnStatement),
    
    // Transaction
    BeginTransaction(BeginTransactionStatement),
    Commit(CommitStatement),
    Rollback(RollbackStatement),
    
    // Variable
    Declare(DeclareStatement),
    Set(SetStatement),
    
    // Misc
    Execute(ExecuteStatement),
    Print(PrintStatement),
    Block(Vec<Statement>),  // BEGIN...END block
}

/// SELECT 文
#[derive(Debug, Clone)]
pub struct SelectStatement {
    pub distinct: bool,
    pub top: Option<TopClause>,
    pub columns: Vec<SelectColumn>,
    pub from: Option<FromClause>,
    pub joins: Vec<JoinClause>,
    pub where_clause: Option<Expr>,
    pub group_by: Vec<Expr>,
    pub having: Option<Expr>,
    pub order_by: Vec<OrderByItem>,
    pub union: Option<Box<UnionClause>>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TopClause {
    pub count: Expr,
    pub percent: bool,
    pub with_ties: bool,
}

#[derive(Debug, Clone)]
pub enum SelectColumn {
    Expr { expr: Expr, alias: Option<Ident> },
    AllColumns,                          // *
    TableAllColumns { table: Ident },    // table.*
}

/// FROM 句
#[derive(Debug, Clone)]
pub struct FromClause {
    pub tables: Vec<TableReference>,
}

#[derive(Debug, Clone)]
pub enum TableReference {
    Table {
        name: ObjectName,
        alias: Option<Ident>,
        hints: Vec<TableHint>,
    },
    Subquery {
        query: Box<SelectStatement>,
        alias: Ident,
    },
    TableFunction {
        function: FunctionCall,
        alias: Option<Ident>,
    },
}

/// JOIN 句
#[derive(Debug, Clone)]
pub struct JoinClause {
    pub join_type: JoinType,
    pub table: TableReference,
    pub condition: Option<JoinCondition>,
}

#[derive(Debug, Clone, Copy)]
pub enum JoinType {
    Inner,
    LeftOuter,
    RightOuter,
    FullOuter,
    Cross,
}

#[derive(Debug, Clone)]
pub enum JoinCondition {
    On(Expr),
    Using(Vec<Ident>),
}
```

#### 3.3.3 Expression AST

```rust
// ==================== Expression AST ====================

/// 式の AST
#[derive(Debug, Clone)]
pub enum Expr {
    // Literals
    Literal(Literal),
    
    // Identifiers
    Ident(Ident),
    CompoundIdent(Vec<Ident>),      // schema.table.column
    
    // Operators
    BinaryOp {
        left: Box<Expr>,
        op: BinaryOperator,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnaryOperator,
        expr: Box<Expr>,
    },
    
    // Comparison
    Between {
        expr: Box<Expr>,
        negated: bool,
        low: Box<Expr>,
        high: Box<Expr>,
    },
    InList {
        expr: Box<Expr>,
        list: Vec<Expr>,
        negated: bool,
    },
    InSubquery {
        expr: Box<Expr>,
        subquery: Box<SelectStatement>,
        negated: bool,
    },
    Like {
        expr: Box<Expr>,
        pattern: Box<Expr>,
        escape: Option<Box<Expr>>,
        negated: bool,
    },
    IsNull {
        expr: Box<Expr>,
        negated: bool,
    },
    
    // Functions
    Function(FunctionCall),
    Cast {
        expr: Box<Expr>,
        data_type: DataType,
    },
    Convert {
        expr: Box<Expr>,
        data_type: DataType,
        style: Option<Box<Expr>>,  // ASE specific
    },
    
    // Case
    Case {
        operand: Option<Box<Expr>>,
        conditions: Vec<(Expr, Expr)>,  // WHEN...THEN pairs
        else_result: Option<Box<Expr>>,
    },
    
    // Subquery
    Subquery(Box<SelectStatement>),
    Exists(Box<SelectStatement>),
    
    // Variables
    Variable(Variable),
    
    // Misc
    Parenthesized(Box<Expr>),
    Collate { expr: Box<Expr>, collation: String },
}

#[derive(Debug, Clone)]
pub struct Literal {
    pub kind: LiteralKind,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum LiteralKind {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    NString(String),    // Unicode string
    HexString(Vec<u8>),
}

#[derive(Debug, Clone, Copy)]
pub enum BinaryOperator {
    // Arithmetic
    Plus, Minus, Multiply, Divide, Modulo,
    // Comparison
    Eq, NotEq, Lt, LtEq, Gt, GtEq,
    // Logical
    And, Or,
    // Bitwise
    BitwiseAnd, BitwiseOr, BitwiseXor,
    // String
    Concat,
}

#[derive(Debug, Clone, Copy)]
pub enum UnaryOperator {
    Plus, Minus, Not, BitwiseNot,
}

/// 変数参照
#[derive(Debug, Clone)]
pub struct Variable {
    pub kind: VariableKind,
    pub name: String,
}

#[derive(Debug, Clone, Copy)]
pub enum VariableKind {
    Local,      // @variable
    Global,     // @@variable
    TempTable,  // #table
    GlobalTemp, // ##table
}
```

#### 3.3.4 エラーリカバリ戦略

```rust
/// パーサーエラー
#[derive(Debug, Clone)]
pub struct ParseError {
    pub kind: ParseErrorKind,
    pub span: Span,
    pub context: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ParseErrorKind {
    UnexpectedToken {
        expected: Vec<TokenKind>,
        found: TokenKind,
    },
    UnexpectedEof {
        expected: Vec<TokenKind>,
    },
    InvalidSyntax(String),
    UnsupportedFeature(String),
}

impl Parser<'_> {
    /// エラーリカバリ: 次の文の開始まで同期
    fn synchronize(&mut self) {
        while !self.is_at_end() {
            // セミコロンで同期
            if self.previous().kind == TokenKind::Semicolon {
                return;
            }
            
            // 文の開始キーワードで同期
            match self.peek().kind {
                TokenKind::Select |
                TokenKind::Insert |
                TokenKind::Update |
                TokenKind::Delete |
                TokenKind::Create |
                TokenKind::Alter |
                TokenKind::Drop |
                TokenKind::If |
                TokenKind::While |
                TokenKind::Begin |
                TokenKind::Declare |
                TokenKind::Set |
                TokenKind::Exec |
                TokenKind::Execute => return,
                _ => {}
            }
            
            self.advance();
        }
    }
}
```

#### 3.3.5 演算子優先度

```rust
/// Pratt パーサー用の演算子優先度
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    Lowest = 0,
    Or = 1,
    And = 2,
    Not = 3,
    Comparison = 4,    // =, <>, <, >, <=, >=
    Like = 5,          // LIKE, IN, BETWEEN
    BitwiseOr = 6,
    BitwiseXor = 7,
    BitwiseAnd = 8,
    Additive = 9,      // +, -
    Multiplicative = 10, // *, /, %
    Unary = 11,        // -, +, ~, NOT
    Highest = 12,
}

impl TokenKind {
    pub fn precedence(&self) -> Precedence {
        match self {
            TokenKind::Or => Precedence::Or,
            TokenKind::And => Precedence::And,
            TokenKind::Not => Precedence::Not,
            TokenKind::Eq | TokenKind::Ne | 
            TokenKind::Lt | TokenKind::Le |
            TokenKind::Gt | TokenKind::Ge => Precedence::Comparison,
            TokenKind::Like | TokenKind::In | 
            TokenKind::Between => Precedence::Like,
            TokenKind::Pipe => Precedence::BitwiseOr,
            TokenKind::Caret => Precedence::BitwiseXor,
            TokenKind::Ampersand => Precedence::BitwiseAnd,
            TokenKind::Plus | TokenKind::Minus => Precedence::Additive,
            TokenKind::Star | TokenKind::Slash | 
            TokenKind::Percent => Precedence::Multiplicative,
            TokenKind::Tilde => Precedence::Unary,
            _ => Precedence::Lowest,
        }
    }
}
```

---

### 3.4 Common SQL IR (common-sql)

#### 3.4.1 方言非依存表現

```rust
/// 方言非依存の SQL AST
/// 
/// このモジュールはソース方言（ASE）とターゲット方言（MySQL）の間の
/// 抽象的な中間表現を定義します。
pub mod ir {
    use super::*;
    
    /// 中間表現の Statement
    #[derive(Debug, Clone)]
    pub enum IrStatement {
        Select(IrSelect),
        Insert(IrInsert),
        Update(IrUpdate),
        Delete(IrDelete),
        CreateTable(IrCreateTable),
        CreateProcedure(IrCreateProcedure),
        // ... etc
    }
    
    /// 中間表現の SELECT
    #[derive(Debug, Clone)]
    pub struct IrSelect {
        pub distinct: bool,
        pub limit: Option<IrLimit>,  // TOP → LIMIT 変換済み
        pub columns: Vec<IrSelectColumn>,
        pub from: Option<IrFrom>,
        pub joins: Vec<IrJoin>,
        pub filter: Option<IrExpr>,
        pub group_by: Vec<IrExpr>,
        pub having: Option<IrExpr>,
        pub order_by: Vec<IrOrderBy>,
    }
    
    /// LIMIT/OFFSET（TOP の抽象化）
    #[derive(Debug, Clone)]
    pub struct IrLimit {
        pub count: IrExpr,
        pub offset: Option<IrExpr>,
    }
    
    /// データ型の抽象表現
    #[derive(Debug, Clone)]
    pub enum IrDataType {
        // Integers
        TinyInt,
        SmallInt,
        Int,
        BigInt,
        
        // Decimals
        Decimal { precision: Option<u8>, scale: Option<u8> },
        Float { precision: Option<u8> },
        Double,
        
        // Strings
        Char { length: Option<u32> },
        VarChar { length: Option<u32> },
        Text,
        
        // Unicode Strings
        NChar { length: Option<u32> },
        NVarChar { length: Option<u32> },
        NText,
        
        // Binary
        Binary { length: Option<u32> },
        VarBinary { length: Option<u32> },
        Blob,
        
        // Date/Time
        Date,
        Time { precision: Option<u8> },
        DateTime,
        Timestamp,
        
        // Boolean
        Boolean,
        
        // Special
        Uuid,
        Json,
        
        // Unknown/Custom
        Custom(String),
    }
    
    /// 関数の抽象表現
    #[derive(Debug, Clone)]
    pub enum IrFunction {
        // Aggregate
        Count { distinct: bool, expr: Option<Box<IrExpr>> },
        Sum(Box<IrExpr>),
        Avg(Box<IrExpr>),
        Min(Box<IrExpr>),
        Max(Box<IrExpr>),
        
        // String
        Concat(Vec<IrExpr>),
        Substring { string: Box<IrExpr>, start: Box<IrExpr>, length: Option<Box<IrExpr>> },
        Trim { expr: Box<IrExpr>, trim_type: TrimType },
        Upper(Box<IrExpr>),
        Lower(Box<IrExpr>),
        Replace { string: Box<IrExpr>, from: Box<IrExpr>, to: Box<IrExpr> },
        CharLength(Box<IrExpr>),
        
        // Date/Time
        CurrentDate,
        CurrentTime,
        CurrentTimestamp,
        DateAdd { interval: DateInterval, amount: Box<IrExpr>, date: Box<IrExpr> },
        DateDiff { interval: DateInterval, start: Box<IrExpr>, end: Box<IrExpr> },
        DatePart { part: DatePart, date: Box<IrExpr> },
        
        // Conversion
        Cast { expr: Box<IrExpr>, target_type: IrDataType },
        Coalesce(Vec<IrExpr>),
        NullIf { expr1: Box<IrExpr>, expr2: Box<IrExpr> },
        
        // Math
        Abs(Box<IrExpr>),
        Ceiling(Box<IrExpr>),
        Floor(Box<IrExpr>),
        Round { expr: Box<IrExpr>, precision: Option<Box<IrExpr>> },
        
        // Custom (dialect-specific, with warning)
        Custom { name: String, args: Vec<IrExpr> },
    }
}
```

#### 3.4.2 サポートする SQL 構文

| カテゴリ | 構文 | サポートレベル |
|----------|------|----------------|
| **DML** | SELECT, INSERT, UPDATE, DELETE | Full |
| **JOIN** | INNER, LEFT, RIGHT, FULL, CROSS | Full |
| **集約** | GROUP BY, HAVING, DISTINCT | Full |
| **サブクエリ** | スカラー、IN、EXISTS | Full |
| **UNION** | UNION, UNION ALL | Full |
| **CTE** | WITH (Common Table Expression) | Phase 3 |
| **DDL** | CREATE/ALTER/DROP TABLE | Partial |
| **ストアドプロシージャ** | CREATE PROCEDURE | Partial (変換に制限あり) |
| **関数** | CREATE FUNCTION | Partial |
| **トリガー** | CREATE TRIGGER | Warning (MySQL 構文が大きく異なる) |
| **カーソル** | CURSOR, FETCH | Warning (変換困難) |

---

### 3.5 MySQL Emitter (mysql-emitter)

#### 3.5.1 ASE → MySQL マッピングルール

```rust
/// MySQL コード生成器
pub struct MySqlEmitter {
    output: String,
    indent_level: u32,
    warnings: Vec<EmitterWarning>,
    options: EmitterOptions,
}

#[derive(Debug, Clone)]
pub struct EmitterOptions {
    pub indent_string: String,
    pub uppercase_keywords: bool,
    pub preserve_comments: bool,
    pub generate_warnings_as_comments: bool,
}

impl Default for EmitterOptions {
    fn default() -> Self {
        Self {
            indent_string: "    ".to_string(),
            uppercase_keywords: true,
            preserve_comments: true,
            generate_warnings_as_comments: true,
        }
    }
}

impl MySqlEmitter {
    pub fn emit(&mut self, stmt: &IrStatement) -> EmitResult<String> {
        match stmt {
            IrStatement::Select(select) => self.emit_select(select),
            IrStatement::Insert(insert) => self.emit_insert(insert),
            IrStatement::Update(update) => self.emit_update(update),
            IrStatement::Delete(delete) => self.emit_delete(delete),
            IrStatement::CreateTable(create) => self.emit_create_table(create),
            IrStatement::CreateProcedure(proc) => self.emit_create_procedure(proc),
            // ...
        }
    }
    
    fn emit_select(&mut self, select: &IrSelect) -> EmitResult<String> {
        self.write_keyword("SELECT");
        
        if select.distinct {
            self.write(" ");
            self.write_keyword("DISTINCT");
        }
        
        self.write(" ");
        self.emit_select_columns(&select.columns)?;
        
        if let Some(from) = &select.from {
            self.newline();
            self.write_keyword("FROM");
            self.write(" ");
            self.emit_from(from)?;
        }
        
        for join in &select.joins {
            self.newline();
            self.emit_join(join)?;
        }
        
        if let Some(filter) = &select.filter {
            self.newline();
            self.write_keyword("WHERE");
            self.write(" ");
            self.emit_expr(filter)?;
        }
        
        if !select.group_by.is_empty() {
            self.newline();
            self.write_keyword("GROUP BY");
            self.write(" ");
            self.emit_expr_list(&select.group_by)?;
        }
        
        if let Some(having) = &select.having {
            self.newline();
            self.write_keyword("HAVING");
            self.write(" ");
            self.emit_expr(having)?;
        }
        
        if !select.order_by.is_empty() {
            self.newline();
            self.write_keyword("ORDER BY");
            self.write(" ");
            self.emit_order_by(&select.order_by)?;
        }
        
        // TOP → LIMIT 変換
        if let Some(limit) = &select.limit {
            self.newline();
            self.write_keyword("LIMIT");
            self.write(" ");
            self.emit_expr(&limit.count)?;
            if let Some(offset) = &limit.offset {
                self.write(" ");
                self.write_keyword("OFFSET");
                self.write(" ");
                self.emit_expr(offset)?;
            }
        }
        
        Ok(self.output.clone())
    }
}
```

#### 3.5.2 非互換性の処理

```rust
/// エミッター警告
#[derive(Debug, Clone)]
pub struct EmitterWarning {
    pub kind: WarningKind,
    pub message: String,
    pub span: Option<Span>,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone)]
pub enum WarningKind {
    /// 機能が MySQL でサポートされていない
    UnsupportedFeature,
    /// 動作が異なる可能性がある
    BehaviorDifference,
    /// 手動レビューが必要
    ManualReviewRequired,
    /// データ精度の損失の可能性
    PrecisionLoss,
}

impl MySqlEmitter {
    /// サポートされていない機能に対する警告を追加
    fn add_warning(&mut self, kind: WarningKind, message: &str, span: Option<Span>) {
        self.warnings.push(EmitterWarning {
            kind,
            message: message.to_string(),
            span,
            suggestion: None,
        });
        
        if self.options.generate_warnings_as_comments {
            self.write(&format!("/* WARNING: {} */\n", message));
        }
    }
    
    fn emit_unsupported(&mut self, feature: &str, span: Option<Span>) -> EmitResult<()> {
        self.add_warning(
            WarningKind::UnsupportedFeature,
            &format!("'{}' is not supported in MySQL", feature),
            span,
        );
        self.write(&format!("/* UNSUPPORTED: {} */", feature));
        Ok(())
    }
}
```

---

## 4. SAP ASE TSQL 固有の処理

### 4.1 データ型マッピング

| SAP ASE 型 | MySQL 型 | 注意事項 |
|------------|----------|----------|
| `int` | `INT` | 同一 |
| `smallint` | `SMALLINT` | 同一 |
| `tinyint` | `TINYINT UNSIGNED` | ASE: 0-255, MySQL: 要 UNSIGNED |
| `bigint` | `BIGINT` | 同一 |
| `numeric(p,s)` | `DECIMAL(p,s)` | 同一 |
| `decimal(p,s)` | `DECIMAL(p,s)` | 同一 |
| `float` | `DOUBLE` | 精度確認が必要 |
| `real` | `FLOAT` | 精度確認が必要 |
| `money` | `DECIMAL(19,4)` | 固定精度に変換 |
| `smallmoney` | `DECIMAL(10,4)` | 固定精度に変換 |
| `char(n)` | `CHAR(n)` | 同一 |
| `varchar(n)` | `VARCHAR(n)` | 同一 |
| `varchar(max)` | `LONGTEXT` | MySQL は max を持たない |
| `nchar(n)` | `CHAR(n) CHARACTER SET utf8mb4` | Unicode |
| `nvarchar(n)` | `VARCHAR(n) CHARACTER SET utf8mb4` | Unicode |
| `ntext` | `LONGTEXT CHARACTER SET utf8mb4` | 非推奨 |
| `text` | `LONGTEXT` | 非推奨 |
| `image` | `LONGBLOB` | バイナリ |
| `binary(n)` | `BINARY(n)` | 同一 |
| `varbinary(n)` | `VARBINARY(n)` | 同一 |
| `datetime` | `DATETIME(3)` | ミリ秒精度 |
| `smalldatetime` | `DATETIME` | 分精度 |
| `date` | `DATE` | 同一 |
| `time` | `TIME` | 同一 |
| `timestamp` | `TIMESTAMP` | 動作が異なる（要注意） |
| `bit` | `TINYINT(1)` | MySQL は真の BIT 型あり |
| `uniqueidentifier` | `CHAR(36)` または `BINARY(16)` | UUID |

```rust
impl IrDataType {
    /// ASE データ型から IR データ型への変換
    pub fn from_ase(ase_type: &AseDataType) -> (Self, Option<String>) {
        match ase_type {
            AseDataType::Int => (IrDataType::Int, None),
            AseDataType::TinyInt => (
                IrDataType::TinyInt,
                Some("TINYINT in ASE is unsigned (0-255)".to_string())
            ),
            AseDataType::Money => (
                IrDataType::Decimal { precision: Some(19), scale: Some(4) },
                Some("MONEY converted to DECIMAL(19,4)".to_string())
            ),
            AseDataType::SmallMoney => (
                IrDataType::Decimal { precision: Some(10), scale: Some(4) },
                Some("SMALLMONEY converted to DECIMAL(10,4)".to_string())
            ),
            AseDataType::Text => (
                IrDataType::Text,
                Some("TEXT is deprecated, consider LONGTEXT".to_string())
            ),
            AseDataType::Image => (
                IrDataType::Blob,
                Some("IMAGE converted to LONGBLOB".to_string())
            ),
            AseDataType::UniqueIdentifier => (
                IrDataType::Custom("CHAR(36)".to_string()),
                Some("UNIQUEIDENTIFIER converted to CHAR(36)".to_string())
            ),
            // ... etc
        }
    }
}
```

### 4.2 関数マッピング

| SAP ASE 関数 | MySQL 関数 | 変換例 |
|--------------|------------|--------|
| `GETDATE()` | `NOW()` | 直接置換 |
| `DATEADD(unit, n, date)` | `DATE_ADD(date, INTERVAL n unit)` | 構文変換 |
| `DATEDIFF(unit, start, end)` | `TIMESTAMPDIFF(unit, start, end)` | 構文変換 |
| `DATEPART(part, date)` | `EXTRACT(part FROM date)` または関数 | 部分により異なる |
| `CONVERT(type, expr, style)` | `CAST(expr AS type)` + `DATE_FORMAT` | style による分岐 |
| `ISNULL(expr, replacement)` | `IFNULL(expr, replacement)` | 直接置換 |
| `CHARINDEX(substr, str)` | `LOCATE(substr, str)` | 引数順序同一 |
| `LEN(str)` | `CHAR_LENGTH(str)` | 直接置換 |
| `SUBSTRING(str, start, len)` | `SUBSTRING(str, start, len)` | 同一 |
| `LTRIM(str)` / `RTRIM(str)` | `LTRIM(str)` / `RTRIM(str)` | 同一 |
| `UPPER(str)` / `LOWER(str)` | `UPPER(str)` / `LOWER(str)` | 同一 |
| `REPLACE(str, from, to)` | `REPLACE(str, from, to)` | 同一 |
| `COALESCE(...)` | `COALESCE(...)` | 同一 |
| `NULLIF(a, b)` | `NULLIF(a, b)` | 同一 |
| `CAST(expr AS type)` | `CAST(expr AS type)` | 型名の変換が必要 |
| `STR(number, len, dec)` | `FORMAT(number, dec)` | 互換性注意 |
| `NEWID()` | `UUID()` | 直接置換 |
| `@@IDENTITY` | `LAST_INSERT_ID()` | 直接置換 |
| `@@ROWCOUNT` | `ROW_COUNT()` | 直接置換 |

```rust
impl MySqlEmitter {
    fn emit_function(&mut self, func: &IrFunction) -> EmitResult<()> {
        match func {
            IrFunction::DateAdd { interval, amount, date } => {
                // DATEADD(day, 5, date) → DATE_ADD(date, INTERVAL 5 DAY)
                self.write("DATE_ADD(");
                self.emit_expr(date)?;
                self.write(", INTERVAL ");
                self.emit_expr(amount)?;
                self.write(" ");
                self.emit_date_interval(interval);
                self.write(")");
            }
            
            IrFunction::DateDiff { interval, start, end } => {
                // DATEDIFF(day, start, end) → TIMESTAMPDIFF(DAY, start, end)
                self.write("TIMESTAMPDIFF(");
                self.emit_date_interval(interval);
                self.write(", ");
                self.emit_expr(start)?;
                self.write(", ");
                self.emit_expr(end)?;
                self.write(")");
            }
            
            IrFunction::Cast { expr, target_type } => {
                self.write("CAST(");
                self.emit_expr(expr)?;
                self.write(" AS ");
                self.emit_mysql_type(target_type)?;
                self.write(")");
            }
            
            // ...
        }
        Ok(())
    }
}
```

### 4.3 ストアドプロシージャ構文の差異

```sql
-- SAP ASE
CREATE PROCEDURE get_user @user_id INT, @name VARCHAR(100) OUTPUT
AS
BEGIN
    SELECT @name = name FROM users WHERE id = @user_id
    IF @@ROWCOUNT = 0
        RAISERROR 50001 'User not found'
    RETURN 0
END
GO

-- MySQL (変換後)
DELIMITER //
CREATE PROCEDURE get_user(IN p_user_id INT, OUT p_name VARCHAR(100))
BEGIN
    DECLARE v_rowcount INT DEFAULT 0;
    
    SELECT name INTO p_name FROM users WHERE id = p_user_id;
    SET v_rowcount = ROW_COUNT();
    
    IF v_rowcount = 0 THEN
        SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'User not found';
    END IF;
END //
DELIMITER ;
```

**主な変換ポイント**:

| ASE 構文 | MySQL 構文 | 変換処理 |
|----------|------------|----------|
| `@param` | `p_param` (IN/OUT/INOUT) | パラメータ宣言の変換 |
| `SELECT @var = col` | `SELECT col INTO var` | 変数代入の変換 |
| `@@ROWCOUNT` | `ROW_COUNT()` | 関数置換 |
| `RAISERROR code 'msg'` | `SIGNAL SQLSTATE '...'` | エラー処理の変換 |
| `RETURN value` | 値の返却方法が異なる | 変換困難（警告） |
| `IF cond statement` | `IF cond THEN ... END IF` | 構文変換 |
| `BEGIN...END` | `BEGIN...END` | 同一 |

### 4.4 一時テーブル構文

```sql
-- SAP ASE: ローカル一時テーブル
CREATE TABLE #temp_users (id INT, name VARCHAR(100))

-- MySQL (変換後): CREATE TEMPORARY TABLE
CREATE TEMPORARY TABLE temp_users (id INT, name VARCHAR(100))

-- SAP ASE: グローバル一時テーブル
CREATE TABLE ##global_temp (id INT)

-- MySQL: グローバル一時テーブルはサポートされない（警告）
/* WARNING: Global temporary tables (##) are not supported in MySQL */
CREATE TEMPORARY TABLE global_temp (id INT)
```

### 4.5 Identity vs AUTO_INCREMENT

```sql
-- SAP ASE
CREATE TABLE users (
    id INT IDENTITY(1,1) PRIMARY KEY,
    name VARCHAR(100)
)

-- MySQL (変換後)
CREATE TABLE users (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(100)
)
```

### 4.6 TOP N vs LIMIT

```sql
-- SAP ASE
SELECT TOP 10 * FROM users ORDER BY created_at DESC
SELECT TOP 10 PERCENT * FROM users
SELECT TOP 10 WITH TIES * FROM users ORDER BY score DESC

-- MySQL (変換後)
SELECT * FROM users ORDER BY created_at DESC LIMIT 10
/* WARNING: TOP PERCENT requires subquery workaround */
SELECT * FROM users LIMIT (SELECT CEILING(COUNT(*) * 0.1) FROM users)
/* WARNING: WITH TIES requires window function or subquery */
SELECT * FROM users WHERE score >= (SELECT MIN(score) FROM (SELECT score FROM users ORDER BY score DESC LIMIT 10) t) ORDER BY score DESC
```

### 4.7 文字列結合

```sql
-- SAP ASE (+ 演算子)
SELECT first_name + ' ' + last_name AS full_name FROM users

-- MySQL (CONCAT 関数)
SELECT CONCAT(first_name, ' ', last_name) AS full_name FROM users
```

---

## 5. エラーハンドリング戦略

### 5.1 エラータイプの階層

```rust
/// tsqlremaker のエラー型階層
pub mod error {
    use thiserror::Error;
    
    /// 最上位のエラー型
    #[derive(Error, Debug)]
    pub enum TsqlRemakerError {
        #[error("Lexer error: {0}")]
        Lexer(#[from] LexError),
        
        #[error("Parser error: {0}")]
        Parser(#[from] ParseError),
        
        #[error("Semantic error: {0}")]
        Semantic(#[from] SemanticError),
        
        #[error("Emitter error: {0}")]
        Emitter(#[from] EmitError),
        
        #[error("IO error: {0}")]
        Io(#[from] std::io::Error),
    }
    
    /// 字句解析エラー
    #[derive(Error, Debug, Clone)]
    pub enum LexError {
        #[error("Unexpected character '{ch}' at position {pos}")]
        UnexpectedChar { ch: char, pos: usize },
        
        #[error("Unterminated string literal starting at position {pos}")]
        UnterminatedString { pos: usize },
        
        #[error("Unterminated block comment starting at position {pos}")]
        UnterminatedComment { pos: usize },
        
        #[error("Invalid number format at position {pos}")]
        InvalidNumber { pos: usize },
        
        #[error("Invalid escape sequence at position {pos}")]
        InvalidEscape { pos: usize },
    }
    
    /// 構文解析エラー
    #[derive(Error, Debug, Clone)]
    pub enum ParseError {
        #[error("Unexpected token: expected {expected:?}, found {found:?}")]
        UnexpectedToken {
            expected: Vec<String>,
            found: String,
            span: Span,
        },
        
        #[error("Unexpected end of input, expected {expected:?}")]
        UnexpectedEof { expected: Vec<String> },
        
        #[error("Invalid syntax: {message}")]
        InvalidSyntax { message: String, span: Span },
        
        #[error("Unsupported feature: {feature}")]
        UnsupportedFeature { feature: String, span: Span },
    }
    
    /// 意味解析エラー
    #[derive(Error, Debug, Clone)]
    pub enum SemanticError {
        #[error("Unknown identifier: {name}")]
        UnknownIdentifier { name: String, span: Span },
        
        #[error("Type mismatch: expected {expected}, found {found}")]
        TypeMismatch {
            expected: String,
            found: String,
            span: Span,
        },
        
        #[error("Ambiguous column reference: {name}")]
        AmbiguousColumn { name: String, span: Span },
    }
    
    /// 出力生成エラー
    #[derive(Error, Debug, Clone)]
    pub enum EmitError {
        #[error("Cannot convert {feature} to MySQL")]
        UnsupportedConversion { feature: String },
        
        #[error("Internal emitter error: {message}")]
        Internal { message: String },
    }
}
```

### 5.2 エラーレポート

```rust
/// 見やすいエラーレポートの生成
pub struct ErrorReporter<'src> {
    source: &'src str,
    filename: Option<String>,
}

impl<'src> ErrorReporter<'src> {
    pub fn report(&self, error: &TsqlRemakerError) -> String {
        match error {
            TsqlRemakerError::Lexer(lex_err) => self.format_lex_error(lex_err),
            TsqlRemakerError::Parser(parse_err) => self.format_parse_error(parse_err),
            // ...
        }
    }
    
    fn format_parse_error(&self, error: &ParseError) -> String {
        match error {
            ParseError::UnexpectedToken { expected, found, span } => {
                let (line, col) = self.span_to_line_col(*span);
                let source_line = self.get_source_line(line);
                
                format!(
                    r#"
error: unexpected token
  --> {}:{}:{}
   |
{:3} | {}
   | {}{}
   |
   = expected: {}
   = found: {}
"#,
                    self.filename.as_deref().unwrap_or("<input>"),
                    line, col,
                    line, source_line,
                    " ".repeat(col),
                    "^".repeat(span.len().max(1)),
                    expected.join(", "),
                    found
                )
            }
            // ...
        }
    }
}
```

出力例：
```
error: unexpected token
  --> script.sql:15:23
   |
15 |     SELECT * FROM users WHER id = 1
   |                         ^^^^
   |
   = expected: WHERE, JOIN, ORDER, GROUP, UNION
   = found: WHER (identifier)
```

### 5.3 警告の収集

```rust
/// 変換警告
#[derive(Debug, Clone)]
pub struct ConversionWarning {
    pub level: WarningLevel,
    pub code: WarningCode,
    pub message: String,
    pub span: Option<Span>,
    pub suggestion: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningLevel {
    Info,       // 情報提供
    Warning,    // 注意が必要
    Error,      // 変換不可（エラーとして扱う）
}

#[derive(Debug, Clone, Copy)]
pub enum WarningCode {
    W001,  // 非推奨機能の使用
    W002,  // 動作が異なる可能性
    W003,  // 精度の損失
    W004,  // サポートされない機能
    W005,  // 手動レビューが必要
}

/// 警告コレクター
pub struct WarningCollector {
    warnings: Vec<ConversionWarning>,
}

impl WarningCollector {
    pub fn add(&mut self, warning: ConversionWarning) {
        self.warnings.push(warning);
    }
    
    pub fn has_errors(&self) -> bool {
        self.warnings.iter().any(|w| w.level == WarningLevel::Error)
    }
    
    pub fn report(&self) -> String {
        let mut output = String::new();
        for warning in &self.warnings {
            output.push_str(&format!(
                "[{:?}] {}: {}\n",
                warning.level, warning.code, warning.message
            ));
            if let Some(suggestion) = &warning.suggestion {
                output.push_str(&format!("  Suggestion: {}\n", suggestion));
            }
        }
        output
    }
}
```

---

## 6. テスト戦略

### 6.1 ユニットテスト

```rust
// tsql-lexer/tests/lexer_tests.rs

#[cfg(test)]
mod tests {
    use tsql_lexer::Lexer;
    use tsql_token::TokenKind;
    
    #[test]
    fn test_simple_select() {
        let input = "SELECT * FROM users";
        let lexer = Lexer::new(input);
        let tokens: Vec<_> = lexer.collect();
        
        assert_eq!(tokens[0].kind, TokenKind::Select);
        assert_eq!(tokens[1].kind, TokenKind::Star);
        assert_eq!(tokens[2].kind, TokenKind::From);
        assert_eq!(tokens[3].kind, TokenKind::Ident);
        assert_eq!(tokens[3].text, "users");
        assert_eq!(tokens[4].kind, TokenKind::Eof);
    }
    
    #[test]
    fn test_string_literal() {
        let input = "'hello world'";
        let lexer = Lexer::new(input);
        let tokens: Vec<_> = lexer.collect();
        
        assert_eq!(tokens[0].kind, TokenKind::String_);
        assert_eq!(tokens[0].text, "'hello world'");
    }
    
    #[test]
    fn test_unicode_string() {
        let input = "N'こんにちは'";
        let lexer = Lexer::new(input);
        let tokens: Vec<_> = lexer.collect();
        
        assert_eq!(tokens[0].kind, TokenKind::NString);
    }
    
    #[test]
    fn test_variables() {
        let input = "@local_var @@global_var #temp ##global_temp";
        let lexer = Lexer::new(input);
        let tokens: Vec<_> = lexer.collect();
        
        assert_eq!(tokens[0].kind, TokenKind::At);
        assert_eq!(tokens[2].kind, TokenKind::AtAt);
        assert_eq!(tokens[4].kind, TokenKind::Hash);
        assert_eq!(tokens[6].kind, TokenKind::HashHash);
    }
}
```

### 6.2 スナップショットテスト

```rust
// tests/snapshot_tests.rs
use insta::assert_snapshot;

#[test]
fn test_select_parsing() {
    let input = r#"
        SELECT 
            u.id,
            u.name,
            COUNT(*) as order_count
        FROM users u
        LEFT JOIN orders o ON u.id = o.user_id
        WHERE u.status = 'active'
        GROUP BY u.id, u.name
        HAVING COUNT(*) > 5
        ORDER BY order_count DESC
    "#;
    
    let ast = parse(input).unwrap();
    assert_snapshot!(format!("{:#?}", ast));
}

#[test]
fn test_ase_to_mysql_conversion() {
    let input = r#"
        SELECT TOP 10 
            GETDATE() as current_date,
            DATEADD(day, 7, created_at) as next_week,
            ISNULL(nickname, name) as display_name
        FROM users
        WHERE LEN(name) > 5
    "#;
    
    let mysql = convert_to_mysql(input).unwrap();
    assert_snapshot!(mysql);
}
```

### 6.3 統合テスト

```rust
// tests/integration_tests.rs

#[test]
fn test_full_pipeline() {
    let ase_sql = include_str!("fixtures/ase/complex_query.sql");
    let expected_mysql = include_str!("fixtures/mysql/complex_query.sql");
    
    let result = tsqlremaker::convert(ase_sql, Target::MySQL).unwrap();
    
    assert_eq!(normalize_sql(&result.sql), normalize_sql(expected_mysql));
    assert!(result.warnings.is_empty());
}

#[test]
fn test_conversion_with_warnings() {
    let ase_sql = "SELECT TOP 10 PERCENT * FROM users";
    
    let result = tsqlremaker::convert(ase_sql, Target::MySQL).unwrap();
    
    assert!(result.warnings.iter().any(|w| 
        w.code == WarningCode::W002 && 
        w.message.contains("PERCENT")
    ));
}
```

### 6.4 実データ SQL コーパステスト

```rust
// tests/corpus_tests.rs

use std::fs;
use walkdir::WalkDir;

#[test]
fn test_sql_corpus() {
    let corpus_dir = "tests/corpus/";
    let mut failures = Vec::new();
    
    for entry in WalkDir::new(corpus_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "sql"))
    {
        let sql = fs::read_to_string(entry.path()).unwrap();
        
        match tsqlremaker::convert(&sql, Target::MySQL) {
            Ok(result) => {
                // 変換成功、MySQL 構文として妥当かチェック
                if let Err(e) = validate_mysql_syntax(&result.sql) {
                    failures.push(format!(
                        "{}: Generated invalid MySQL: {}",
                        entry.path().display(),
                        e
                    ));
                }
            }
            Err(e) => {
                failures.push(format!(
                    "{}: Conversion failed: {}",
                    entry.path().display(),
                    e
                ));
            }
        }
    }
    
    if !failures.is_empty() {
        panic!("Corpus test failures:\n{}", failures.join("\n"));
    }
}
```

### 6.5 互換性検証

```rust
// tests/compatibility_tests.rs

/// MySQL 実環境での実行テスト（オプション）
#[test]
#[ignore] // CI では無効化、手動実行
fn test_mysql_execution() {
    let conn = mysql::Pool::new("mysql://test@localhost/testdb").unwrap();
    
    let test_cases = vec![
        ("SELECT 1 + 1", "2"),
        ("SELECT CONCAT('a', 'b')", "ab"),
        ("SELECT DATE_ADD('2024-01-01', INTERVAL 1 DAY)", "2024-01-02"),
    ];
    
    for (sql, expected) in test_cases {
        let result: String = conn.first_exec(sql, ()).unwrap().unwrap();
        assert_eq!(result, expected, "SQL: {}", sql);
    }
}
```

---

## 7. フェーズ別実装計画

### Phase 1: Lexer 完成（2-3週間）

**目標**: SAP ASE T-SQL の完全な字句解析

**タスク**:
- [ ] TokenKind enum のリファクタリング（現在の String 型から enum へ）
- [ ] 全演算子の対応（比較、算術、ビット演算）
- [ ] 文字列リテラル対応（'...'、N'...'）
- [ ] コメント対応（-- line、/* block */）
- [ ] 特殊識別子対応（@var、@@global、#temp、##global_temp、[quoted]）
- [ ] 数値リテラル完全対応（整数、小数、科学記法）
- [ ] エラーハンドリングの改善（panic! → Result）
- [ ] Span/位置情報の追加
- [ ] 包括的なテストスイート

**成果物**:
- 完全な Lexer 実装
- Token Stream Iterator
- 80%+ テストカバレッジ

### Phase 2: Parser 構築（4-6週間）

**目標**: AST の構築

**タスク**:
- [ ] common-sql crate の AST 定義
- [ ] 再帰下降パーサーの基盤
- [ ] Pratt パーサー（式解析用）
- [ ] SELECT 文の完全パース
- [ ] INSERT/UPDATE/DELETE のパース
- [ ] JOIN 句のパース
- [ ] WHERE 句と式のパース
- [ ] GROUP BY/HAVING/ORDER BY のパース
- [ ] サブクエリのパース
- [ ] エラーリカバリ機構

**成果物**:
- DML 文の完全なパーサー
- AST 型定義
- パーサーテストスイート

### Phase 3: DDL & 制御フロー（3-4週間）

**目標**: DDL 文と制御フロー構文のサポート

**タスク**:
- [ ] CREATE TABLE パース
- [ ] CREATE PROCEDURE パース
- [ ] CREATE VIEW パース
- [ ] ALTER/DROP 文
- [ ] IF/ELSE 文
- [ ] WHILE ループ
- [ ] BEGIN/END ブロック
- [ ] DECLARE/SET 文
- [ ] RETURN 文

**成果物**:
- DDL パーサー
- 制御フロー AST
- プロシージャ変換の基盤

### Phase 4: MySQL Emitter（3-4週間）

**目標**: MySQL SQL の生成

**タスク**:
- [ ] mysql-emitter crate の作成
- [ ] 基本的な SELECT 変換
- [ ] TOP → LIMIT 変換
- [ ] データ型マッピング
- [ ] 関数マッピング（DATEADD、CONVERT 等）
- [ ] 文字列結合の変換
- [ ] プロシージャ構文変換
- [ ] 警告システム

**成果物**:
- MySQL コード生成器
- マッピングルール
- 変換テストスイート

### Phase 5: 統合 & CLI（2-3週間）

**目標**: 完全なパイプラインと CLI ツール

**タスク**:
- [ ] 全コンポーネントの統合
- [ ] CLI インターフェース（clap 使用）
- [ ] ファイル入出力
- [ ] バッチ処理
- [ ] エラーレポート改善
- [ ] ドキュメント作成

**成果物**:
- tsqlremaker CLI ツール
- 使用ドキュメント
- サンプル変換

### MVP 定義

**Minimum Viable Product** として以下を含む:

1. **入力**: SAP ASE T-SQL ファイル
2. **出力**: MySQL SQL ファイル + 警告レポート
3. **対応構文**:
   - SELECT（TOP、JOIN、WHERE、GROUP BY、ORDER BY）
   - INSERT/UPDATE/DELETE
   - 基本的なデータ型変換
   - 主要関数変換（GETDATE、DATEADD、ISNULL 等）
4. **CLI コマンド**:
   ```bash
   tsqlremaker convert input.sql -o output.sql --target mysql
   tsqlremaker check input.sql  # 構文チェックのみ
   ```

---

## 8. 将来の検討事項

### 8.1 WASM コンパイル

**目的**: ブラウザでの SQL 変換ツール

**実装方針**:
```toml
# Cargo.toml
[lib]
crate-type = ["cdylib", "rlib"]

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
```

```rust
// src/wasm.rs
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn convert_sql(input: &str) -> Result<String, JsValue> {
    tsqlremaker::convert(input, Target::MySQL)
        .map(|r| r.sql)
        .map_err(|e| JsValue::from_str(&e.to_string()))
}
```

### 8.2 LSP 統合

**目的**: IDE でのリアルタイム変換支援

**機能**:
- 構文エラーのハイライト
- 変換警告の表示
- ホバーでの MySQL プレビュー
- 変換アクションの提供

```rust
// 将来の LSP サーバー設計
pub struct TsqlRemakerLsp {
    documents: HashMap<Url, Document>,
    diagnostics: DiagnosticPublisher,
}

impl TsqlRemakerLsp {
    pub fn handle_did_change(&mut self, params: DidChangeTextDocumentParams) {
        let uri = params.text_document.uri;
        let content = &params.content_changes[0].text;
        
        // 増分解析
        let diagnostics = self.analyze(content);
        self.diagnostics.publish(uri, diagnostics);
    }
}
```

### 8.3 追加ターゲット方言

**優先順位**:
1. **PostgreSQL** - 人気のオープンソース DB
2. **SQLite** - 組み込み用途
3. **Oracle** - エンタープライズ需要

**実装方針**:
```rust
pub trait SqlEmitter {
    fn emit_select(&mut self, stmt: &IrSelect) -> EmitResult<String>;
    fn emit_insert(&mut self, stmt: &IrInsert) -> EmitResult<String>;
    // ...
}

pub struct PostgresEmitter { /* ... */ }
pub struct OracleEmitter { /* ... */ }

impl SqlEmitter for PostgresEmitter { /* ... */ }
impl SqlEmitter for OracleEmitter { /* ... */ }
```

### 8.4 IDE プラグイン

**対象**:
- VS Code Extension
- JetBrains Plugin (IntelliJ, DataGrip)

**機能**:
- SQL ファイルの自動検出
- 右クリックで変換
- サイドバイサイドプレビュー
- バッチ変換

---

## 付録

### A. 参考リソース

- [SAP ASE Documentation](https://help.sap.com/docs/SAP_ASE)
- [MySQL Reference Manual](https://dev.mysql.com/doc/refman/8.0/en/)
- [sqlparser-rs](https://github.com/apache/datafusion-sqlparser-rs) - Rust SQL パーサー参考実装
- [Crafting Interpreters](https://craftinginterpreters.com/) - パーサー設計の参考書

### B. 用語集

| 用語 | 説明 |
|------|------|
| **ASE** | SAP Adaptive Server Enterprise（旧 Sybase） |
| **T-SQL** | Transact-SQL、Microsoft/Sybase の SQL 方言 |
| **AST** | Abstract Syntax Tree、抽象構文木 |
| **IR** | Intermediate Representation、中間表現 |
| **Lexer** | 字句解析器、ソースコードをトークンに分解 |
| **Parser** | 構文解析器、トークンから AST を構築 |
| **Emitter** | コード生成器、AST から出力コードを生成 |
| **Span** | ソースコード上の位置範囲 |

### C. 変更履歴

| 日付 | バージョン | 変更内容 |
|------|------------|----------|
| 2026-01-19 | 0.1.0 | 初版作成 |
| 2026-01-19 | 0.2.0 | 追加要件（セクション9）を追加、Rust言語特性最適化 |

---

## 9. 追加要件（v0.2.0）

本セクションは、初期SDDに対する追加要件を定義する。
MVP（Must）とPost-MVP（Should/Could）に分類し、優先度を明確化する。

### 9.1 Rust言語特性に関する制約（MANDATORY）

#### 9.1.1 パニック禁止ポリシー

**ライブラリcrateにおいて `panic!`, `unwrap()`, `expect()` は禁止する。**

| 禁止事項 | 代替手段 |
|----------|----------|
| `panic!("message")` | `return Err(Error::...)` |
| `option.unwrap()` | `option.ok_or(Error::...)?` または `option.ok_or_else(\|\| Error::...)?` |
| `result.unwrap()` | `result?` または `result.map_err(\|e\| Error::...)?` |
| `option.expect("msg")` | `option.ok_or(Error::WithContext("msg"))?` |
| `slice[index]` (境界チェックなし) | `slice.get(index).ok_or(Error::OutOfBounds)?` |
| `assert!` (リリースビルド) | `if !cond { return Err(...) }` |

**例外**: `#[cfg(test)]` モジュール内のテストコードのみ許可。

```rust
// ❌ 禁止
pub fn next_token(&mut self) -> Token {
    let ch = self.input.chars().nth(0).unwrap(); // PANIC!
    // ...
}

// ✅ 許可
pub fn next_token(&mut self) -> Result<Token, LexError> {
    let ch = self.input.chars().next()
        .ok_or(LexError::UnexpectedEof { pos: self.position })?;
    // ...
}
```

#### 9.1.2 Result型の統一

```rust
/// 全crateで共通のResult型エイリアス
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// エラー型は thiserror を使用
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Lexer error: {0}")]
    Lex(#[from] LexError),
    
    #[error("Parser error: {0}")]
    Parse(#[from] ParseError),
    
    #[error("Emit error: {0}")]
    Emit(#[from] EmitError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
```

#### 9.1.3 Zero-cost abstractions の活用

```rust
// ✅ Zero-copy Token（メモリ割り当てなし）
pub struct Token<'src> {
    pub kind: TokenKind,
    pub text: &'src str,    // ソースへの借用
    pub span: Span,
}

// ✅ 静的キーワードマップ（once_cell / LazyLock）
static KEYWORDS: LazyLock<HashMap<&'static str, TokenKind>> = LazyLock::new(|| {
    // 一度だけ初期化
});

// ❌ 毎回HashMapを作成（現在の実装）
pub fn lookup_ident(ident: &str) -> token_type {
    let mut map = HashMap::new(); // 毎回allocate!
    // ...
}
```

#### 9.1.4 所有権とライフタイムの設計指針

| パターン | 使用場面 | 例 |
|----------|----------|-----|
| `&'src str` | Token, Span, 一時参照 | `Token<'src>` |
| `String` | エラーメッセージ, 出力SQL | `EmitError::message` |
| `Cow<'a, str>` | 入力そのまま or 変換が必要 | 文字列リテラルのエスケープ |
| `Box<T>` | 再帰的構造（AST） | `Box<Expr>` |
| `Arc<T>` | 複数所有者（並列処理） | 将来のLSP用 |

---

### 9.2 中間表現（IR）とシリアライズ形式【MVP - Must】

#### 9.2.1 内部表現と外部形式の分離

```
┌─────────────────────────────────────────────────────────────────┐
│                        内部表現層                                │
│  common-sql crate: Rust型（AST/IR）                             │
│  - Statement, Expr, DataType, Span                              │
│  - 全ての処理はRust型を正とする                                   │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        変換層                                    │
│  proto-mapper crate: Rust ⇄ Protobuf 双方向変換                  │
│  - From/Into trait実装                                          │
│  - 変換時の検証ロジック                                          │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                        外部形式層                                │
│  /proto/*.proto: 外部ツール連携用                                │
│  - tsqlremaker_ir.proto: 変換パイプライン用                      │
│  - tsqlremaker_index.proto: コードインテリジェンス用             │
│  - JSON: デバッグ用補助出力                                      │
└─────────────────────────────────────────────────────────────────┘
```

#### 9.2.2 Protobufスキーマ分割

**ファイル構成**:
```
proto/
├── common.proto           # 共通定義（Span, Location）
├── tsqlremaker_ir.proto   # 変換パイプライン用
└── tsqlremaker_index.proto # コードインテリジェンス用
```

**common.proto**:
```protobuf
syntax = "proto3";
package tsqlremaker.common;

// バイトオフセットベースの位置情報
message Span {
  uint32 start = 1;  // 開始バイトオフセット（UTF-8）
  uint32 end = 2;    // 終了バイトオフセット
}

// 行・列情報（LSP向け）
message Location {
  uint32 line = 1;    // 1-indexed
  uint32 column = 2;  // 1-indexed (UTF-16 code units for LSP)
  uint32 offset = 3;  // 0-indexed byte offset
}

// ソース位置の完全情報
message SourceSpan {
  Span span = 1;
  Location start_loc = 2;
  Location end_loc = 3;
}
```

**tsqlremaker_ir.proto**:
```protobuf
syntax = "proto3";
package tsqlremaker.ir;

import "common.proto";

// フィールド番号ルール:
// - 1-15: 頻出フィールド（1バイトエンコード）
// - 16-2047: 通常フィールド
// - reserved で削除フィールドを管理
// - 破壊的変更禁止（後方互換維持）

message Statement {
  oneof stmt {
    SelectStatement select = 1;
    InsertStatement insert = 2;
    UpdateStatement update = 3;
    DeleteStatement delete = 4;
    CreateTableStatement create_table = 5;
    CreateProcedureStatement create_procedure = 6;
    // ...
  }
  common.SourceSpan span = 15;
}

message SelectStatement {
  bool distinct = 1;
  optional LimitClause limit = 2;
  repeated SelectColumn columns = 3;
  optional FromClause from = 4;
  repeated JoinClause joins = 5;
  optional Expr where_clause = 6;
  repeated Expr group_by = 7;
  optional Expr having = 8;
  repeated OrderByItem order_by = 9;
  common.SourceSpan span = 15;
}

message Expr {
  oneof expr {
    Literal literal = 1;
    Identifier ident = 2;
    BinaryExpr binary = 3;
    UnaryExpr unary = 4;
    FunctionCall function = 5;
    CaseExpr case = 6;
    SubqueryExpr subquery = 7;
    ErrorExpr error = 14;  // 不完全ノード
  }
  common.SourceSpan span = 15;
}

// 不完全ノード（エラー回復用）
message ErrorExpr {
  string message = 1;
  repeated string partial_tokens = 2;  // 回収できたトークン
}
```

**tsqlremaker_index.proto**:
```protobuf
syntax = "proto3";
package tsqlremaker.index;

import "common.proto";

message Index {
  repeated Symbol symbols = 1;
  repeated Reference references = 2;
  repeated Scope scopes = 3;
  repeated Dependency dependencies = 4;
}

message Symbol {
  string id = 1;           // ユニークID
  string name = 2;         // 識別子名
  SymbolKind kind = 3;
  common.SourceSpan span = 4;
  string scope_id = 5;     // 所属スコープ
  optional string type_info = 6;  // 型情報（解決済みの場合）
}

enum SymbolKind {
  SYMBOL_KIND_UNSPECIFIED = 0;
  SYMBOL_KIND_TABLE = 1;
  SYMBOL_KIND_COLUMN = 2;
  SYMBOL_KIND_VARIABLE = 3;
  SYMBOL_KIND_PARAMETER = 4;
  SYMBOL_KIND_PROCEDURE = 5;
  SYMBOL_KIND_FUNCTION = 6;
  SYMBOL_KIND_ALIAS = 7;
  SYMBOL_KIND_TEMP_TABLE = 8;
}

message Reference {
  string id = 1;
  string name = 2;
  common.SourceSpan span = 3;
  ReferenceKind kind = 4;
  ResolutionStatus resolution = 5;
  optional string resolved_symbol_id = 6;  // 解決済みの場合
  optional string unresolved_reason = 7;   // 未解決の場合の理由
}

enum ReferenceKind {
  REFERENCE_KIND_UNSPECIFIED = 0;
  REFERENCE_KIND_READ = 1;
  REFERENCE_KIND_WRITE = 2;
  REFERENCE_KIND_CALL = 3;
}

enum ResolutionStatus {
  RESOLUTION_STATUS_UNSPECIFIED = 0;
  RESOLUTION_STATUS_RESOLVED = 1;
  RESOLUTION_STATUS_UNRESOLVED = 2;
  RESOLUTION_STATUS_AMBIGUOUS = 3;
}

message Scope {
  string id = 1;
  ScopeKind kind = 2;
  common.SourceSpan span = 3;
  optional string parent_id = 4;
  repeated string symbol_ids = 5;
}

enum ScopeKind {
  SCOPE_KIND_UNSPECIFIED = 0;
  SCOPE_KIND_GLOBAL = 1;
  SCOPE_KIND_PROCEDURE = 2;
  SCOPE_KIND_BLOCK = 3;
  SCOPE_KIND_QUERY = 4;
}

message Dependency {
  string source_object = 1;  // 依存元
  string target_object = 2;  // 依存先
  DependencyKind kind = 3;
  bool is_dynamic = 4;       // 動的SQL由来か
}

enum DependencyKind {
  DEPENDENCY_KIND_UNSPECIFIED = 0;
  DEPENDENCY_KIND_TABLE_READ = 1;
  DEPENDENCY_KIND_TABLE_WRITE = 2;
  DEPENDENCY_KIND_PROCEDURE_CALL = 3;
  DEPENDENCY_KIND_FUNCTION_CALL = 4;
}
```

#### 9.2.3 後方互換性ルール

| ルール | 説明 |
|--------|------|
| フィールド番号固定 | 一度割り当てた番号は変更不可 |
| reserved運用 | 削除フィールドは `reserved` に追加 |
| 新規フィールド | `optional` または `repeated` で追加 |
| 列挙値 | `_UNSPECIFIED = 0` を必須、新規値は末尾追加 |
| 破壊的変更禁止 | メジャーバージョンアップ時のみ許可 |

---

### 9.3 Span/位置情報の全工程適用【MVP - Must】

#### 9.3.1 位置情報の要件

**全ノードがSpanを保持する**:
- Lexer Token: ✅
- Parser AST: ✅
- Common IR: ✅
- Emitter Warning: ✅

```rust
/// バイトオフセットベースのSpan
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: u32,  // UTF-8 byte offset
    pub end: u32,
}

impl Span {
    pub const DUMMY: Span = Span { start: 0, end: 0 };
    
    pub fn new(start: usize, end: usize) -> Self {
        Self {
            start: start as u32,
            end: end as u32,
        }
    }
    
    pub fn len(&self) -> usize {
        (self.end - self.start) as usize
    }
    
    pub fn merge(self, other: Span) -> Span {
        Span {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }
}

/// LSP向けの行・列変換
pub struct LineIndex {
    line_starts: Vec<u32>,  // 各行の開始オフセット
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (i, c) in text.char_indices() {
            if c == '\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self { line_starts }
    }
    
    /// byte offset → (line, column) (both 0-indexed)
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        let line = self.line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1);
        let line_start = self.line_starts[line];
        let col = offset - line_start;
        (line as u32, col)
    }
    
    /// LSP用: UTF-16 code units への変換
    pub fn to_lsp_position(&self, text: &str, offset: u32) -> lsp_types::Position {
        let (line, col_bytes) = self.line_col(offset);
        let line_start = self.line_starts[line as usize] as usize;
        let col_utf16 = text[line_start..offset as usize]
            .encode_utf16()
            .count();
        lsp_types::Position {
            line,
            character: col_utf16 as u32,
        }
    }
}
```

#### 9.3.2 エラー/警告の統一構造

```rust
/// 診断情報（エラー・警告共通）
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: DiagnosticCode,
    pub message: String,
    pub span: Span,
    pub suggestion: Option<Suggestion>,
    pub related: Vec<RelatedInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

/// 診断コード（カテゴリ + 番号）
#[derive(Debug, Clone, Copy)]
pub struct DiagnosticCode {
    pub category: DiagnosticCategory,
    pub number: u16,
}

#[derive(Debug, Clone, Copy)]
pub enum DiagnosticCategory {
    Lex,    // L001, L002, ...
    Parse,  // P001, P002, ...
    Sem,    // S001, S002, ... (Semantic)
    Emit,   // E001, E002, ...
}

impl std::fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let prefix = match self.category {
            DiagnosticCategory::Lex => 'L',
            DiagnosticCategory::Parse => 'P',
            DiagnosticCategory::Sem => 'S',
            DiagnosticCategory::Emit => 'E',
        };
        write!(f, "{}{:03}", prefix, self.number)
    }
}

/// 修正提案
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub message: String,
    pub replacement: Option<String>,
    pub span: Span,
}

/// 関連情報（複数箇所に跨るエラー用）
#[derive(Debug, Clone)]
pub struct RelatedInfo {
    pub message: String,
    pub span: Span,
}
```

---

### 9.4 エラー回復（壊れたSQLでも解析継続）【MVP - Must】

#### 9.4.1 エラー回復戦略

```rust
/// パーサーのエラー回復モード
pub struct Parser<'src> {
    tokens: Vec<Token<'src>>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
    panic_mode: bool,  // エラー回復中フラグ
}

impl<'src> Parser<'src> {
    /// エラー発生時の同期点まで回復
    fn synchronize(&mut self) {
        self.panic_mode = true;
        
        while !self.is_at_end() {
            // セミコロンで同期
            if self.previous().kind == TokenKind::Semicolon {
                self.panic_mode = false;
                return;
            }
            
            // 文の開始キーワードで同期
            match self.peek().kind {
                TokenKind::Select |
                TokenKind::Insert |
                TokenKind::Update |
                TokenKind::Delete |
                TokenKind::Create |
                TokenKind::Alter |
                TokenKind::Drop |
                TokenKind::If |
                TokenKind::While |
                TokenKind::Begin |
                TokenKind::Declare |
                TokenKind::Set |
                TokenKind::Exec |
                TokenKind::Execute |
                TokenKind::Go => {
                    self.panic_mode = false;
                    return;
                }
                _ => {}
            }
            
            self.advance();
        }
        
        self.panic_mode = false;
    }
    
    /// エラーを記録してErrorノードを返す
    fn error_expr(&mut self, message: &str) -> Expr {
        let span = self.current_span();
        self.diagnostics.push(Diagnostic {
            severity: Severity::Error,
            code: DiagnosticCode { 
                category: DiagnosticCategory::Parse, 
                number: 1 
            },
            message: message.to_string(),
            span,
            suggestion: None,
            related: vec![],
        });
        
        Expr::Error(ErrorExpr {
            message: message.to_string(),
            span,
        })
    }
    
    /// 複数エラー収集（fail-fast禁止）
    pub fn parse(&mut self) -> ParseResult {
        let mut statements = Vec::new();
        
        while !self.is_at_end() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(_) => {
                    // エラー発生時も継続
                    self.synchronize();
                }
            }
        }
        
        ParseResult {
            statements,
            diagnostics: std::mem::take(&mut self.diagnostics),
        }
    }
}

/// 不完全ノードを許容するAST
#[derive(Debug, Clone)]
pub enum Expr {
    Literal(Literal),
    Ident(Ident),
    Binary(BinaryExpr),
    // ... 他のノード
    
    /// エラー回復用の不完全ノード
    Error(ErrorExpr),
}

#[derive(Debug, Clone)]
pub struct ErrorExpr {
    pub message: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Select(SelectStatement),
    Insert(InsertStatement),
    // ... 他のノード
    
    /// エラー回復用の不完全ノード
    Error(ErrorStatement),
}

#[derive(Debug, Clone)]
pub struct ErrorStatement {
    pub message: String,
    pub partial_tokens: Vec<TokenKind>,  // 回収できたトークン種別
    pub span: Span,
}
```

#### 9.4.2 エラー回復の同期点

| 同期点 | トークン/パターン |
|--------|------------------|
| 文の終端 | `;` |
| 文の開始 | `SELECT`, `INSERT`, `UPDATE`, `DELETE`, `CREATE`, `ALTER`, `DROP` |
| ブロック開始 | `BEGIN`, `IF`, `WHILE` |
| バッチ区切り | `GO` |
| 宣言 | `DECLARE`, `SET` |

---

### 9.5 共通API定義【MVP - Must】

#### 9.5.1 統一APIインターフェース

```rust
/// tsqlremaker の公開API
pub mod api {
    use crate::*;

    /// 解析結果
    #[derive(Debug)]
    pub struct AnalysisResult<'src> {
        pub ast: Vec<Statement>,
        pub diagnostics: Vec<Diagnostic>,
        pub source: &'src str,
        pub line_index: LineIndex,
    }

    /// 変換結果
    #[derive(Debug)]
    pub struct ConversionResult {
        pub sql: String,
        pub warnings: Vec<Diagnostic>,
        pub source_map: Option<SourceMap>,  // 将来用
    }

    /// インデックス結果
    #[derive(Debug)]
    pub struct IndexResult {
        pub symbols: Vec<Symbol>,
        pub references: Vec<Reference>,
        pub scopes: Vec<Scope>,
        pub dependencies: Vec<Dependency>,
        pub diagnostics: Vec<Diagnostic>,
    }

    // ==================== Core API ====================

    /// ソースコードをパース
    /// 
    /// # Example
    /// ```
    /// let result = tsqlremaker::parse("SELECT * FROM users")?;
    /// assert!(result.diagnostics.is_empty());
    /// ```
    pub fn parse(source: &str) -> Result<AnalysisResult<'_>> {
        let lexer = Lexer::new(source);
        let tokens: Vec<_> = lexer.collect::<Result<_, _>>()?;
        let mut parser = Parser::new(&tokens);
        let parse_result = parser.parse();
        
        Ok(AnalysisResult {
            ast: parse_result.statements,
            diagnostics: parse_result.diagnostics,
            source,
            line_index: LineIndex::new(source),
        })
    }

    /// AST を Common IR に変換（lower）
    pub fn lower(ast: &[Statement]) -> Result<Vec<IrStatement>> {
        let mut lowerer = Lowerer::new();
        ast.iter()
            .map(|stmt| lowerer.lower_statement(stmt))
            .collect()
    }

    /// AST/IR からインデックスを構築
    pub fn index(ast: &[Statement]) -> Result<IndexResult> {
        let mut indexer = Indexer::new();
        indexer.index_statements(ast)?;
        Ok(indexer.into_result())
    }

    /// IR をターゲットSQLに出力
    pub fn emit(ir: &[IrStatement], target: Target) -> Result<ConversionResult> {
        match target {
            Target::MySQL => {
                let mut emitter = MySqlEmitter::new();
                let sql = emitter.emit_statements(ir)?;
                Ok(ConversionResult {
                    sql,
                    warnings: emitter.warnings,
                    source_map: None,
                })
            }
            Target::PostgreSQL => {
                // PostgreSQL エミッターで変換
                use postgresql_emitter::PostgreSqlEmitter;
                let emitter = PostgreSqlEmitter::new();
                let sql = emitter.emit(&ast)?;
                Ok(ConversionResult {
                    sql,
                    warnings: emitter.warnings,
                    source_map: None,
                })
            }
        }
    }

    /// 一括変換（parse → lower → emit）
    pub fn convert(source: &str, target: Target) -> Result<FullConversionResult> {
        let analysis = parse(source)?;
        
        // エラーがあっても継続（警告として扱う）
        let has_errors = analysis.diagnostics.iter()
            .any(|d| d.severity == Severity::Error);
        
        let ir = lower(&analysis.ast)?;
        let conversion = emit(&ir, target)?;
        
        Ok(FullConversionResult {
            sql: conversion.sql,
            diagnostics: analysis.diagnostics,
            warnings: conversion.warnings,
            has_errors,
        })
    }

    /// 完全な変換結果
    #[derive(Debug)]
    pub struct FullConversionResult {
        pub sql: String,
        pub diagnostics: Vec<Diagnostic>,  // パース時のエラー/警告
        pub warnings: Vec<Diagnostic>,     // 変換時の警告
        pub has_errors: bool,
    }

    /// ターゲット方言
    #[derive(Debug, Clone, Copy)]
    pub enum Target {
        MySQL,
        PostgreSQL,
    }
}
```

#### 9.5.2 CLI インターフェース

```rust
// tsql-cli/src/main.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "tsqlremaker")]
#[command(about = "SAP ASE T-SQL to MySQL/PostgreSQL converter")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Parse and check syntax
    Check {
        /// Input SQL file
        #[arg(short, long)]
        input: PathBuf,
        
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: OutputFormat,
    },
    
    /// Convert SQL to target dialect
    Convert {
        /// Input SQL file
        #[arg(short, long)]
        input: PathBuf,
        
        /// Output SQL file
        #[arg(short, long)]
        output: Option<PathBuf>,
        
        /// Target dialect
        #[arg(short, long, default_value = "mysql")]
        target: Target,
        
        /// Continue on errors (emit warnings as comments)
        #[arg(long)]
        continue_on_error: bool,
    },
    
    /// Build symbol index
    Index {
        /// Input SQL file or directory
        #[arg(short, long)]
        input: PathBuf,
        
        /// Output format (json, protobuf)
        #[arg(short, long, default_value = "json")]
        format: IndexFormat,
        
        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    
    /// Dump AST/IR for debugging
    Dump {
        /// Input SQL file
        #[arg(short, long)]
        input: PathBuf,
        
        /// Output format (ast, ir, tokens)
        #[arg(short, long, default_value = "ast")]
        format: DumpFormat,
    },
}
```

---

### 9.6 変換器の要件（変換不能時の挙動）【MVP - Must】

#### 9.6.1 変換モード

```rust
/// 変換オプション
#[derive(Debug, Clone)]
pub struct EmitOptions {
    /// エラー時の動作
    pub on_error: ErrorBehavior,
    
    /// キーワードの大文字/小文字
    pub keyword_case: KeywordCase,
    
    /// インデント文字列
    pub indent: String,
    
    /// コメント保持
    pub preserve_comments: bool,
    
    /// 警告をSQLコメントとして出力
    pub embed_warnings: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorBehavior {
    /// エラーで即座に停止
    Strict,
    /// 警告コメントを埋めて継続
    Lenient,
}

impl Default for EmitOptions {
    fn default() -> Self {
        Self {
            on_error: ErrorBehavior::Lenient,
            keyword_case: KeywordCase::Upper,
            indent: "    ".to_string(),
            preserve_comments: true,
            embed_warnings: true,
        }
    }
}
```

#### 9.6.2 変換不能時の出力例

```sql
-- 入力 (ASE)
SELECT TOP 10 PERCENT WITH TIES *
FROM users
WHERE PATINDEX('%test%', name) > 0

-- 出力 (MySQL, Lenient mode)
/* [E001] TOP PERCENT WITH TIES cannot be directly converted to MySQL.
   Suggestion: Use subquery with window function for equivalent behavior.
   Manual review required. */
SELECT *
FROM users
WHERE
/* [E002] PATINDEX is not available in MySQL.
   Suggestion: Use LOCATE() or REGEXP for pattern matching. */
LOCATE('test', name) > 0
LIMIT 10
```

---

### 9.7 動的SQLの扱い【Post-MVP - Should】

#### 9.7.1 動的SQL検出

```rust
/// 動的SQLの種類
#[derive(Debug, Clone)]
pub enum DynamicSqlKind {
    /// EXEC(@sql)
    ExecVariable { variable: String },
    
    /// EXEC('SELECT ...')
    ExecLiteral { sql: String },
    
    /// 文字列連結による構築
    StringConcatenation { 
        template: Option<String>,  // 推定されたテンプレート
        variables: Vec<String>,    // 使用変数
    },
}

/// 動的SQL情報
#[derive(Debug, Clone)]
pub struct DynamicSqlInfo {
    pub kind: DynamicSqlKind,
    pub span: Span,
    pub estimated_dependencies: Vec<String>,  // 推定される依存
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy)]
pub enum Confidence {
    High,    // 定数部分から確実に推定
    Medium,  // パターンから推定
    Low,     // ほぼ推定不可
}
```

#### 9.7.2 動的SQLの警告

```rust
impl Indexer {
    fn handle_dynamic_sql(&mut self, info: &DynamicSqlInfo) {
        self.diagnostics.push(Diagnostic {
            severity: Severity::Warning,
            code: DiagnosticCode { 
                category: DiagnosticCategory::Sem, 
                number: 100 
            },
            message: format!(
                "Dynamic SQL detected. Dependencies may be incomplete. \
                 Confidence: {:?}",
                info.confidence
            ),
            span: info.span,
            suggestion: Some(Suggestion {
                message: "Consider adding explicit dependency comments \
                          or using static SQL where possible.".to_string(),
                replacement: None,
                span: info.span,
            }),
            related: vec![],
        });
        
        // 推定依存をマーク
        for dep in &info.estimated_dependencies {
            self.add_dependency(Dependency {
                source_object: self.current_object.clone(),
                target_object: dep.clone(),
                kind: DependencyKind::Unknown,
                is_dynamic: true,
            });
        }
    }
}
```

---

### 9.8 意味解析（名前解決）【Post-MVP - Should】

#### 9.8.1 MVP範囲の名前解決

| 対象 | 解決レベル | 例 |
|------|-----------|-----|
| ローカル変数 `@x` | 完全解決 | `DECLARE @x INT; SELECT @x` |
| テーブル別名 | 完全解決 | `SELECT t.col FROM tbl t` |
| 一時テーブル `#t` | 完全解決 | `CREATE TABLE #t ...; SELECT * FROM #t` |
| 列参照 | 別名スコープ内 | `SELECT t.col` → テーブル別名から解決 |
| DBオブジェクト | 未解決許容 | `dbo.users` → カタログ連携時に解決 |

#### 9.8.2 スコープ管理

```rust
pub struct ScopeManager {
    scopes: Vec<Scope>,
    current: ScopeId,
}

impl ScopeManager {
    /// スコープに入る
    pub fn enter_scope(&mut self, kind: ScopeKind, span: Span) -> ScopeId {
        let id = ScopeId::new();
        self.scopes.push(Scope {
            id,
            kind,
            span,
            parent: Some(self.current),
            symbols: HashMap::new(),
        });
        self.current = id;
        id
    }
    
    /// スコープから出る
    pub fn leave_scope(&mut self) {
        if let Some(scope) = self.scopes.iter().find(|s| s.id == self.current) {
            if let Some(parent) = scope.parent {
                self.current = parent;
            }
        }
    }
    
    /// シンボル定義
    pub fn define(&mut self, name: &str, symbol: Symbol) -> Result<(), SemanticError> {
        let scope = self.current_scope_mut();
        if scope.symbols.contains_key(name) {
            return Err(SemanticError::DuplicateDefinition {
                name: name.to_string(),
                span: symbol.span,
            });
        }
        scope.symbols.insert(name.to_string(), symbol);
        Ok(())
    }
    
    /// シンボル解決（現在スコープから親へ遡る）
    pub fn resolve(&self, name: &str) -> Option<&Symbol> {
        let mut scope_id = Some(self.current);
        while let Some(id) = scope_id {
            if let Some(scope) = self.scopes.iter().find(|s| s.id == id) {
                if let Some(symbol) = scope.symbols.get(name) {
                    return Some(symbol);
                }
                scope_id = scope.parent;
            } else {
                break;
            }
        }
        None
    }
}
```

---

### 9.9 性能・テスト要件【MVP - Must】

#### 9.9.1 コーパステスト

```rust
// tests/corpus_test.rs

use std::fs;
use walkdir::WalkDir;

/// コーパステスト：実SQLファイルの解析成功率を計測
#[test]
fn test_sql_corpus() {
    let corpus_dir = "tests/corpus/";
    let mut stats = CorpusStats::default();
    
    for entry in WalkDir::new(corpus_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "sql"))
    {
        let sql = fs::read_to_string(entry.path()).unwrap();
        stats.total += 1;
        
        match tsqlremaker::parse(&sql) {
            Ok(result) => {
                let has_errors = result.diagnostics.iter()
                    .any(|d| d.severity == Severity::Error);
                
                if has_errors {
                    stats.parse_with_errors += 1;
                } else {
                    stats.parse_success += 1;
                }
                
                // 変換テスト
                match tsqlremaker::convert(&sql, Target::MySQL) {
                    Ok(conv) => {
                        if conv.has_errors {
                            stats.convert_with_warnings += 1;
                        } else {
                            stats.convert_success += 1;
                        }
                    }
                    Err(_) => stats.convert_failure += 1,
                }
            }
            Err(_) => stats.parse_failure += 1,
        }
    }
    
    // 成功率の検証
    let parse_rate = stats.parse_success as f64 / stats.total as f64 * 100.0;
    let convert_rate = stats.convert_success as f64 / stats.total as f64 * 100.0;
    
    println!("Corpus Stats:");
    println!("  Total files: {}", stats.total);
    println!("  Parse success: {} ({:.1}%)", stats.parse_success, parse_rate);
    println!("  Convert success: {} ({:.1}%)", stats.convert_success, convert_rate);
    
    // MVP目標: パース成功率 95%以上
    assert!(
        parse_rate >= 95.0,
        "Parse success rate ({:.1}%) below MVP target (95%)",
        parse_rate
    );
}

#[derive(Default)]
struct CorpusStats {
    total: u32,
    parse_success: u32,
    parse_with_errors: u32,
    parse_failure: u32,
    convert_success: u32,
    convert_with_warnings: u32,
    convert_failure: u32,
}
```

#### 9.9.2 スナップショットテスト

```rust
// tests/snapshot_test.rs

use insta::{assert_snapshot, assert_debug_snapshot};

#[test]
fn snapshot_simple_select() {
    let sql = "SELECT id, name FROM users WHERE active = 1";
    let result = tsqlremaker::parse(sql).unwrap();
    
    // AST のスナップショット
    assert_debug_snapshot!("simple_select_ast", &result.ast);
    
    // 変換結果のスナップショット
    let converted = tsqlremaker::convert(sql, Target::MySQL).unwrap();
    assert_snapshot!("simple_select_mysql", &converted.sql);
}

#[test]
fn snapshot_complex_join() {
    let sql = r#"
        SELECT 
            u.id,
            u.name,
            COUNT(o.id) as order_count
        FROM users u
        LEFT JOIN orders o ON u.id = o.user_id
        WHERE u.status = 'active'
        GROUP BY u.id, u.name
        HAVING COUNT(o.id) > 5
        ORDER BY order_count DESC
    "#;
    
    let result = tsqlremaker::parse(sql).unwrap();
    assert_debug_snapshot!("complex_join_ast", &result.ast);
    
    let converted = tsqlremaker::convert(sql, Target::MySQL).unwrap();
    assert_snapshot!("complex_join_mysql", &converted.sql);
}

#[test]
fn snapshot_ase_specific() {
    let sql = r#"
        SELECT TOP 10 
            GETDATE() as current_date,
            DATEADD(day, 7, created_at) as next_week,
            ISNULL(nickname, name) as display_name
        FROM users
        WHERE LEN(name) > 5
    "#;
    
    let converted = tsqlremaker::convert(sql, Target::MySQL).unwrap();
    assert_snapshot!("ase_specific_mysql", &converted.sql);
    assert_debug_snapshot!("ase_specific_warnings", &converted.warnings);
}
```

#### 9.9.3 ベンチマーク

```rust
// benches/parse_bench.rs

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_lexer(c: &mut Criterion) {
    let small_sql = "SELECT * FROM users WHERE id = 1";
    let medium_sql = include_str!("fixtures/medium_query.sql");
    let large_sql = include_str!("fixtures/large_procedure.sql");
    
    let mut group = c.benchmark_group("lexer");
    
    for (name, sql) in [
        ("small", small_sql),
        ("medium", medium_sql),
        ("large", large_sql),
    ] {
        group.bench_with_input(
            BenchmarkId::new("tokenize", name),
            sql,
            |b, sql| {
                b.iter(|| {
                    let lexer = Lexer::new(sql);
                    let _tokens: Vec<_> = lexer.collect::<Result<_, _>>().unwrap();
                });
            },
        );
    }
    
    group.finish();
}

fn bench_parser(c: &mut Criterion) {
    // similar structure
}

fn bench_convert(c: &mut Criterion) {
    // similar structure
}

criterion_group!(benches, bench_lexer, bench_parser, bench_convert);
criterion_main!(benches);
```

---

### 9.10 差分ツール（sqldef相当）【Post-MVP - Could】

> **注**: この機能はPost-MVPとして将来実装を検討。

#### 9.10.1 DDL正規化

```rust
/// DDL正規化オプション
pub struct NormalizeOptions {
    /// 列の並び順を名前でソート
    pub sort_columns: bool,
    /// 制約名を正規化（自動生成名を統一）
    pub normalize_constraint_names: bool,
    /// デフォルト値の正規化
    pub normalize_defaults: bool,
}

/// 正規化されたDDL表現
pub struct NormalizedDdl {
    pub tables: Vec<NormalizedTable>,
    pub indexes: Vec<NormalizedIndex>,
    pub procedures: Vec<NormalizedProcedure>,
}
```

#### 9.10.2 差分検出

```rust
/// DDL差分
pub struct DdlDiff {
    pub added_tables: Vec<TableDef>,
    pub dropped_tables: Vec<String>,
    pub altered_tables: Vec<TableDiff>,
    pub added_indexes: Vec<IndexDef>,
    pub dropped_indexes: Vec<String>,
}

/// テーブル差分
pub struct TableDiff {
    pub table_name: String,
    pub added_columns: Vec<ColumnDef>,
    pub dropped_columns: Vec<String>,
    pub altered_columns: Vec<ColumnDiff>,
    pub added_constraints: Vec<ConstraintDef>,
    pub dropped_constraints: Vec<String>,
}

/// 列差分
pub struct ColumnDiff {
    pub column_name: String,
    pub old_type: Option<DataType>,
    pub new_type: Option<DataType>,
    pub old_nullable: Option<bool>,
    pub new_nullable: Option<bool>,
    pub old_default: Option<String>,
    pub new_default: Option<String>,
}
```

---

### 9.11 要件の優先度まとめ

| プロンプト | 要件 | 優先度 | フェーズ |
|-----------|------|--------|----------|
| **Rust制約** | panic禁止、Result統一 | **MANDATORY** | Phase 1 |
| **#1** | IR/Protobufスキーマ | **MVP-Must** | Phase 2 |
| **#2** | Protobuf分割（IR/Index） | **MVP-Must** | Phase 2 |
| **#3** | Span全工程適用 | **MVP-Must** | Phase 1 |
| **#4** | エラー回復 | **MVP-Must** | Phase 2 |
| **#5** | 意味解析（名前解決） | Post-MVP | Phase 3 |
| **#6** | 動的SQL扱い | Post-MVP | Phase 4 |
| **#7** | 共通API | **MVP-Must** | Phase 2 |
| **#8** | 変換器要件 | **MVP-Must** | Phase 3 |
| **#9** | 差分ツール | Could | Phase 5+ |
| **#10** | 性能・コーパステスト | **MVP-Must** | Phase 1-4 |

---

### 9.12 更新されたCrate構成

```
tsqlremaker/
├── Cargo.toml                          # Workspace root
├── proto/
│   ├── common.proto                    # 共通定義
│   ├── tsqlremaker_ir.proto            # 変換パイプライン用
│   └── tsqlremaker_index.proto         # インデックス用
│
├── crates/
│   ├── tsql-token/                     # Token定義
│   ├── tsql-lexer/                     # 字句解析（panic禁止、Result<T, LexError>）
│   ├── tsql-parser/                    # 構文解析（エラー回復対応）
│   ├── common-sql/                     # 共通AST/IR定義
│   ├── proto-mapper/                   # Rust ⇄ Protobuf変換
│   ├── tsql-indexer/                   # シンボルインデックス
│   ├── mysql-emitter/                  # MySQL出力
│   ├── postgresql-emitter/             # PostgreSQL出力
│   └── tsql-cli/                       # CLIバイナリ
│
├── tests/
│   ├── corpus/                         # 実SQLコーパス
│   ├── snapshots/                      # スナップショット
│   └── fixtures/                       # テストデータ
│
└── benches/                            # ベンチマーク
```

---

*このドキュメントは tsqlremaker プロジェクトの設計指針を定義するものであり、実装の進行に伴い更新されます。*
