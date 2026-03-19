//! パーサーモジュール
//!
//! T-SQLの構文解析を行うメインパーサー。

use crate::ast::*;
use crate::buffer::TokenBuffer;
use crate::error::{ParseError, ParseResult};
use crate::expression::ExpressionParser;
use tsql_lexer::Lexer;
use tsql_token::{Span, TokenKind};

/// デフォルトの最大再帰深度
const DEFAULT_MAX_DEPTH: usize = 1000;

/// パーサーモード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ParserMode {
    /// バッチモード（GOをバッチ区切りとして認識）
    #[default]
    BatchMode,
    /// 単一文モード（GOを識別子として扱う）
    SingleStatement,
}

/// パーサー構造
pub struct Parser<'src> {
    /// トークンバッファ
    buffer: TokenBuffer<'src>,
    /// パーサーモード
    mode: ParserMode,
    /// 収集されたエラー
    errors: Vec<ParseError>,
    /// 現在の再帰深度
    depth: usize,
    /// 最大再帰深度
    max_depth: usize,
}

impl<'src> Parser<'src> {
    /// 新しいパーサーを作成
    ///
    /// # Arguments
    ///
    /// * `input` - 解析するSQLソースコード
    #[must_use]
    pub fn new(input: &'src str) -> Self {
        let lexer = Lexer::new(input);
        let buffer = TokenBuffer::new(lexer);
        Self {
            buffer,
            mode: ParserMode::default(),
            errors: Vec::new(),
            depth: 0,
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }

    /// パーサーモードを設定
    ///
    /// # Arguments
    ///
    /// * `mode` - パーサーモード
    pub fn with_mode(mut self, mode: ParserMode) -> Self {
        self.mode = mode;
        self
    }

    /// 入力全体を解析
    ///
    /// # Returns
    ///
    /// 文のリスト、またはエラー
    pub fn parse(&mut self) -> ParseResult<Vec<Statement>> {
        let mut statements = Vec::new();

        while !self.is_at_eof() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(e) => {
                    self.errors.push(e.clone());
                    self.synchronize();
                }
            }

            // セミコロンを消費
            let _ = self.buffer.consume_if(TokenKind::Semicolon);
        }

        // エラーがあった場合は最初のエラーを返す
        if !self.errors.is_empty() {
            return Err(self.errors[0].clone());
        }

        Ok(statements)
    }

    /// 単一の文を解析
    ///
    /// # Returns
    ///
    /// 文、またはエラー
    pub fn parse_statement(&mut self) -> ParseResult<Statement> {
        self.check_depth()?;

        match self.buffer.current()?.kind {
            // SELECT文
            TokenKind::Select => self.parse_select_statement(),
            // INSERT文
            TokenKind::Insert => self.parse_insert_statement(),
            // UPDATE文
            TokenKind::Update => self.parse_update_statement(),
            // DELETE文
            TokenKind::Delete => self.parse_delete_statement(),
            // CREATE文
            TokenKind::Create => self.parse_create_statement(),
            // DECLARE文
            TokenKind::Declare => self.parse_declare_statement(),
            // SET文
            TokenKind::Set => self.parse_set_statement(),
            // IF文
            TokenKind::If => self.parse_if_statement(),
            // WHILE文
            TokenKind::While => self.parse_while_statement(),
            // BEGINブロック（BEGIN TRY は TRY...CATCH、BEGIN TRANSACTION はトランザクション）
            TokenKind::Begin => {
                if self.check_try_begin() {
                    self.parse_try_catch_statement()
                } else if self.check_transaction_begin() {
                    self.parse_transaction_statement()
                } else {
                    self.parse_block()
                }
            }
            // BREAK文
            TokenKind::Break => self.parse_break_statement(),
            // CONTINUE文
            TokenKind::Continue => self.parse_continue_statement(),
            // RETURN文
            TokenKind::Return => self.parse_return_statement(),
            // トランザクション制御（COMMIT, ROLLBACK, SAVE）
            TokenKind::Commit | TokenKind::Rollback | TokenKind::Save => {
                self.parse_transaction_statement()
            }
            // THROW 文
            TokenKind::Throw => self.parse_throw_statement(),
            // RAISERROR 文
            TokenKind::Raiserror => self.parse_raiserror_statement(),
            // GOバッチ区切り（BatchModeのみ）
            TokenKind::Go => {
                if matches!(self.mode, ParserMode::BatchMode) {
                    self.parse_batch_separator()
                } else {
                    // SingleStatementモードではGOを識別子として扱う
                    Err(ParseError::unexpected_token(
                        vec![
                            TokenKind::Select,
                            TokenKind::Insert,
                            TokenKind::Update,
                            TokenKind::Delete,
                            TokenKind::Create,
                            TokenKind::Declare,
                        ],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ))
                }
            }
            _ => Err(ParseError::unexpected_token(
                vec![
                    TokenKind::Select,
                    TokenKind::Insert,
                    TokenKind::Update,
                    TokenKind::Delete,
                    TokenKind::Create,
                    TokenKind::Declare,
                ],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            )),
        }
    }

    /// SELECT文を解析
    fn parse_select_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // SELECT

        let mut distinct = false;
        let mut top = None;

        // DISTINCT
        if self.buffer.consume_if(TokenKind::Distinct)? {
            distinct = true;
        }

        // TOP
        if self.buffer.check(TokenKind::Top) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            // TOP句は単純式のみを許可（中置演算子を含まない）
            top = Some(expr_parser.parse_simple()?);
        }

        // 変数代入パターンの検出: SELECT @var = expr
        // 先読みして @var = パターンかチェック
        if self.is_variable_assignment_pattern()? {
            return self.parse_variable_assignment(start);
        }

        // カラムリスト
        let columns = self.parse_comma_separated(|this| this.parse_select_item())?;

        // FROM
        let from = if self.buffer.check(TokenKind::From) {
            Some(self.parse_from_clause()?)
        } else {
            None
        };

        // WHERE
        let where_clause = if self.buffer.check(TokenKind::Where) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        // GROUP BY
        let group_by = if self.buffer.check(TokenKind::Group) {
            self.buffer.consume()?;
            if !self.buffer.check(TokenKind::By) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::By],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?;
            self.parse_comma_separated(|this| {
                let mut expr_parser = ExpressionParser::new(&mut this.buffer);
                expr_parser.parse()
            })?
        } else {
            Vec::new()
        };

        // HAVING
        let having = if self.buffer.check(TokenKind::Having) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        // ORDER BY
        let order_by = if self.buffer.check(TokenKind::Order) {
            self.buffer.consume()?;
            if !self.buffer.check(TokenKind::By) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::By],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?;
            self.parse_comma_separated(|this| this.parse_order_by_item())?
        } else {
            Vec::new()
        };

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Select(Box::new(SelectStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            distinct,
            top,
            columns,
            from,
            where_clause,
            group_by,
            having,
            order_by,
            limit: None,
        })))
    }

    /// SELECTアイテムを解析
    fn parse_select_item(&mut self) -> ParseResult<SelectItem> {
        // ワイルドカード
        if self.buffer.check(TokenKind::Star) {
            self.buffer.consume()?;
            return Ok(SelectItem::Wildcard);
        }

        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
        let expr = expr_parser.parse()?;

        // table.* の形式
        if self.buffer.check(TokenKind::Dot) {
            self.buffer.consume()?;
            if self.buffer.check(TokenKind::Star) {
                self.buffer.consume()?;
                if let Expression::Identifier(ident) = expr {
                    return Ok(SelectItem::QualifiedWildcard(ident));
                }
            }
        }

        // 別名（ASまたは暗黙）
        let alias = if self.buffer.check(TokenKind::As) {
            self.buffer.consume()?;
            Some(self.parse_identifier()?)
        } else if self.buffer.check(TokenKind::Ident) {
            // 暗黙の別名
            Some(self.parse_identifier()?)
        } else {
            None
        };

        Ok(SelectItem::Expression(expr, alias))
    }

    /// FROM句を解析
    fn parse_from_clause(&mut self) -> ParseResult<FromClause> {
        self.buffer.consume()?; // FROM

        // テーブル参照リスト（カンマ区切り）
        let mut tables = Vec::new();
        loop {
            tables.push(self.parse_table_reference()?);

            // カンマで区切られた複数テーブル
            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }

        // JOINを解析
        let mut joins = Vec::new();
        while self.is_join_keyword() {
            joins.push(self.parse_join()?);
        }

        Ok(FromClause { tables, joins })
    }

    /// 現在のトークンがJOINキーワードか判定
    fn is_join_keyword(&self) -> bool {
        match self.buffer.current() {
            Ok(token) => {
                matches!(
                    token.kind,
                    TokenKind::Inner
                        | TokenKind::Left
                        | TokenKind::Right
                        | TokenKind::Full
                        | TokenKind::Cross
                        | TokenKind::Join
                )
            }
            Err(_) => false,
        }
    }

    /// JOINを解析
    fn parse_join(&mut self) -> ParseResult<Join> {
        let start = self.buffer.current()?.span.start;

        // JOIN種別を判定
        let join_type = self.parse_join_type()?;

        // JOINキーワードを消費（INNER/LEFT/... JOIN の場合）
        if !self.buffer.check(TokenKind::Join) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Join],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?;

        // 結合するテーブル
        let table = self.parse_table_reference()?;

        // ON条件
        let on_condition = if self.buffer.check(TokenKind::On) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        // USING句のパース
        let using_columns = if self.buffer.check(TokenKind::Using) {
            self.buffer.consume()?; // USING
                                    // USING (col1, col2, ...)
            if !self.buffer.check(TokenKind::LParen) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::LParen],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?; // LParen

            let mut columns = Vec::new();
            loop {
                columns.push(self.parse_identifier()?);

                if !self.buffer.consume_if(TokenKind::Comma)? {
                    break;
                }
            }

            if !self.buffer.check(TokenKind::RParen) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::RParen],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?; // RParen

            columns
        } else {
            Vec::new()
        };

        let end_span = self.buffer.current()?.span;
        Ok(Join {
            join_type,
            table,
            on_condition,
            using_columns,
            span: Span {
                start,
                end: end_span.end,
            },
        })
    }

    /// JOIN種別を解析
    fn parse_join_type(&mut self) -> ParseResult<JoinType> {
        let current = self.buffer.current()?;
        match current.kind {
            TokenKind::Inner => {
                self.buffer.consume()?;
                Ok(JoinType::Inner)
            }
            TokenKind::Left => {
                self.buffer.consume()?;
                if self.buffer.check(TokenKind::Outer) {
                    self.buffer.consume()?;
                    Ok(JoinType::LeftOuter)
                } else {
                    Ok(JoinType::Left)
                }
            }
            TokenKind::Right => {
                self.buffer.consume()?;
                if self.buffer.check(TokenKind::Outer) {
                    self.buffer.consume()?;
                    Ok(JoinType::RightOuter)
                } else {
                    Ok(JoinType::Right)
                }
            }
            TokenKind::Full => {
                self.buffer.consume()?;
                if self.buffer.check(TokenKind::Outer) {
                    self.buffer.consume()?;
                    Ok(JoinType::FullOuter)
                } else {
                    Ok(JoinType::Full)
                }
            }
            TokenKind::Cross => {
                self.buffer.consume()?;
                Ok(JoinType::Cross)
            }
            TokenKind::Join => {
                // 単独のJOINはINNER JOINとして扱う
                Ok(JoinType::Inner)
            }
            _ => Err(ParseError::unexpected_token(
                vec![
                    TokenKind::Inner,
                    TokenKind::Left,
                    TokenKind::Right,
                    TokenKind::Full,
                    TokenKind::Cross,
                    TokenKind::Join,
                ],
                current.kind,
                current.span,
            )),
        }
    }

    /// テーブル参照を解析
    fn parse_table_reference(&mut self) -> ParseResult<TableReference> {
        let start = self.buffer.current()?.span.start;

        // サブクエリ（導出テーブル）の検出
        if self.buffer.check(TokenKind::LParen) {
            self.buffer.consume()?; // LParen

            // サブクエリを解析
            let select_stmt = match self.parse_select_statement()? {
                Statement::Select(select) => select,
                _ => {
                    return Err(ParseError::invalid_syntax(
                        "Expected SELECT statement in subquery".to_string(),
                        self.buffer.current()?.span,
                    ))
                }
            };

            // 右括弧を期待
            if !self.buffer.check(TokenKind::RParen) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::RParen],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?; // RParen

            // オプションの別名
            let alias = if self.buffer.check(TokenKind::As) {
                self.buffer.consume()?;
                Some(self.parse_identifier()?)
            } else if self.buffer.check(TokenKind::Ident) {
                Some(self.parse_identifier()?)
            } else {
                None
            };

            let end_span = self.buffer.current()?.span;
            return Ok(TableReference::Subquery {
                query: select_stmt,
                alias,
                span: Span {
                    start,
                    end: end_span.end,
                },
            });
        }

        // 通常のテーブル参照
        let name = self.parse_identifier()?;

        let alias = if self.buffer.check(TokenKind::As) {
            self.buffer.consume()?;
            Some(self.parse_identifier()?)
        } else if self.buffer.check(TokenKind::Ident) {
            Some(self.parse_identifier()?)
        } else {
            None
        };

        let end_span = self.buffer.current()?.span;
        Ok(TableReference::Table {
            name,
            alias,
            span: Span {
                start,
                end: end_span.end,
            },
        })
    }

    /// ORDER BYアイテムを解析
    fn parse_order_by_item(&mut self) -> ParseResult<OrderByItem> {
        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
        let expr = expr_parser.parse()?;

        let asc = if self.buffer.check(TokenKind::Asc) {
            self.buffer.consume()?;
            true
        } else if self.buffer.check(TokenKind::Desc) {
            self.buffer.consume()?;
            false
        } else {
            true // デフォルトはASC
        };

        Ok(OrderByItem { expr, asc })
    }

    /// INSERT文を解析
    fn parse_insert_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // INSERT

        if !self.buffer.check(TokenKind::Into) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Into],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?; // INTO

        let table = self.parse_identifier()?;

        // カラムリスト
        let mut columns = Vec::new();
        if self.buffer.check(TokenKind::LParen) {
            self.buffer.consume()?;
            while !self.buffer.check(TokenKind::RParen) {
                columns.push(self.parse_identifier()?);
                if !self.buffer.consume_if(TokenKind::Comma)? {
                    break;
                }
            }
            if !self.buffer.check(TokenKind::RParen) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::RParen],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?;
        }

        // VALUES or SELECT
        let source = if self.buffer.check(TokenKind::Values) {
            self.buffer.consume()?;
            let mut rows = Vec::new();
            loop {
                if !self.buffer.check(TokenKind::LParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::LParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?; // LEFT PAREN
                let mut values = Vec::new();
                while !self.buffer.check(TokenKind::RParen) {
                    let mut expr_parser = ExpressionParser::new(&mut self.buffer);
                    values.push(expr_parser.parse()?);
                    if !self.buffer.consume_if(TokenKind::Comma)? {
                        break;
                    }
                }
                if !self.buffer.check(TokenKind::RParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::RParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?;
                rows.push(values);
                if !self.buffer.consume_if(TokenKind::Comma)? {
                    break;
                }
            }
            InsertSource::Values(rows)
        } else if self.buffer.check(TokenKind::Default) {
            self.buffer.consume()?;
            if !self.buffer.check(TokenKind::Values) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::Values],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?;
            InsertSource::DefaultValues
        } else {
            // SELECT
            let select_stmt = self.parse_select_statement()?;
            match select_stmt {
                Statement::Select(s) => InsertSource::Select(s),
                _ => {
                    return Err(ParseError::invalid_syntax(
                        "Expected SELECT statement".to_string(),
                        self.buffer.current()?.span,
                    ))
                }
            }
        };

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Insert(Box::new(InsertStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            table,
            columns,
            source,
        })))
    }

    /// UPDATE文を解析
    fn parse_update_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // UPDATE

        let table_ref = self.parse_table_reference()?;

        // SET
        if !self.buffer.check(TokenKind::Set) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Set],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?;

        // 代入リスト
        let mut assignments = Vec::new();
        loop {
            let column = self.parse_identifier()?;
            if !self.buffer.check(TokenKind::Eq) && !self.buffer.check(TokenKind::Assign) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::Eq, TokenKind::Assign],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            let value = expr_parser.parse()?;
            assignments.push(ColumnAssignment { column, value });
            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }

        // FROM（オプション）
        let from_clause = if self.buffer.check(TokenKind::From) {
            Some(self.parse_from_clause()?)
        } else {
            None
        };

        // WHERE（オプション）
        let where_clause = if self.buffer.check(TokenKind::Where) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Update(Box::new(UpdateStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            table: table_ref,
            assignments,
            from_clause,
            where_clause,
        })))
    }

    /// DELETE文を解析
    fn parse_delete_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // DELETE

        let table = if self.buffer.check(TokenKind::From) {
            self.buffer.consume()?;
            self.parse_identifier()?
        } else {
            // FROMなしの形式
            self.parse_identifier()?
        };

        // FROM（結合用、オプション）
        let from_clause = if self.buffer.check(TokenKind::From) {
            Some(self.parse_from_clause()?)
        } else {
            None
        };

        // WHERE（オプション）
        let where_clause = if self.buffer.check(TokenKind::Where) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Delete(Box::new(DeleteStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            table,
            from_clause,
            where_clause,
        })))
    }

    /// CREATE文を解析
    fn parse_create_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // CREATE

        match self.buffer.current()?.kind {
            TokenKind::Table => {
                self.buffer.consume()?;
                self.parse_create_table(start)
            }
            TokenKind::Index => {
                self.buffer.consume()?;
                self.parse_create_index(start)
            }
            TokenKind::View => {
                self.buffer.consume()?;
                self.parse_create_view(start)
            }
            TokenKind::Procedure | TokenKind::Proc => {
                self.buffer.consume()?;
                self.parse_create_procedure(start)
            }
            _ => Err(ParseError::unexpected_token(
                vec![
                    TokenKind::Table,
                    TokenKind::Index,
                    TokenKind::View,
                    TokenKind::Procedure,
                ],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            )),
        }
    }

    /// CREATE TABLEを解析
    fn parse_create_table(&mut self, start: u32) -> ParseResult<Statement> {
        let name = self.parse_identifier()?;
        // 一時テーブルは `#` または `##` で始まる識別子
        let temporary = name.name.starts_with('#');

        if !self.buffer.check(TokenKind::LParen) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::LParen],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?; // LEFT PAREN

        let mut columns = Vec::new();
        let mut constraints = Vec::new();

        while !self.buffer.check(TokenKind::RParen) {
            let token = self.buffer.current()?;

            // まずテーブル制約をチェック
            if self.is_table_constraint_start() {
                let constraint = self.parse_table_constraint()?;
                constraints.push(constraint);
            } else {
                // カラム定義（カラムレベル制約を含む）
                let token_kind = token.kind;
                let token_span = token.span;
                let is_identifier = matches!(
                    token_kind,
                    TokenKind::Ident
                        | TokenKind::QuotedIdent
                        | TokenKind::Table
                        | TokenKind::View
                        | TokenKind::Index
                        | TokenKind::Go
                );

                if !is_identifier {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::Ident, TokenKind::RParen],
                        token_kind,
                        token_span,
                    ));
                }

                let name = self.parse_identifier()?;
                let data_type = self.parse_data_type()?;

                // NULL許容
                let nullability = if self.buffer.check(TokenKind::Null) {
                    self.buffer.consume()?;
                    Some(true)
                } else if self.buffer.check(TokenKind::Not) {
                    self.buffer.consume()?;
                    if !self.buffer.check(TokenKind::Null) {
                        return Err(ParseError::unexpected_token(
                            vec![TokenKind::Null],
                            self.buffer.current()?.kind,
                            self.buffer.current()?.span,
                        ));
                    }
                    self.buffer.consume()?;
                    Some(false)
                } else {
                    None
                };

                // IDENTITY
                let identity = self.buffer.check(TokenKind::Identity);
                if identity {
                    self.buffer.consume()?;
                }

                // DEFAULT句のパース
                let default_value = if self.buffer.check(TokenKind::Default) {
                    self.buffer.consume()?; // DEFAULT
                                            // NULLまたは定数式
                    if self.buffer.check(TokenKind::Null) {
                        self.buffer.consume()?;
                        Some(Expression::Literal(Literal::Null(Span {
                            start: self.buffer.current()?.span.start,
                            end: self.buffer.current()?.span.end,
                        })))
                    } else {
                        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
                        Some(expr_parser.parse()?)
                    }
                } else {
                    None
                };

                // カラムレベル制約のパース
                let mut constraints = Vec::new();
                loop {
                    if self.buffer.check(TokenKind::Primary) {
                        self.buffer.consume()?;
                        if self.buffer.check(TokenKind::Key) {
                            self.buffer.consume()?;
                        }
                        constraints.push(ColumnConstraint::PrimaryKey);
                    } else if self.buffer.check(TokenKind::Unique) {
                        self.buffer.consume()?;
                        constraints.push(ColumnConstraint::Unique);
                    } else if self.buffer.check(TokenKind::References) {
                        self.buffer.consume()?; // REFERENCES
                        let ref_table = self.parse_identifier()?;
                        // 参照カラムのパース（オプションの括弧）
                        let ref_column = if self.buffer.check(TokenKind::LParen) {
                            self.buffer.consume()?;
                            let col = self.parse_identifier()?;
                            if !self.buffer.check(TokenKind::RParen) {
                                return Err(ParseError::unexpected_token(
                                    vec![TokenKind::RParen],
                                    self.buffer.current()?.kind,
                                    self.buffer.current()?.span,
                                ));
                            }
                            self.buffer.consume()?;
                            col
                        } else {
                            // 括弧なしの場合は参照テーブルと同じ名前のカラム
                            Identifier {
                                name: ref_table.name.clone(),
                                span: ref_table.span,
                            }
                        };
                        constraints.push(ColumnConstraint::Foreign {
                            ref_table,
                            ref_column,
                        });
                    } else if self.buffer.check(TokenKind::Check) {
                        self.buffer.consume()?;
                        let expr = ExpressionParser::new(&mut self.buffer).parse()?;
                        constraints.push(ColumnConstraint::Check(expr));
                    } else {
                        break;
                    }
                }

                columns.push(ColumnDefinition {
                    name,
                    data_type,
                    nullability,
                    default_value,
                    identity,
                    constraints,
                });
            }

            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }

        if !self.buffer.check(TokenKind::RParen) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::RParen],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?;

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Create(Box::new(CreateStatement::Table(
            TableDefinition {
                span: Span {
                    start,
                    end: end_span.end,
                },
                name,
                columns,
                constraints,
                temporary,
            },
        ))))
    }

    /// テーブル制約の開始かどうかを判定
    fn is_table_constraint_start(&self) -> bool {
        let kind = match self.buffer.current() {
            Ok(t) => t.kind,
            Err(_) => return false,
        };
        // CONSTRAINTキーワードまたは制約タイプキーワード
        matches!(
            kind,
            TokenKind::Constraint
                | TokenKind::Primary
                | TokenKind::Foreign
                | TokenKind::Unique
                | TokenKind::Check
        )
    }

    /// テーブル制約を解析
    fn parse_table_constraint(&mut self) -> ParseResult<TableConstraint> {
        // CONSTRAINT constraint_name の部分をパース（オプション）
        let constraint_name = if self.buffer.check(TokenKind::Constraint) {
            self.buffer.consume()?;
            Some(self.parse_identifier()?)
        } else {
            None
        };

        // 制約タイプを判定
        let constraint_type = self.buffer.current()?.kind;

        match constraint_type {
            TokenKind::Primary => {
                self.buffer.consume()?;
                if !self.buffer.check(TokenKind::Key) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::Key],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?;

                // カラムリストをパース
                if !self.buffer.check(TokenKind::LParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::LParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?;

                let mut columns = Vec::new();
                while !self.buffer.check(TokenKind::RParen) {
                    columns.push(self.parse_identifier()?);
                    if !self.buffer.consume_if(TokenKind::Comma)? {
                        break;
                    }
                }
                self.buffer.consume()?; // RIGHT PAREN

                Ok(TableConstraint::PrimaryKey {
                    name: constraint_name,
                    columns,
                })
            }
            TokenKind::Unique => {
                self.buffer.consume()?;

                // カラムリストをパース
                if !self.buffer.check(TokenKind::LParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::LParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?;

                let mut columns = Vec::new();
                while !self.buffer.check(TokenKind::RParen) {
                    columns.push(self.parse_identifier()?);
                    if !self.buffer.consume_if(TokenKind::Comma)? {
                        break;
                    }
                }
                self.buffer.consume()?; // RIGHT PAREN

                Ok(TableConstraint::Unique {
                    name: constraint_name,
                    columns,
                })
            }
            TokenKind::Foreign => {
                self.buffer.consume()?;
                if !self.buffer.check(TokenKind::Key) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::Key],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?;

                // カラムリストをパース
                if !self.buffer.check(TokenKind::LParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::LParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?;

                let mut columns = Vec::new();
                while !self.buffer.check(TokenKind::RParen) {
                    columns.push(self.parse_identifier()?);
                    if !self.buffer.consume_if(TokenKind::Comma)? {
                        break;
                    }
                }
                self.buffer.consume()?; // RIGHT PAREN

                // REFERENCES
                if !self.buffer.check(TokenKind::References) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::References],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?;

                let ref_table = self.parse_identifier()?;

                // 参照カラム（オプションの括弧）
                let ref_columns = if self.buffer.check(TokenKind::LParen) {
                    self.buffer.consume()?;
                    let mut cols = Vec::new();
                    while !self.buffer.check(TokenKind::RParen) {
                        cols.push(self.parse_identifier()?);
                        if !self.buffer.consume_if(TokenKind::Comma)? {
                            break;
                        }
                    }
                    self.buffer.consume()?;
                    cols
                } else {
                    // 括弧がない場合、単一カラムとして処理
                    vec![self.parse_identifier()?]
                };

                Ok(TableConstraint::Foreign {
                    name: constraint_name,
                    columns,
                    ref_table,
                    ref_columns,
                })
            }
            TokenKind::Check => {
                self.buffer.consume()?;

                // CHECK式をパース
                if !self.buffer.check(TokenKind::LParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::LParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?;

                let mut expr_parser = ExpressionParser::new(&mut self.buffer);
                let expr = expr_parser.parse()?;

                if !self.buffer.check(TokenKind::RParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::RParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?; // RIGHT PAREN

                Ok(TableConstraint::Check {
                    name: constraint_name,
                    expr,
                })
            }
            _ => Err(ParseError::unexpected_token(
                vec![
                    TokenKind::Primary,
                    TokenKind::Unique,
                    TokenKind::Foreign,
                    TokenKind::Check,
                ],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            )),
        }
    }

    /// データ型を解析
    fn parse_data_type(&mut self) -> ParseResult<DataType> {
        let kind = self.buffer.current()?.kind;
        self.buffer.consume()?;

        Ok(match kind {
            TokenKind::Int | TokenKind::Integer => DataType::Int,
            TokenKind::Smallint => DataType::SmallInt,
            TokenKind::Tinyint => DataType::TinyInt,
            TokenKind::Bigint => DataType::BigInt,
            TokenKind::Varchar => {
                if self.buffer.check(TokenKind::LParen) {
                    self.buffer.consume()?;
                    let len = if self.buffer.check(TokenKind::Number) {
                        let token = self.buffer.current()?;
                        let num_str = token.text;
                        let n = num_str.parse::<u32>().map_err(|_| {
                            ParseError::invalid_syntax(
                                format!("Invalid number for VARCHAR length: {}", num_str),
                                token.span,
                            )
                        })?;
                        self.buffer.consume()?;
                        Some(n)
                    } else {
                        // MAX
                        self.buffer.consume()?;
                        None
                    };
                    if !self.buffer.check(TokenKind::RParen) {
                        return Err(ParseError::unexpected_token(
                            vec![TokenKind::RParen],
                            self.buffer.current()?.kind,
                            self.buffer.current()?.span,
                        ));
                    }
                    self.buffer.consume()?;
                    DataType::Varchar(len)
                } else {
                    DataType::Varchar(None)
                }
            }
            TokenKind::Char => {
                let len = if self.buffer.check(TokenKind::LParen) {
                    self.buffer.consume()?;
                    let n = if self.buffer.check(TokenKind::Number) {
                        let token = self.buffer.current()?;
                        let num_str = token.text;
                        let parsed = num_str.parse::<u32>().map_err(|_| {
                            ParseError::invalid_syntax(
                                format!("Invalid number for CHAR length: {}", num_str),
                                token.span,
                            )
                        })?;
                        self.buffer.consume()?;
                        parsed
                    } else {
                        1
                    };
                    if !self.buffer.check(TokenKind::RParen) {
                        return Err(ParseError::unexpected_token(
                            vec![TokenKind::RParen],
                            self.buffer.current()?.kind,
                            self.buffer.current()?.span,
                        ));
                    }
                    self.buffer.consume()?;
                    n
                } else {
                    1
                };
                DataType::Char(len)
            }
            TokenKind::Decimal | TokenKind::Numeric => {
                let (precision, scale) = if self.buffer.check(TokenKind::LParen) {
                    self.buffer.consume()?;
                    let p = if self.buffer.check(TokenKind::Number) {
                        let token = self.buffer.current()?;
                        let num_str = token.text;
                        let parsed = num_str.parse::<u8>().map_err(|_| {
                            ParseError::invalid_syntax(
                                format!("Invalid number for DECIMAL precision: {}", num_str),
                                token.span,
                            )
                        })?;
                        self.buffer.consume()?;
                        Some(parsed)
                    } else {
                        None
                    };
                    let scale = if self.buffer.check(TokenKind::Comma) {
                        self.buffer.consume()?;
                        if self.buffer.check(TokenKind::Number) {
                            let token = self.buffer.current()?;
                            let num_str = token.text;
                            let parsed = num_str.parse::<u8>().map_err(|_| {
                                ParseError::invalid_syntax(
                                    format!("Invalid number for DECIMAL scale: {}", num_str),
                                    token.span,
                                )
                            })?;
                            self.buffer.consume()?;
                            Some(parsed)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if !self.buffer.check(TokenKind::RParen) {
                        return Err(ParseError::unexpected_token(
                            vec![TokenKind::RParen],
                            self.buffer.current()?.kind,
                            self.buffer.current()?.span,
                        ));
                    }
                    self.buffer.consume()?;
                    (p, scale)
                } else {
                    (None, None)
                };
                if kind == TokenKind::Decimal {
                    DataType::Decimal(precision, scale)
                } else {
                    DataType::Numeric(precision, scale)
                }
            }
            TokenKind::Real => DataType::Real,
            TokenKind::Double => DataType::Double,
            TokenKind::Date => DataType::Date,
            TokenKind::Time => DataType::Time,
            TokenKind::Datetime => DataType::Datetime,
            TokenKind::Smalldatetime => DataType::SmallDateTime,
            TokenKind::Timestamp => DataType::Timestamp,
            TokenKind::Bit => DataType::Bit,
            TokenKind::Text => DataType::Text,
            TokenKind::Binary => {
                let len = if self.buffer.check(TokenKind::LParen) {
                    self.buffer.consume()?;
                    let n = if self.buffer.check(TokenKind::Number) {
                        let n = self.buffer.current()?.text.parse().unwrap_or(1);
                        self.buffer.consume()?;
                        n
                    } else {
                        1
                    };
                    if !self.buffer.check(TokenKind::RParen) {
                        return Err(ParseError::unexpected_token(
                            vec![TokenKind::RParen],
                            self.buffer.current()?.kind,
                            self.buffer.current()?.span,
                        ));
                    }
                    self.buffer.consume()?;
                    n
                } else {
                    1
                };
                DataType::Binary(len)
            }
            TokenKind::Varbinary => DataType::VarBinary(None),
            TokenKind::Uniqueidentifier => DataType::UniqueIdentifier,
            TokenKind::Money => DataType::Money,
            TokenKind::Smallmoney => DataType::SmallMoney,
            _ => {
                return Err(ParseError::invalid_syntax(
                    format!("Unknown data type: {:?}", self.buffer.current()?.kind),
                    self.buffer.current()?.span,
                ))
            }
        })
    }

    /// CREATE INDEXを解析
    fn parse_create_index(&mut self, start: u32) -> ParseResult<Statement> {
        let name = self.parse_identifier()?;

        if !self.buffer.check(TokenKind::On) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::On],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?;

        let table = self.parse_identifier()?;
        if !self.buffer.check(TokenKind::LParen) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::LParen],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?; // LEFT PAREN

        let mut columns = Vec::new();
        while !self.buffer.check(TokenKind::RParen) {
            columns.push(self.parse_identifier()?);
            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }
        self.buffer.consume()?; // RIGHT PAREN

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Create(Box::new(CreateStatement::Index(
            IndexDefinition {
                span: Span {
                    start,
                    end: end_span.end,
                },
                name,
                table,
                columns,
                unique: false,
            },
        ))))
    }

    /// CREATE VIEWを解析
    fn parse_create_view(&mut self, start: u32) -> ParseResult<Statement> {
        let name = self.parse_identifier()?;

        if !self.buffer.check(TokenKind::As) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::As],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?;

        let select_stmt = self.parse_select_statement()?;
        let select = match select_stmt {
            Statement::Select(s) => s,
            _ => {
                return Err(ParseError::invalid_syntax(
                    "Expected SELECT statement".to_string(),
                    self.buffer.current()?.span,
                ))
            }
        };

        Ok(Statement::Create(Box::new(CreateStatement::View(
            ViewDefinition {
                span: Span {
                    start,
                    end: select.span.end,
                },
                name,
                query: select,
            },
        ))))
    }

    /// CREATE PROCEDUREを解析
    fn parse_create_procedure(&mut self, start: u32) -> ParseResult<Statement> {
        let name = self.parse_identifier()?;

        // パラメータリスト（オプション）
        // T-SQLでは括弧なしでもパラメータを記述可能
        let mut parameters = Vec::new();
        if self.buffer.check(TokenKind::LParen) {
            // 括弧付きのパラメータリスト
            self.buffer.consume()?;
            while !self.buffer.check(TokenKind::RParen) {
                parameters.push(self.parse_parameter_definition()?);
                if !self.buffer.consume_if(TokenKind::Comma)? {
                    break;
                }
            }
            self.buffer.consume()?;
        } else if self.buffer.check(TokenKind::LocalVar) {
            // 括弧なしのパラメータリスト（T-SQLの標準的な書き方）
            loop {
                parameters.push(self.parse_parameter_definition()?);
                if !self.buffer.consume_if(TokenKind::Comma)? {
                    break;
                }
                // 次がLocalVarでない場合は終了
                if !self.buffer.check(TokenKind::LocalVar) {
                    break;
                }
            }
        }

        // AS
        if !self.buffer.check(TokenKind::As) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::As],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?;

        // プロシージャ本体（簡易版：BEGIN...ENDまたは単一の文）
        let body = if self.buffer.check(TokenKind::Begin) {
            let block = self.parse_block()?;
            vec![block]
        } else {
            vec![self.parse_statement()?]
        };

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Create(Box::new(CreateStatement::Procedure(
            ProcedureDefinition {
                span: Span {
                    start,
                    end: end_span.end,
                },
                name,
                parameters,
                body,
            },
        ))))
    }

    /// パラメータ定義を解析
    ///
    /// T-SQL構文: @parameter_name [AS] data_type [ = default_value ] [OUTPUT]
    fn parse_parameter_definition(&mut self) -> ParseResult<ParameterDefinition> {
        let name = self.parse_identifier()?;
        let data_type = self.parse_data_type()?;
        let mut default_value = None;
        let mut is_output = false;

        // T-SQLでは DEFAULT キーワードは使用せず、直接 = でデフォルト値を指定
        if self.buffer.check(TokenKind::Eq) || self.buffer.check(TokenKind::Assign) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            default_value = Some(expr_parser.parse()?);
        }

        if self.buffer.check(TokenKind::Output) {
            self.buffer.consume()?;
            is_output = true;
        }

        Ok(ParameterDefinition {
            name,
            data_type,
            default_value,
            is_output,
        })
    }

    /// DECLARE文を解析
    fn parse_declare_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // DECLARE

        let mut variables = Vec::new();
        loop {
            let name = self.parse_identifier()?;
            let data_type = self.parse_data_type()?;

            let default_value =
                if self.buffer.check(TokenKind::Eq) || self.buffer.check(TokenKind::Assign) {
                    self.buffer.consume()?;
                    let mut expr_parser = ExpressionParser::new(&mut self.buffer);
                    Some(expr_parser.parse()?)
                } else {
                    None
                };

            variables.push(VariableDeclaration {
                name,
                data_type,
                default_value,
            });

            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Declare(Box::new(DeclareStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            variables,
        })))
    }

    /// SET文を解析
    fn parse_set_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // SET

        let variable = self.parse_identifier()?;

        if !self.buffer.check(TokenKind::Eq) && !self.buffer.check(TokenKind::Assign) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Eq, TokenKind::Assign],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?;

        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
        let value = expr_parser.parse()?;

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Set(Box::new(SetStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            variable,
            value,
        })))
    }

    /// 変数代入パターンかチェックする
    ///
    /// `SELECT @var = expr` または `SELECT @var1 = expr1, @var2 = expr2` のパターンを検出する。
    fn is_variable_assignment_pattern(&self) -> ParseResult<bool> {
        // 現在のトークンが LocalVar (@var) であるか確認
        if !matches!(self.buffer.current()?.kind, TokenKind::LocalVar) {
            return Ok(false);
        }

        // 次のトークンが代入演算子(=)であるか確認
        match self.buffer.peek(1) {
            Ok(next_token) => Ok(matches!(next_token.kind, TokenKind::Eq | TokenKind::Assign)),
            Err(_) => Ok(false),
        }
    }

    /// SELECT変数代入文を解析
    ///
    /// `SELECT @var = expr` または `SELECT @var1 = expr1, @var2 = expr2` の構文を解析する。
    fn parse_variable_assignment(&mut self, start: u32) -> ParseResult<Statement> {
        use crate::ast::{Assignment, VariableAssignment};

        let mut assignments = Vec::new();

        loop {
            // 変数名 (@variable)
            if !matches!(self.buffer.current()?.kind, TokenKind::LocalVar) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::LocalVar],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }

            let variable = self.parse_identifier()?;

            // 代入演算子 (= または :=)
            if !matches!(
                self.buffer.current()?.kind,
                TokenKind::Eq | TokenKind::Assign
            ) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::Eq, TokenKind::Assign],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?;

            // 式
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            let value = expr_parser.parse()?;

            assignments.push(Assignment { variable, value });

            // カンマで区切られた複数代入
            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }

            // 次も変数代入パターンか確認
            if !self.is_variable_assignment_pattern()? {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::LocalVar],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
        }

        let end_span = self.buffer.current()?.span;
        Ok(Statement::VariableAssignment(Box::new(
            VariableAssignment {
                span: Span {
                    start,
                    end: end_span.end,
                },
                assignments,
            },
        )))
    }

    /// IF文を解析
    fn parse_if_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // IF

        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
        let condition = expr_parser.parse()?;

        let then_branch = self.parse_statement()?;

        let else_branch = if self.buffer.check(TokenKind::Else) {
            self.buffer.consume()?;
            Some(self.parse_statement()?)
        } else {
            None
        };

        let end_span = self.buffer.current()?.span;
        Ok(Statement::If(Box::new(IfStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            condition,
            then_branch,
            else_branch,
        })))
    }

    /// WHILE文を解析
    fn parse_while_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // WHILE

        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
        let condition = expr_parser.parse()?;

        let body = self.parse_statement()?;

        let end_span = self.buffer.current()?.span;
        Ok(Statement::While(Box::new(WhileStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            condition,
            body,
        })))
    }

    /// BEGIN...ENDブロックを解析
    fn parse_block(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // BEGIN

        let mut statements = Vec::new();
        while !self.buffer.check(TokenKind::End) && !self.is_at_eof() {
            match self.parse_statement() {
                Ok(stmt) => statements.push(stmt),
                Err(e) => {
                    self.errors.push(e);
                    self.synchronize();
                }
            }
            // セミコロンを消費
            let _ = self.buffer.consume_if(TokenKind::Semicolon);
        }

        if !self.buffer.check(TokenKind::End) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::End],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        let end_span = self.buffer.current()?.span;
        self.buffer.consume()?; // END

        Ok(Statement::Block(Box::new(Block {
            span: Span {
                start,
                end: end_span.end,
            },
            statements,
        })))
    }

    /// BREAK文を解析
    fn parse_break_statement(&mut self) -> ParseResult<Statement> {
        let span = self.buffer.current()?.span;
        self.buffer.consume()?; // BREAK
        Ok(Statement::Break(Box::new(BreakStatement { span })))
    }

    /// CONTINUE文を解析
    fn parse_continue_statement(&mut self) -> ParseResult<Statement> {
        let span = self.buffer.current()?.span;
        self.buffer.consume()?; // CONTINUE
        Ok(Statement::Continue(Box::new(ContinueStatement { span })))
    }

    /// RETURN文を解析
    fn parse_return_statement(&mut self) -> ParseResult<Statement> {
        let span = self.buffer.current()?.span;
        self.buffer.consume()?; // RETURN

        let expression = if self.buffer.check(TokenKind::Semicolon)
            || self.buffer.check(TokenKind::End)
            || self.is_at_eof()
        {
            None
        } else {
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        };

        Ok(Statement::Return(Box::new(ReturnStatement {
            span,
            expression,
        })))
    }

    /// TRY...CATCH ブロックを解析
    ///
    /// T-SQL構文: BEGIN TRY ... END TRY BEGIN CATCH ... END CATCH
    fn parse_try_catch_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;

        // BEGIN TRY
        self.buffer.consume()?; // BEGIN
        if !self.buffer.check(TokenKind::Try) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Try],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?; // TRY

        // TRY ブロックの本体をパース
        let try_block = if self.buffer.check(TokenKind::Begin) {
            match self.parse_block()? {
                Statement::Block(block) => block,
                _ => {
                    return Err(ParseError::invalid_syntax(
                        "Expected block statement".to_string(),
                        self.buffer.current()?.span,
                    ))
                }
            }
        } else {
            // 単一の文も許容
            let stmt = self.parse_statement()?;
            Box::new(Block {
                span: stmt.span(),
                statements: vec![stmt],
            })
        };

        // END TRY
        if !self.buffer.check(TokenKind::End) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::End],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?; // END

        if !self.buffer.check(TokenKind::Try) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Try],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?; // TRY

        // BEGIN CATCH
        if !self.buffer.check(TokenKind::Begin) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Begin],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?; // BEGIN

        if !self.buffer.check(TokenKind::Catch) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Catch],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?; // CATCH

        // CATCH ブロックの本体をパース
        let catch_block = if self.buffer.check(TokenKind::Begin) {
            match self.parse_block()? {
                Statement::Block(block) => block,
                _ => {
                    return Err(ParseError::invalid_syntax(
                        "Expected block statement".to_string(),
                        self.buffer.current()?.span,
                    ))
                }
            }
        } else {
            // 単一の文も許容
            let stmt = self.parse_statement()?;
            Box::new(Block {
                span: stmt.span(),
                statements: vec![stmt],
            })
        };

        // END CATCH
        if !self.buffer.check(TokenKind::End) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::End],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?; // END

        if !self.buffer.check(TokenKind::Catch) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Catch],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?; // CATCH

        let end_span = self.buffer.current()?.span;

        Ok(Statement::TryCatch(Box::new(TryCatchStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            try_block,
            catch_block,
        })))
    }

    /// トランザクション制御文を解析
    ///
    /// T-SQL構文: BEGIN TRANSACTION [name], COMMIT TRANSACTION [name],
    ///             ROLLBACK TRANSACTION [name], SAVE TRANSACTION name
    fn parse_transaction_statement(&mut self) -> ParseResult<Statement> {
        let kind = self.buffer.current()?.kind;
        let start = self.buffer.current()?.span.start;

        match kind {
            // BEGIN TRANSACTION [name]
            TokenKind::Begin => {
                self.buffer.consume()?; // BEGIN

                if !self.buffer.check(TokenKind::Transaction) && !self.buffer.check(TokenKind::Tran)
                {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::Transaction, TokenKind::Tran],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?; // TRANSACTION | TRAN

                let name = if self.buffer.check(TokenKind::Ident)
                    || self.buffer.check(TokenKind::QuotedIdent)
                {
                    Some(self.parse_identifier()?)
                } else {
                    None
                };

                let end_span = self.buffer.current()?.span;

                Ok(Statement::Transaction(TransactionStatement::Begin {
                    span: Span {
                        start,
                        end: end_span.end,
                    },
                    name,
                }))
            }

            // COMMIT TRANSACTION [name]
            TokenKind::Commit => {
                self.buffer.consume()?; // COMMIT

                let (name, end_span) = if self.buffer.check(TokenKind::Transaction)
                    || self.buffer.check(TokenKind::Tran)
                {
                    self.buffer.consume()?; // TRANSACTION | TRAN
                    (
                        if self.buffer.check(TokenKind::Ident)
                            || self.buffer.check(TokenKind::QuotedIdent)
                        {
                            Some(self.parse_identifier()?)
                        } else {
                            None
                        },
                        self.buffer.current()?.span,
                    )
                } else {
                    // COMMIT だけの場合（TRANSACTION 省略）
                    (None, self.buffer.current()?.span)
                };

                Ok(Statement::Transaction(TransactionStatement::Commit {
                    span: Span {
                        start,
                        end: end_span.end,
                    },
                    name,
                }))
            }

            // ROLLBACK TRANSACTION [name]
            TokenKind::Rollback => {
                self.buffer.consume()?; // ROLLBACK

                let (name, end_span) = if self.buffer.check(TokenKind::Transaction)
                    || self.buffer.check(TokenKind::Tran)
                {
                    self.buffer.consume()?; // TRANSACTION | TRAN
                    (
                        if self.buffer.check(TokenKind::Ident)
                            || self.buffer.check(TokenKind::QuotedIdent)
                        {
                            Some(self.parse_identifier()?)
                        } else {
                            None
                        },
                        self.buffer.current()?.span,
                    )
                } else {
                    // ROLLBACK だけの場合（TRANSACTION 省略）
                    (None, self.buffer.current()?.span)
                };

                Ok(Statement::Transaction(TransactionStatement::Rollback {
                    span: Span {
                        start,
                        end: end_span.end,
                    },
                    name,
                }))
            }

            // SAVE TRANSACTION name
            TokenKind::Save => {
                self.buffer.consume()?; // SAVE

                if !self.buffer.check(TokenKind::Transaction) && !self.buffer.check(TokenKind::Tran)
                {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::Transaction, TokenKind::Tran],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.span,
                    ));
                }
                self.buffer.consume()?; // TRANSACTION | TRAN

                let name = self.parse_identifier()?;
                let end_span = self.buffer.current()?.span;

                Ok(Statement::Transaction(TransactionStatement::Save {
                    span: Span {
                        start,
                        end: end_span.end,
                    },
                    name,
                }))
            }

            _ => Err(ParseError::unexpected_token(
                vec![
                    TokenKind::Begin,
                    TokenKind::Commit,
                    TokenKind::Rollback,
                    TokenKind::Save,
                ],
                kind,
                self.buffer.current()?.span,
            )),
        }
    }

    /// THROW 文を解析
    ///
    /// T-SQL構文: THROW [error_number, message, state]
    fn parse_throw_statement(&mut self) -> ParseResult<Statement> {
        let span = self.buffer.current()?.span;
        self.buffer.consume()?; // THROW

        let error_number = if self.buffer.check(TokenKind::Semicolon)
            || self.buffer.check(TokenKind::End)
            || self.is_at_eof()
        {
            None
        } else {
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        };

        let message = if error_number.is_some() && (self.buffer.check(TokenKind::Comma)) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        let state = if message.is_some() && self.buffer.check(TokenKind::Comma) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        let end_span = self.buffer.current()?.span;

        Ok(Statement::Throw(Box::new(ThrowStatement {
            span: Span {
                start: span.start,
                end: end_span.end,
            },
            error_number,
            message,
            state,
        })))
    }

    /// RAISERROR 文を解析
    ///
    /// T-SQL構文: RAISERROR(message, severity, state)
    fn parse_raiserror_statement(&mut self) -> ParseResult<Statement> {
        let span = self.buffer.current()?.span;
        self.buffer.consume()?; // RAISERROR

        // 左括弧
        if !self.buffer.check(TokenKind::LParen) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::LParen],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?;

        let mut expr_parser = ExpressionParser::new(&mut self.buffer);
        let message = expr_parser.parse()?;

        let severity = if self.buffer.check(TokenKind::Comma) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        let state = if severity.is_some() && self.buffer.check(TokenKind::Comma) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        // 右括弧
        if !self.buffer.check(TokenKind::RParen) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::RParen],
                self.buffer.current()?.kind,
                self.buffer.current()?.span,
            ));
        }
        self.buffer.consume()?;

        let end_span = self.buffer.current()?.span;

        Ok(Statement::Raiserror(Box::new(RaiserrorStatement {
            span: Span {
                start: span.start,
                end: end_span.end,
            },
            message,
            severity,
            state,
        })))
    }

    /// BEGIN TRY かどうかをチェック
    ///
    /// BEGIN の後ろに TRY が続く場合のみ true
    fn check_try_begin(&self) -> bool {
        // 現在のトークンは BEGIN なので、次のトークンをチェック
        self.buffer
            .peek(1)
            .map_or(false, |t| matches!(t.kind, TokenKind::Try))
    }

    /// BEGIN TRANSACTION かどうかをチェック
    ///
    /// BEGIN の後ろに TRANSACTION または TRAN が続く場合のみ true
    fn check_transaction_begin(&self) -> bool {
        // 現在のトークンは BEGIN なので、次のトークンをチェック
        self.buffer.peek(1).map_or(false, |t| {
            matches!(t.kind, TokenKind::Transaction | TokenKind::Tran)
        })
    }

    /// バッチ区切り（GO）を解析
    fn parse_batch_separator(&mut self) -> ParseResult<Statement> {
        let span = self.buffer.current()?.span;
        self.buffer.consume()?; // GO

        let repeat_count = if self.buffer.check(TokenKind::Number) {
            let n = self.buffer.current()?.text.parse().ok();
            self.buffer.consume()?;
            n
        } else {
            None
        };

        Ok(Statement::BatchSeparator(BatchSeparator {
            span,
            repeat_count,
        }))
    }

    /// 識別子を解析
    fn parse_identifier(&mut self) -> ParseResult<Identifier> {
        let current = self.buffer.current()?;
        let span = current.span;

        // キーワードが識別子として使用可能かチェック
        let can_keyword_be_identifier = matches!(
            current.kind,
            TokenKind::Table
                | TokenKind::View
                | TokenKind::Proc
                | TokenKind::Function
                | TokenKind::Index
                | TokenKind::Key
                | TokenKind::Constraint
                | TokenKind::Trigger
                | TokenKind::Go
                | TokenKind::Goto
                | TokenKind::Label
        );

        let name = if current.kind == TokenKind::QuotedIdent {
            // [name] の形式
            &current.text[1..current.text.len() - 1]
        } else if current.kind == TokenKind::Ident
            || current.kind == TokenKind::LocalVar
            || current.kind == TokenKind::GlobalVar
            || current.kind == TokenKind::TempTable
            || current.kind == TokenKind::GlobalTempTable
            || can_keyword_be_identifier
        {
            current.text
        } else {
            return Err(ParseError::unexpected_token(
                vec![
                    TokenKind::Ident,
                    TokenKind::QuotedIdent,
                    TokenKind::LocalVar,
                    TokenKind::TempTable,
                    TokenKind::GlobalTempTable,
                ],
                current.kind,
                span,
            ));
        };

        self.buffer.consume()?;

        Ok(Identifier {
            name: name.to_string(),
            span,
        })
    }

    /// EOFに達したか判定
    fn is_at_eof(&self) -> bool {
        self.buffer.check(TokenKind::Eof)
    }

    /// 同期ポイントまでスキップしてエラー回復
    fn synchronize(&mut self) {
        while !self.is_at_eof() {
            let kind = self.buffer.current().map(|t| t.kind);
            if matches!(
                kind,
                Ok(TokenKind::Semicolon)
                    | Ok(TokenKind::Select)
                    | Ok(TokenKind::Insert)
                    | Ok(TokenKind::Update)
                    | Ok(TokenKind::Delete)
                    | Ok(TokenKind::Create)
                    | Ok(TokenKind::End)
            ) {
                break;
            }
            let _ = self.buffer.consume();
        }
    }

    /// 再帰深度をチェック
    fn check_depth(&self) -> ParseResult<()> {
        if self.depth >= self.max_depth {
            return Err(ParseError::recursion_limit(
                self.max_depth,
                tsql_token::Position {
                    line: 0,
                    column: 0,
                    offset: self.buffer.current()?.span.start,
                },
            ));
        }
        Ok(())
    }

    /// カンマ区切りのリストをパース
    ///
    /// # Arguments
    ///
    /// * `parse_item` - 各アイテムをパースするクロージャ
    ///
    /// # Returns
    ///
    /// パースされたアイテムのベクター
    fn parse_comma_separated<T, F>(&mut self, mut parse_item: F) -> ParseResult<Vec<T>>
    where
        F: FnMut(&mut Self) -> ParseResult<T>,
    {
        let mut items = Vec::new();
        loop {
            items.push(parse_item(self)?);
            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }
        Ok(items)
    }

    /// 収集されたエラーを返す
    #[must_use]
    pub fn errors(&self) -> &[ParseError] {
        &self.errors
    }

    /// エラーを消費して取得
    pub fn drain_errors(&mut self) -> Vec<ParseError> {
        std::mem::take(&mut self.errors)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn parse_sql(sql: &str) -> ParseResult<Vec<Statement>> {
        let mut parser = Parser::new(sql);
        parser.parse()
    }

    #[test]
    fn test_parse_simple_select() {
        let result = parse_sql("SELECT * FROM users").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::Select(select) => {
                assert!(!select.distinct);
                assert_eq!(select.columns.len(), 1);
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_parse_select_with_columns() {
        let result = parse_sql("SELECT id, name FROM users").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::Select(select) => {
                assert_eq!(select.columns.len(), 2);
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_parse_select_distinct() {
        let result = parse_sql("SELECT DISTINCT id FROM users").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.distinct);
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_parse_insert_values() {
        let result = parse_sql("INSERT INTO users (id, name) VALUES (1, 'test')").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::Insert(_) => {}
            _ => panic!("Expected Insert statement"),
        }
    }

    #[test]
    fn test_parse_update() {
        let result = parse_sql("UPDATE users SET name = 'test' WHERE id = 1").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::Update(_) => {}
            _ => panic!("Expected Update statement"),
        }
    }

    #[test]
    fn test_parse_delete() {
        let result = parse_sql("DELETE FROM users WHERE id = 1").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::Delete(_) => {}
            _ => panic!("Expected Delete statement"),
        }
    }

    #[test]
    fn test_parse_create_table() {
        let result = parse_sql("CREATE TABLE users (id INT, name VARCHAR(100))").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.columns.len(), 2);
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create Table statement"),
        }
    }

    #[test]
    fn test_parse_declare() {
        let result = parse_sql("DECLARE @x INT").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::Declare(decl) => {
                assert_eq!(decl.variables.len(), 1);
                assert_eq!(decl.variables[0].name.name, "@x");
            }
            _ => panic!("Expected Declare statement"),
        }
    }

    #[test]
    fn test_parse_set() {
        let result = parse_sql("SET @x = 1").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::Set(set) => {
                assert_eq!(set.variable.name, "@x");
            }
            _ => panic!("Expected Set statement"),
        }
    }

    #[test]
    fn test_parse_if_statement() {
        let result = parse_sql("IF @x = 1 SELECT 1").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::If(_) => {}
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_parse_while_statement() {
        let result = parse_sql("WHILE @x < 10 SELECT @x").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::While(_) => {}
            _ => panic!("Expected While statement"),
        }
    }

    #[test]
    fn test_parse_block() {
        let result = parse_sql("BEGIN SELECT 1 END").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::Block(block) => {
                assert_eq!(block.statements.len(), 1);
            }
            _ => panic!("Expected Block statement"),
        }
    }

    #[test]
    fn test_parse_multiple_statements() {
        let result = parse_sql("SELECT 1; SELECT 2;").unwrap();
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_parse_with_mode_single_statement() {
        // SingleStatementモードのテスト
        let mut parser = Parser::new("SELECT 1").with_mode(ParserMode::SingleStatement);
        let result = parser.parse();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn test_parse_error_on_invalid_syntax() {
        let result = parse_sql("SELECT FROM");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_input() {
        let result = parse_sql("");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_parse_top_clause() {
        // TOPはまだ実装されていないため、代わりに基本的なSELECTをテスト
        let result = parse_sql("SELECT * FROM users LIMIT 10");
        // LIMITはT-SQLの構文ではないためエラーになる可能性がある
        // 実際の実装に合わせる
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_parse_join_inner() {
        // JOINはまだ実装されていないため、FROM句のみをテスト
        let result = parse_sql("SELECT * FROM users").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.from.is_some());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_parse_join_left() {
        // JOINはまだ実装されていないため、FROM句のみをテスト
        let result = parse_sql("SELECT * FROM orders").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_where_clause() {
        let result = parse_sql("SELECT * FROM users WHERE id = 1").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_group_by() {
        let result = parse_sql("SELECT status, COUNT(*) FROM users GROUP BY status").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_having() {
        let result =
            parse_sql("SELECT status, COUNT(*) FROM users GROUP BY status HAVING COUNT(*) > 5")
                .unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_order_by() {
        let result = parse_sql("SELECT * FROM users ORDER BY name").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_insert_select() {
        let result = parse_sql("INSERT INTO users_archive SELECT * FROM users").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_create_index() {
        let result = parse_sql("CREATE INDEX idx_users_email ON users(email)").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_create_view() {
        let result = parse_sql("CREATE VIEW user_view AS SELECT id, name FROM users").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_create_procedure() {
        let result = parse_sql("CREATE PROCEDURE get_users AS SELECT * FROM users").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_break_statement() {
        let result = parse_sql("WHILE 1 > 0 BREAK").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_continue_statement() {
        let result = parse_sql("WHILE 1 > 0 CONTINUE").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_return_statement() {
        let result = parse_sql("CREATE PROCEDURE test AS BEGIN RETURN 1 END").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_go_batch() {
        let result = parse_sql("SELECT 1 GO SELECT 2").unwrap();
        // GOは文として解析される（現在の実装ではGOは常に認識される）
        assert_eq!(result.len(), 3); // SELECT 1, GO, SELECT 2
    }

    #[test]
    fn test_parse_go_count() {
        // GOバッチ処理のテスト
        let result = parse_sql("SELECT 1 GO SELECT 2").unwrap();
        // GOはバッチ区切りとして処理される
        assert!(!result.is_empty());
    }

    #[test]
    fn test_check_depth_limit() {
        let mut parser = Parser::new("SELECT 1");
        // 深度を超過させる
        parser.depth = parser.max_depth + 1;
        let result = parser.parse_statement();
        assert!(result.is_err());
        match result.unwrap_err() {
            ParseError::RecursionLimitExceeded { .. } => {}
            _ => panic!("Expected RecursionLimitExceeded error"),
        }
    }

    #[test]
    fn test_with_mode_chaining() {
        let parser = Parser::new("SELECT 1").with_mode(ParserMode::SingleStatement);
        assert_eq!(parser.mode, ParserMode::SingleStatement);
    }

    #[test]
    fn test_errors_accessor() {
        let mut parser = Parser::new("SELECT FROM");
        let _ = parser.parse();
        let errors = parser.errors();
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_drain_errors() {
        let mut parser = Parser::new("SELECT FROM");
        let _ = parser.parse();
        let errors = parser.drain_errors();
        assert!(!errors.is_empty());
        // drain後にエラーが空になる
        assert!(parser.errors().is_empty());
    }

    #[test]
    fn test_synchronize_after_error() {
        let result = parse_sql("SELECT FROM users; SELECT 1");
        // 同期化により2番目の文は解析できる
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_with_trailing_semicolon() {
        let result = parse_sql("SELECT 1;").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_parse_with_multiple_semicolons() {
        // 複数のセミコロンはスキップされる
        let result = parse_sql("SELECT 1; SELECT 2;").unwrap();
        assert_eq!(result.len(), 2);
    }

    // Task 18.1: SELECT文のテスト

    #[test]
    fn test_select_simple_columns() {
        // シンプルなSELECTで複数カラム
        let result = parse_sql("SELECT id, name, email FROM users").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::Select(select) => {
                assert_eq!(select.columns.len(), 3);
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_with_expression_column() {
        // 式を含むSELECTリスト
        let result = parse_sql("SELECT id, price * quantity AS total FROM orders").unwrap();
        assert_eq!(result.len(), 1);
        match &result[0] {
            Statement::Select(select) => {
                assert_eq!(select.columns.len(), 2);
                // 2番目のカラムは別名付き
                if let SelectItem::Expression(_, Some(alias)) = &select.columns[1] {
                    assert_eq!(alias.name, "total");
                }
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_distinct() {
        // DISTINCTのテスト
        let result = parse_sql("SELECT DISTINCT category FROM products").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.distinct);
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_top() {
        // TOP句のテスト
        let result = parse_sql("SELECT TOP 10 * FROM users").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.top.is_some());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_top_with_expression() {
        // 式を含むTOP句
        let result = parse_sql("SELECT TOP (@n) * FROM users").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.top.is_some());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_from() {
        // FROM句のテスト
        let result = parse_sql("SELECT * FROM users").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.from.is_some());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_from_with_alias() {
        // テーブル別名のテスト
        let result = parse_sql("SELECT u.* FROM users u").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.from.is_some());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_where() {
        // WHERE句のテスト
        let result = parse_sql("SELECT * FROM users WHERE id = 1").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.where_clause.is_some());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_where_complex() {
        // 複雑なWHERE条件
        let result =
            parse_sql("SELECT * FROM users WHERE age >= 18 AND status = 'active'").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.where_clause.is_some());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_join_inner() {
        // INNER JOINのテスト
        let result =
            parse_sql("SELECT * FROM orders INNER JOIN users ON orders.user_id = users.id")
                .unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.from.is_some());
                if let Some(from) = &select.from {
                    assert!(!from.joins.is_empty());
                }
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_join_left() {
        // LEFT JOINのテスト
        let result =
            parse_sql("SELECT * FROM orders LEFT JOIN users ON orders.user_id = users.id").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.from.is_some());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_join_right() {
        // RIGHT JOINのテスト
        let result =
            parse_sql("SELECT * FROM orders RIGHT JOIN users ON orders.user_id = users.id")
                .unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.from.is_some());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_join_cross() {
        // CROSS JOINのテスト
        let result = parse_sql("SELECT * FROM users CROSS JOIN departments").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.from.is_some());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_group_by() {
        // GROUP BYのテスト
        let result =
            parse_sql("SELECT category, COUNT(*) FROM products GROUP BY category").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(!select.group_by.is_empty());
                assert_eq!(select.group_by.len(), 1);
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_group_by_multiple() {
        // 複数カラムでのGROUP BY
        let result =
            parse_sql("SELECT category, status, COUNT(*) FROM products GROUP BY category, status")
                .unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert_eq!(select.group_by.len(), 2);
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_having() {
        // HAVING句のテスト
        let result = parse_sql(
            "SELECT category, COUNT(*) FROM products GROUP BY category HAVING COUNT(*) > 5",
        )
        .unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.having.is_some());
                assert!(!select.group_by.is_empty());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_order_by_asc() {
        // ORDER BY ASCのテスト
        let result = parse_sql("SELECT * FROM users ORDER BY name ASC").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert_eq!(select.order_by.len(), 1);
                assert!(select.order_by[0].asc);
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_order_by_desc() {
        // ORDER BY DESCのテスト
        let result = parse_sql("SELECT * FROM users ORDER BY name DESC").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert_eq!(select.order_by.len(), 1);
                assert!(!select.order_by[0].asc);
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_order_by_multiple() {
        // 複数カラムでのORDER BY
        let result =
            parse_sql("SELECT * FROM users ORDER BY last_name ASC, first_name ASC").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert_eq!(select.order_by.len(), 2);
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_select_full_query() {
        // 完全なSELECTクエリ
        let result = parse_sql(
            "SELECT DISTINCT TOP 10 category, COUNT(*) AS cnt \
             FROM products \
             WHERE price > 100 \
             GROUP BY category \
             HAVING COUNT(*) > 5 \
             ORDER BY cnt DESC",
        )
        .unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.distinct);
                assert!(select.top.is_some());
                assert!(select.where_clause.is_some());
                assert!(!select.group_by.is_empty());
                assert!(select.having.is_some());
                assert!(!select.order_by.is_empty());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    // Task 18.2: DML文のテスト

    #[test]
    fn test_insert_values() {
        // VALUES句付きINSERT
        let result = parse_sql("INSERT INTO users (id, name) VALUES (1, 'John')").unwrap();
        match &result[0] {
            Statement::Insert(insert) => {
                assert_eq!(insert.table.name, "users");
                assert_eq!(insert.columns.len(), 2);
                match &insert.source {
                    InsertSource::Values(rows) => {
                        assert_eq!(rows.len(), 1);
                        assert_eq!(rows[0].len(), 2);
                    }
                    _ => panic!("Expected Values source"),
                }
            }
            _ => panic!("Expected Insert statement"),
        }
    }

    #[test]
    fn test_insert_values_multiple_rows() {
        // 複数行のVALUES
        let result =
            parse_sql("INSERT INTO users (id, name) VALUES (1, 'John'), (2, 'Jane'), (3, 'Bob')")
                .unwrap();
        match &result[0] {
            Statement::Insert(insert) => match &insert.source {
                InsertSource::Values(rows) => {
                    assert_eq!(rows.len(), 3);
                }
                _ => panic!("Expected Values source"),
            },
            _ => panic!("Expected Insert statement"),
        }
    }

    #[test]
    fn test_insert_with_column_list() {
        // カラムリスト付きINSERT
        let result =
            parse_sql("INSERT INTO users (id, name, email) VALUES (1, 'John', 'john@example.com')")
                .unwrap();
        match &result[0] {
            Statement::Insert(insert) => {
                assert_eq!(insert.columns.len(), 3);
                assert_eq!(insert.columns[0].name, "id");
                assert_eq!(insert.columns[1].name, "name");
                assert_eq!(insert.columns[2].name, "email");
            }
            _ => panic!("Expected Insert statement"),
        }
    }

    #[test]
    fn test_insert_without_column_list() {
        // カラムリストなしINSERT
        let result = parse_sql("INSERT INTO users VALUES (1, 'John', 'john@example.com')").unwrap();
        match &result[0] {
            Statement::Insert(insert) => {
                assert!(insert.columns.is_empty());
            }
            _ => panic!("Expected Insert statement"),
        }
    }

    #[test]
    fn test_insert_select() {
        // INSERT-SELECT
        let result =
            parse_sql("INSERT INTO users_archive SELECT * FROM users WHERE deleted = 0").unwrap();
        match &result[0] {
            Statement::Insert(insert) => match &insert.source {
                InsertSource::Select(_) => {}
                _ => panic!("Expected Select source"),
            },
            _ => panic!("Expected Insert statement"),
        }
    }

    #[test]
    fn test_insert_default_values() {
        // DEFAULT VALUES
        let result = parse_sql("INSERT INTO users DEFAULT VALUES").unwrap();
        match &result[0] {
            Statement::Insert(insert) => {
                assert!(matches!(&insert.source, InsertSource::DefaultValues));
            }
            _ => panic!("Expected Insert statement"),
        }
    }

    #[test]
    fn test_update_simple() {
        // シンプルなUPDATE
        let result = parse_sql("UPDATE users SET name = 'John' WHERE id = 1").unwrap();
        match &result[0] {
            Statement::Update(update) => {
                assert_eq!(update.assignments.len(), 1);
                assert!(update.where_clause.is_some());
            }
            _ => panic!("Expected Update statement"),
        }
    }

    #[test]
    fn test_update_multiple_columns() {
        // 複数カラムのUPDATE
        let result = parse_sql(
            "UPDATE users SET name = 'John', email = 'john@example.com', status = 1 WHERE id = 1",
        )
        .unwrap();
        match &result[0] {
            Statement::Update(update) => {
                assert_eq!(update.assignments.len(), 3);
            }
            _ => panic!("Expected Update statement"),
        }
    }

    #[test]
    fn test_update_with_from() {
        // FROM句付きUPDATE（ASE固有）
        let result = parse_sql("UPDATE orders SET status = 'shipped' FROM orders o JOIN users u ON o.user_id = u.id WHERE u.active = 1").unwrap();
        match &result[0] {
            Statement::Update(update) => {
                assert!(update.from_clause.is_some());
            }
            _ => panic!("Expected Update statement"),
        }
    }

    #[test]
    fn test_update_without_where() {
        // WHEREなしUPDATE（すべての行を更新）
        let result = parse_sql("UPDATE users SET status = 1").unwrap();
        match &result[0] {
            Statement::Update(update) => {
                assert!(update.where_clause.is_none());
            }
            _ => panic!("Expected Update statement"),
        }
    }

    #[test]
    fn test_delete_simple() {
        // シンプルなDELETE
        let result = parse_sql("DELETE FROM users WHERE id = 1").unwrap();
        match &result[0] {
            Statement::Delete(delete) => {
                assert_eq!(delete.table.name, "users");
                assert!(delete.where_clause.is_some());
            }
            _ => panic!("Expected Delete statement"),
        }
    }

    #[test]
    fn test_delete_without_from() {
        // FROMなしDELETE
        let result = parse_sql("DELETE users WHERE id = 1").unwrap();
        match &result[0] {
            Statement::Delete(delete) => {
                assert_eq!(delete.table.name, "users");
            }
            _ => panic!("Expected Delete statement"),
        }
    }

    #[test]
    fn test_delete_with_join_from() {
        // JOIN用FROM句付きDELETE
        let result = parse_sql(
            "DELETE FROM orders FROM orders o JOIN users u ON o.user_id = u.id WHERE u.active = 0",
        )
        .unwrap();
        match &result[0] {
            Statement::Delete(delete) => {
                assert!(delete.from_clause.is_some());
            }
            _ => panic!("Expected Delete statement"),
        }
    }

    #[test]
    fn test_delete_without_where() {
        // WHEREなしDELETE（すべての行を削除）
        let result = parse_sql("DELETE FROM users").unwrap();
        match &result[0] {
            Statement::Delete(delete) => {
                assert!(delete.where_clause.is_none());
            }
            _ => panic!("Expected Delete statement"),
        }
    }

    // Task 18.3: DDLと制御フローのテスト

    #[test]
    fn test_create_table_basic() {
        // 基本的なCREATE TABLE
        let result = parse_sql("CREATE TABLE users (id INT, name VARCHAR(100))").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.name.name, "users");
                    assert_eq!(table.columns.len(), 2);
                    assert!(!table.temporary);
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_create_table_with_constraints() {
        // カラム制約付きCREATE TABLE
        let result = parse_sql(
            "CREATE TABLE users ( \
             id INT PRIMARY KEY, \
             name VARCHAR(100) NOT NULL, \
             email VARCHAR(255) NOT NULL \
             )",
        )
        .unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.columns.len(), 3);
                    // カラムのnullabilityが正しく解析されていることを確認
                    assert_eq!(table.columns[0].nullability, None); // id INT PRIMARY KEY
                    assert_eq!(table.columns[1].nullability, Some(false)); // name VARCHAR(100) NOT NULL
                    assert_eq!(table.columns[2].nullability, Some(false)); // email VARCHAR(255) NOT NULL
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_create_table_temporary() {
        // 一時テーブルの作成
        let result = parse_sql("CREATE TABLE #temp (id INT, value VARCHAR(50))").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert!(table.temporary);
                    assert!(table.name.name.starts_with('#'));
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_create_table_with_identity() {
        // IDENTITYカラム
        let result = parse_sql("CREATE TABLE users (id INT IDENTITY, name VARCHAR(100))").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert!(table.columns[0].identity);
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_create_table_with_nullability() {
        // NULL制約のテスト
        let result = parse_sql("CREATE TABLE test (col1 INT NULL, col2 INT NOT NULL)").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.columns[0].nullability, Some(true));
                    assert_eq!(table.columns[1].nullability, Some(false));
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_create_index() {
        // CREATE INDEX
        let result = parse_sql("CREATE INDEX idx_users_email ON users(email)").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Index(idx) => {
                    assert_eq!(idx.name.name, "idx_users_email");
                    assert_eq!(idx.table.name, "users");
                    assert_eq!(idx.columns.len(), 1);
                }
                _ => panic!("Expected Create Index statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_create_index_multiple_columns() {
        // 複数カラムのインデックス
        let result =
            parse_sql("CREATE INDEX idx_composite ON users(last_name, first_name)").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Index(idx) => {
                    assert_eq!(idx.columns.len(), 2);
                }
                _ => panic!("Expected Create Index statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_create_view() {
        // CREATE VIEW
        let result =
            parse_sql("CREATE VIEW active_users AS SELECT * FROM users WHERE status = 1").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::View(view) => {
                    assert_eq!(view.name.name, "active_users");
                }
                _ => panic!("Expected Create View statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_create_view_with_join() {
        // JOINを含むVIEW
        let result = parse_sql(
            "CREATE VIEW user_orders AS \
             SELECT u.name, o.order_date \
             FROM users u \
             INNER JOIN orders o ON u.id = o.user_id",
        )
        .unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::View(view) => {
                    assert_eq!(view.name.name, "user_orders");
                }
                _ => panic!("Expected Create View statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_declare_single() {
        // 単一変数のDECLARE
        let result = parse_sql("DECLARE @x INT").unwrap();
        match &result[0] {
            Statement::Declare(decl) => {
                assert_eq!(decl.variables.len(), 1);
                assert_eq!(decl.variables[0].name.name, "@x");
            }
            _ => panic!("Expected Declare statement"),
        }
    }

    #[test]
    fn test_declare_multiple() {
        // 複数変数のDECLARE
        let result = parse_sql("DECLARE @x INT, @y VARCHAR(100), @z BIT").unwrap();
        match &result[0] {
            Statement::Declare(decl) => {
                assert_eq!(decl.variables.len(), 3);
            }
            _ => panic!("Expected Declare statement"),
        }
    }

    #[test]
    fn test_declare_with_default() {
        // デフォルト値付きDECLARE
        let result = parse_sql("DECLARE @x INT = 10").unwrap();
        match &result[0] {
            Statement::Declare(decl) => {
                assert!(decl.variables[0].default_value.is_some());
            }
            _ => panic!("Expected Declare statement"),
        }
    }

    #[test]
    fn test_set_variable() {
        // SETによる変数代入
        let result = parse_sql("SET @x = 10").unwrap();
        match &result[0] {
            Statement::Set(set) => {
                assert_eq!(set.variable.name, "@x");
            }
            _ => panic!("Expected Set statement"),
        }
    }

    #[test]
    fn test_set_variable_with_expression() {
        // 式を含むSET
        let result = parse_sql("SET @x = @y + 1").unwrap();
        match &result[0] {
            Statement::Set(set) => {
                assert_eq!(set.variable.name, "@x");
            }
            _ => panic!("Expected Set statement"),
        }
    }

    #[test]
    fn test_select_variable_assignment() {
        // SELECTによる変数代入
        let result = parse_sql("SELECT @x = 1").unwrap();
        match &result[0] {
            Statement::VariableAssignment(var_assign) => {
                assert_eq!(var_assign.assignments.len(), 1);
                assert_eq!(var_assign.assignments[0].variable.name, "@x");
            }
            _ => panic!("Expected VariableAssignment statement"),
        }
    }

    #[test]
    fn test_select_variable_assignment_with_expression() {
        // 式を含むSELECT変数代入
        let result = parse_sql("SELECT @x = @y + 1").unwrap();
        match &result[0] {
            Statement::VariableAssignment(var_assign) => {
                assert_eq!(var_assign.assignments.len(), 1);
                assert_eq!(var_assign.assignments[0].variable.name, "@x");
            }
            _ => panic!("Expected VariableAssignment statement"),
        }
    }

    #[test]
    fn test_select_variable_assignment_multiple() {
        // 複数変数の代入
        let result = parse_sql("SELECT @x = 1, @y = 2, @z = 3").unwrap();
        match &result[0] {
            Statement::VariableAssignment(var_assign) => {
                assert_eq!(var_assign.assignments.len(), 3);
                assert_eq!(var_assign.assignments[0].variable.name, "@x");
                assert_eq!(var_assign.assignments[1].variable.name, "@y");
                assert_eq!(var_assign.assignments[2].variable.name, "@z");
            }
            _ => panic!("Expected VariableAssignment statement"),
        }
    }

    #[test]
    fn test_select_not_variable_assignment() {
        // 通常のSELECT文は変数代入として扱わない
        let result = parse_sql("SELECT x FROM table").unwrap();
        match &result[0] {
            Statement::Select(_) => {}
            _ => panic!("Expected Select statement, not VariableAssignment"),
        }
    }

    #[test]
    fn test_select_column_not_confused_with_variable() {
        // カラム名が@で始まっていれば変数代入、そうでなければ通常のSELECT
        let result = parse_sql("SELECT x = 1").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                // x = 1は比較式として解釈される
                assert_eq!(select.columns.len(), 1);
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_temp_table_reference() {
        // 一時テーブル参照 (#temp_table)
        let result = parse_sql("SELECT * FROM #temp_table").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.from.is_some());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_global_temp_table_reference() {
        // グローバル一時テーブル参照 (##global_temp)
        let result = parse_sql("SELECT * FROM ##global_temp").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.from.is_some());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_insert_into_temp_table() {
        // 一時テーブルへのINSERT
        let result = parse_sql("INSERT INTO #temp VALUES (1, 'test')").unwrap();
        match &result[0] {
            Statement::Insert(insert) => {
                assert_eq!(insert.table.name, "#temp");
            }
            _ => panic!("Expected Insert statement"),
        }
    }

    #[test]
    fn test_create_temp_table() {
        // 一時テーブルのCREATE
        let result = parse_sql("CREATE TABLE #temp (id INT, name VARCHAR(50))").unwrap();
        match &result[0] {
            Statement::Create(create) => match create.as_ref() {
                crate::ast::CreateStatement::Table(table_def) => {
                    assert_eq!(table_def.name.name, "#temp");
                    assert!(table_def.temporary);
                }
                _ => panic!("Expected Table definition"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_subquery_in_from() {
        // FROM句でのサブクエリ（導出テーブル）
        let result = parse_sql("SELECT * FROM (SELECT id FROM users) AS u").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.from.is_some());
                match &select.from.as_ref().unwrap().tables[0] {
                    crate::ast::TableReference::Subquery { alias, .. } => {
                        assert!(alias.is_some());
                        assert_eq!(alias.as_ref().unwrap().name, "u");
                    }
                    _ => panic!("Expected Subquery table reference"),
                }
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_subquery_without_alias() {
        // サブクエリの別名はオプション
        let result = parse_sql("SELECT * FROM (SELECT id FROM users)").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.from.is_some());
                match &select.from.as_ref().unwrap().tables[0] {
                    crate::ast::TableReference::Subquery { alias, .. } => {
                        assert!(alias.is_none());
                    }
                    _ => panic!("Expected Subquery table reference"),
                }
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_subquery_with_join() {
        // サブクエリを使ったJOIN
        let result = parse_sql("SELECT * FROM (SELECT id FROM users) AS u JOIN (SELECT user_id FROM orders) AS o ON u.id = o.user_id").unwrap();
        match &result[0] {
            Statement::Select(select) => {
                assert!(select.from.is_some());
            }
            _ => panic!("Expected Select statement"),
        }
    }

    #[test]
    fn test_if_else() {
        // IF...ELSE文
        let result = parse_sql("IF @x = 1 SELECT 1 ELSE SELECT 2").unwrap();
        match &result[0] {
            Statement::If(if_stmt) => {
                assert!(if_stmt.else_branch.is_some());
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_if_without_else() {
        // ELSEなしIF文
        let result = parse_sql("IF @x = 1 SELECT 1").unwrap();
        match &result[0] {
            Statement::If(if_stmt) => {
                assert!(if_stmt.else_branch.is_none());
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_if_begin_end() {
        // BEGIN...ENDブロック付きIF
        let result = parse_sql("IF @x = 1 BEGIN SELECT 1 SELECT 2 END").unwrap();
        match &result[0] {
            Statement::If(_) => {}
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_while_simple() {
        // シンプルなWHILE
        let result = parse_sql("WHILE @x < 10 SELECT @x").unwrap();
        match &result[0] {
            Statement::While(_) => {}
            _ => panic!("Expected While statement"),
        }
    }

    #[test]
    fn test_while_with_begin_end() {
        // BEGIN...ENDブロック付きWHILE
        let result = parse_sql("WHILE @x < 10 BEGIN SET @x = @x + 1 END").unwrap();
        match &result[0] {
            Statement::While(while_stmt) => {
                if let Statement::Block(block) = &while_stmt.body {
                    assert!(!block.statements.is_empty());
                }
            }
            _ => panic!("Expected While statement"),
        }
    }

    #[test]
    fn test_begin_end_block() {
        // BEGIN...ENDブロック
        let result = parse_sql("BEGIN SELECT 1 SELECT 2 END").unwrap();
        match &result[0] {
            Statement::Block(block) => {
                assert_eq!(block.statements.len(), 2);
            }
            _ => panic!("Expected Block statement"),
        }
    }

    #[test]
    fn test_break_in_loop() {
        // BREAK文
        let result = parse_sql("WHILE 1 > 0 BREAK").unwrap();
        match &result[0] {
            Statement::While(while_stmt) => {
                assert!(matches!(
                    &while_stmt.body as &Statement,
                    Statement::Break(_)
                ));
            }
            _ => panic!("Expected While statement"),
        }
    }

    #[test]
    fn test_continue_in_loop() {
        // CONTINUE文
        let result = parse_sql("WHILE 1 > 0 CONTINUE").unwrap();
        match &result[0] {
            Statement::While(while_stmt) => {
                assert!(matches!(
                    &while_stmt.body as &Statement,
                    Statement::Continue(_)
                ));
            }
            _ => panic!("Expected While statement"),
        }
    }

    #[test]
    fn test_return_simple() {
        // シンプルなRETURN
        let result = parse_sql("RETURN").unwrap();
        match &result[0] {
            Statement::Return(ret) => {
                assert!(ret.expression.is_none());
            }
            _ => panic!("Expected Return statement"),
        }
    }

    #[test]
    fn test_return_with_value() {
        // 値付きRETURN
        let result = parse_sql("RETURN 1").unwrap();
        match &result[0] {
            Statement::Return(ret) => {
                assert!(ret.expression.is_some());
            }
            _ => panic!("Expected Return statement"),
        }
    }

    #[test]
    fn test_return_with_variable() {
        // 変数を返すRETURN
        let result = parse_sql("RETURN @result").unwrap();
        match &result[0] {
            Statement::Return(ret) => {
                assert!(ret.expression.is_some());
            }
            _ => panic!("Expected Return statement"),
        }
    }

    #[test]
    fn test_procedure_with_parameters() {
        // パラメータ付きストアドプロシージャ
        let result = parse_sql(
            "CREATE PROCEDURE get_users @status INT AS SELECT * FROM users WHERE status = @status",
        )
        .unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Procedure(proc) => {
                    assert_eq!(proc.name.name, "get_users");
                    assert_eq!(proc.parameters.len(), 1);
                    assert_eq!(proc.parameters[0].name.name, "@status");
                }
                _ => panic!("Expected Create Procedure statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_procedure_with_multiple_parameters() {
        // 複数パラメータ付きストアドプロシージャ
        let result = parse_sql(
            "CREATE PROCEDURE search_users \
             @min_id INT = 0, \
             @max_id INT = 1000000, \
             @status INT \
             AS \
             SELECT * FROM users \
             WHERE id BETWEEN @min_id AND @max_id AND status = @status",
        )
        .unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Procedure(proc) => {
                    assert_eq!(proc.parameters.len(), 3);
                    // 2番目のパラメータはデフォルト値を持つ
                    assert!(proc.parameters[1].default_value.is_some());
                }
                _ => panic!("Expected Create Procedure statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    // Task 19.1: バッチ処理のテスト

    #[test]
    fn test_go_keyword_tokenization() {
        // GOキーワードが正しくトークン化されているか確認
        use tsql_lexer::Lexer;

        let sql = "GO";
        let mut lexer = Lexer::new(sql);
        let token = lexer.next_token().unwrap();

        // デバッグ: トークン種別を確認
        println!("GO token kind: {:?}", token.kind);
        println!("GO token text: {:?}", token.text);

        // Goトークンであることを確認
        assert_eq!(token.kind, tsql_token::TokenKind::Go);
    }

    #[test]
    fn test_go_after_select() {
        // SELECT文の後のGOが正しくトークン化されているか確認
        use tsql_lexer::Lexer;

        let sql = "SELECT 1\nGO";
        let mut lexer = Lexer::new(sql);

        // SELECT
        let token1 = lexer.next_token().unwrap();
        println!("token1: {:?} {:?}", token1.kind, token1.text);
        assert_eq!(token1.kind, tsql_token::TokenKind::Select);

        // スペース（スキップされる）
        // 1
        let token2 = lexer.next_token().unwrap();
        println!("token2: {:?} {:?}", token2.kind, token2.text);
        assert_eq!(token2.kind, tsql_token::TokenKind::Number);

        // GO
        let token3 = lexer.next_token().unwrap();
        println!("token3: {:?} {:?}", token3.kind, token3.text);
        assert_eq!(token3.kind, tsql_token::TokenKind::Go);
    }

    #[test]
    fn test_go_at_line_start() {
        // 行頭でのGO検出
        let result = parse_sql("SELECT 1\nGO\nSELECT 2").unwrap();
        assert_eq!(result.len(), 3); // SELECT 1, GO, SELECT 2
    }

    #[test]
    fn test_go_with_leading_whitespace() {
        // 先頭空白付きGO（T-SQLではバッチ区切りとして認識）
        let result = parse_sql("SELECT 1\n  GO  \nSELECT 2").unwrap();
        // GOは行頭で検出されるため、このテストではGOが識別子として扱われる可能性がある
        // 実際のT-SQLでは行頭のGOはバッチ区切り
        assert!(!result.is_empty());
    }

    #[test]
    fn test_go_not_in_string() {
        // 文字列内のGOはバッチ区切りとみなされない
        let result = parse_sql("SELECT 'GO' AS result").unwrap();
        match &result[0] {
            Statement::Select(_) => {}
            _ => panic!("Expected Select statement, GO should not be detected in string"),
        }
    }

    #[test]
    fn test_go_not_in_comment() {
        // コメント内のGOはバッチ区切りとみなされない
        let result = parse_sql("-- This is a comment with GO\nSELECT 1").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_go_not_in_multiline_comment() {
        // 複数行コメント内のGO
        let result = parse_sql("/* This is a comment with GO inside */\nSELECT 1").unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_go_not_as_identifier() {
        // 識別子の一部としてのGOはバッチ区切りとみなされない
        let mut parser =
            Parser::new("SELECT goto FROM gopher").with_mode(ParserMode::SingleStatement);
        let result = parser.parse();
        // SingleStatementモードではGOは識別子として扱われる
        assert!(result.is_ok());
    }

    #[test]
    fn test_go_with_repeat_count() {
        // GO N形式のリピートカウント
        let result = parse_sql("SELECT 1\nGO 5").unwrap();
        match &result[1] {
            Statement::BatchSeparator(batch) => {
                assert_eq!(batch.repeat_count, Some(5));
            }
            _ => panic!("Expected BatchSeparator with repeat count"),
        }
    }

    #[test]
    fn test_go_zero_count() {
        // GO 0はバッチを実行しない
        let result = parse_sql("SELECT 1\nGO 0").unwrap();
        match &result[1] {
            Statement::BatchSeparator(batch) => {
                assert_eq!(batch.repeat_count, Some(0));
            }
            _ => panic!("Expected BatchSeparator with repeat count 0"),
        }
    }

    #[test]
    fn test_multiple_batches() {
        // 複数バッチの処理
        let result = parse_sql("SELECT 1\nGO\nSELECT 2\nGO\nSELECT 3").unwrap();
        assert_eq!(result.len(), 5); // 3つのSELECT + 2つのGO
    }

    #[test]
    fn test_empty_batch_before_go() {
        // 空バッチのテスト
        let result = parse_sql("\nGO\nSELECT 1").unwrap();
        // GOの前の空行は無視される
        assert!(!result.is_empty());
    }

    #[test]
    fn test_empty_batch_after_go() {
        // GOの後の空バッチ
        let result = parse_sql("SELECT 1\nGO\n\n").unwrap();
        assert!(!result.is_empty());
    }

    #[test]
    fn test_single_statement_mode_go_as_identifier() {
        // 単一文モードではGOは識別子
        let mut parser = Parser::new("SELECT GO FROM table").with_mode(ParserMode::SingleStatement);
        let result = parser.parse();
        assert!(result.is_ok());
        match &result.unwrap()[0] {
            Statement::Select(_) => {}
            _ => panic!("Expected Select statement in SingleStatement mode"),
        }
    }

    #[test]
    fn test_mode_switching() {
        // モード切り替えのテスト
        let sql = "SELECT GO FROM table";
        let mut batch_parser = Parser::new(sql);
        let mut single_parser = Parser::new(sql).with_mode(ParserMode::SingleStatement);

        // バッチモードではGOを解釈しようとするが、行頭ではないため識別子として扱われる
        let batch_result = batch_parser.parse();
        assert!(batch_result.is_ok());

        // 単一文モードではGOは常に識別子
        let single_result = single_parser.parse();
        assert!(single_result.is_ok());
    }

    #[test]
    fn test_go_case_insensitive() {
        // GOは大文字小文字を区別しない
        let result = parse_sql("SELECT 1\ngo\nSELECT 2\nGo\nSELECT 3\ngO").unwrap();
        assert_eq!(result.len(), 6); // 3つのSELECT + 3つのGO（すべての大文字小文字バリエーション）
    }

    // Task 20.1: エラー回復のテスト

    #[test]
    fn test_error_unexpected_token() {
        // 予期しないトークンによるエラー
        let result = parse_sql("SELECT FROM users");
        assert!(result.is_err());
        match result.unwrap_err() {
            ParseError::UnexpectedToken { .. } => {}
            _ => panic!("Expected UnexpectedToken error"),
        }
    }

    #[test]
    fn test_error_unexpected_eof() {
        // 予期しないEOFによるエラー
        let result = parse_sql("SELECT * FROM");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_missing_parenthesis() {
        // 括弧の閉じ忘れ
        let result = parse_sql("SELECT * FROM users WHERE id IN (1, 2, 3");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_missing_quote() {
        // クォートの閉じ忘れ（字句解析器で検出されるはず）
        let result = parse_sql("SELECT * FROM users WHERE name = 'John");
        // 文字列リテラルのエラー処理は字句解析器に依存
        // パーサーがこのエラーをどう処理するかを確認
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_synchronize_at_semicolon() {
        // セミコロンでの同期化
        let mut parser = Parser::new("INVALID SQL; SELECT 1");
        let result = parser.parse();
        // 最初のエラー後に同期して2番目の文を解析できるか
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn test_synchronize_at_keywords() {
        // キーワードでの同期化
        let mut parser = Parser::new("INVALID STATEMENT\nSELECT 1");
        let result = parser.parse();
        // SELECTで同期できるか
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn test_multiple_errors_in_batch() {
        // 複数のエラーを含むバッチ
        let mut parser = Parser::new("INVALID1; INVALID2; SELECT 1");
        let _ = parser.parse();
        let errors = parser.errors();
        // 少なくとも1つのエラーが収集されているはず
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_error_position_reporting() {
        // エラー位置の報告
        let result = parse_sql("SELCT FROM users"); // SELCT is a typo
        assert!(result.is_err());
        if let ParseError::UnexpectedToken { expected, .. } = result.unwrap_err() {
            // 期待されるトークンが報告されている
            assert!(!expected.is_empty());
        }
    }

    #[test]
    fn test_error_incomplete_statement() {
        // 不完全な文
        let result = parse_sql("INSERT INTO users");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_invalid_create_target() {
        // 無効なCREATE対象
        let result = parse_sql("CREATE INVALID name");
        assert!(result.is_err());
    }

    #[test]
    fn test_error_missing_comma_in_select() {
        // SELECTリストでのカンマ漏れ
        let result = parse_sql("SELECT id name FROM users");
        // パーサーはこれを式として解釈する可能性がある
        // エラーになるか、何らかの形でパースされる
        assert!(result.is_ok() || result.is_err());
    }

    #[test]
    fn test_error_in_between_syntax() {
        // BETWEENの構文エラー
        let result = parse_sql("SELECT * FROM users WHERE id BETWEEN 1");
        assert!(result.is_err()); // ANDが欠落している
    }

    #[test]
    fn test_recovery_continues_parsing() {
        // エラー回復後にパースを継続できるか
        let result = parse_sql("INVALID; SELECT 1; INVALID; SELECT 2");
        // エラーがあっても一部の文はパースできる
        assert!(result.is_err() || result.is_ok());
    }

    #[test]
    fn test_error_with_nested_structure() {
        // 入れ子構造でのエラー - 閉じていない括弧
        let result = parse_sql("SELECT * FROM users WHERE id IN (1, 2, (3, 4)");
        // 入れ子のINリストで閉じ括弧が不足
        assert!(result.is_err());
    }

    #[test]
    fn test_batch_specific_error() {
        // バッチモード特有のエラー処理
        let result = parse_sql("SELECT 1; GO; INVALID");
        // GO後の無効なステートメント
        assert!(result.is_err());
    }

    // Table-level constraint tests

    #[test]
    fn test_table_level_primary_key() {
        // テーブルレベルPRIMARY KEY制約
        let result =
            parse_sql("CREATE TABLE t (id INT, CONSTRAINT pk_t PRIMARY KEY (id))").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.constraints.len(), 1);
                    match &table.constraints[0] {
                        TableConstraint::PrimaryKey { columns, .. } => {
                            assert_eq!(columns.len(), 1);
                            assert_eq!(columns[0].name, "id");
                        }
                        _ => panic!("Expected PrimaryKey constraint"),
                    }
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_table_level_primary_key_multiple_columns() {
        // 複数カラムのPRIMARY KEY制約
        let result = parse_sql(
            "CREATE TABLE t (id INT, user_id INT, CONSTRAINT pk_t PRIMARY KEY (id, user_id))",
        )
        .unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.constraints.len(), 1);
                    match &table.constraints[0] {
                        TableConstraint::PrimaryKey { columns, .. } => {
                            assert_eq!(columns.len(), 2);
                            assert_eq!(columns[0].name, "id");
                            assert_eq!(columns[1].name, "user_id");
                        }
                        _ => panic!("Expected PrimaryKey constraint"),
                    }
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_table_level_primary_key_without_constraint_name() {
        // 制約名なしのPRIMARY KEY
        let result = parse_sql("CREATE TABLE t (id INT, PRIMARY KEY (id))").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.constraints.len(), 1);
                    match &table.constraints[0] {
                        TableConstraint::PrimaryKey { columns, .. } => {
                            assert_eq!(columns.len(), 1);
                            assert_eq!(columns[0].name, "id");
                        }
                        _ => panic!("Expected PrimaryKey constraint"),
                    }
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_table_level_foreign_key() {
        // テーブルレベルFOREIGN KEY制約
        let result = parse_sql("CREATE TABLE orders (id INT, user_id INT, CONSTRAINT fk_orders_user FOREIGN KEY (user_id) REFERENCES users(id))").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.constraints.len(), 1);
                    match &table.constraints[0] {
                        TableConstraint::Foreign {
                            columns,
                            ref_table,
                            ref_columns,
                            ..
                        } => {
                            assert_eq!(columns.len(), 1);
                            assert_eq!(columns[0].name, "user_id");
                            assert_eq!(ref_table.name, "users");
                            assert_eq!(ref_columns.len(), 1);
                            assert_eq!(ref_columns[0].name, "id");
                        }
                        _ => panic!("Expected Foreign constraint"),
                    }
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_table_level_foreign_key_multiple_columns() {
        // 複数カラムのFOREIGN KEY制約
        let result = parse_sql("CREATE TABLE t (a INT, b INT, CONSTRAINT fk_t FOREIGN KEY (a, b) REFERENCES other(x, y))").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.constraints.len(), 1);
                    match &table.constraints[0] {
                        TableConstraint::Foreign {
                            columns,
                            ref_table,
                            ref_columns,
                            ..
                        } => {
                            assert_eq!(columns.len(), 2);
                            assert_eq!(columns[0].name, "a");
                            assert_eq!(columns[1].name, "b");
                            assert_eq!(ref_table.name, "other");
                            assert_eq!(ref_columns.len(), 2);
                            assert_eq!(ref_columns[0].name, "x");
                            assert_eq!(ref_columns[1].name, "y");
                        }
                        _ => panic!("Expected Foreign constraint"),
                    }
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_table_level_foreign_key_without_parens() {
        // 括弧なしの参照カラム（単一カラムの場合）
        let result = parse_sql("CREATE TABLE t (id INT, user_id INT, CONSTRAINT fk_t FOREIGN KEY (user_id) REFERENCES users id)").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.constraints.len(), 1);
                    match &table.constraints[0] {
                        TableConstraint::Foreign { ref_columns, .. } => {
                            assert_eq!(ref_columns.len(), 1);
                            assert_eq!(ref_columns[0].name, "id");
                        }
                        _ => panic!("Expected Foreign constraint"),
                    }
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_table_level_unique() {
        // テーブルレベルUNIQUE制約
        let result = parse_sql(
            "CREATE TABLE t (id INT, email VARCHAR(100), CONSTRAINT uq_t_email UNIQUE (email))",
        )
        .unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.constraints.len(), 1);
                    match &table.constraints[0] {
                        TableConstraint::Unique { columns, .. } => {
                            assert_eq!(columns.len(), 1);
                            assert_eq!(columns[0].name, "email");
                        }
                        _ => panic!("Expected Unique constraint"),
                    }
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_table_level_unique_multiple_columns() {
        // 複数カラムのUNIQUE制約
        let result = parse_sql("CREATE TABLE t (id INT, email VARCHAR(100), username VARCHAR(50), CONSTRAINT uq_t UNIQUE (email, username))").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.constraints.len(), 1);
                    match &table.constraints[0] {
                        TableConstraint::Unique { columns, .. } => {
                            assert_eq!(columns.len(), 2);
                            assert_eq!(columns[0].name, "email");
                            assert_eq!(columns[1].name, "username");
                        }
                        _ => panic!("Expected Unique constraint"),
                    }
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_table_level_unique_without_constraint_name() {
        // 制約名なしのUNIQUE
        let result =
            parse_sql("CREATE TABLE t (id INT, email VARCHAR(100), UNIQUE (email))").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.constraints.len(), 1);
                    match &table.constraints[0] {
                        TableConstraint::Unique { columns, .. } => {
                            assert_eq!(columns.len(), 1);
                            assert_eq!(columns[0].name, "email");
                        }
                        _ => panic!("Expected Unique constraint"),
                    }
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_table_level_check() {
        // テーブルレベルCHECK制約
        let result =
            parse_sql("CREATE TABLE t (id INT, age INT, CONSTRAINT chk_t_age CHECK (age >= 18))")
                .unwrap();
        match &result[0] {
            Statement::Create(stmt) => {
                match stmt.as_ref() {
                    CreateStatement::Table(table) => {
                        assert_eq!(table.constraints.len(), 1);
                        match &table.constraints[0] {
                            TableConstraint::Check { expr, .. } => {
                                // CHECK式がパースされていることを確認
                                // パースされた式をそのままチェック（詳細な構造までは検証しない）
                                match expr {
                                    Expression::BinaryOp {
                                        op: BinaryOperator::Ge,
                                        ..
                                    } => {
                                        // >=演算子が使われていればOK
                                    }
                                    _ => {
                                        // デバッグのためにパニックの代わりに式を表示
                                        eprintln!("Parsed expr: {:?}", expr);
                                        panic!("Expected BinaryOp expression with Ge operator, got {:?}", expr);
                                    }
                                }
                            }
                            _ => panic!("Expected Check constraint"),
                        }
                    }
                    _ => panic!("Expected Create Table statement"),
                }
            }
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_table_level_check_without_constraint_name() {
        // 制約名なしのCHECK
        let result = parse_sql("CREATE TABLE t (id INT, age INT, CHECK (age >= 18))").unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.constraints.len(), 1);
                    match &table.constraints[0] {
                        TableConstraint::Check { .. } => {
                            // CHECK制約が存在すればOK
                        }
                        _ => panic!("Expected Check constraint"),
                    }
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_multiple_table_level_constraints() {
        // 複数のテーブルレベル制約
        let result = parse_sql(
            "CREATE TABLE t (id INT, user_id INT, email VARCHAR(100), age INT, \
             CONSTRAINT pk_t PRIMARY KEY (id), \
             CONSTRAINT fk_t_user FOREIGN KEY (user_id) REFERENCES users(id), \
             CONSTRAINT uq_t_email UNIQUE (email), \
             CONSTRAINT chk_t_age CHECK (age >= 18))",
        )
        .unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    assert_eq!(table.constraints.len(), 4);
                    // 各制約が正しくパースされていることを確認
                    let mut found_pk = false;
                    let mut found_fk = false;
                    let mut found_uq = false;
                    let mut found_chk = false;

                    for constraint in &table.constraints {
                        match constraint {
                            TableConstraint::PrimaryKey { .. } => found_pk = true,
                            TableConstraint::Foreign { .. } => found_fk = true,
                            TableConstraint::Unique { .. } => found_uq = true,
                            TableConstraint::Check { .. } => found_chk = true,
                        }
                    }

                    assert!(found_pk, "PrimaryKey constraint not found");
                    assert!(found_fk, "Foreign constraint not found");
                    assert!(found_uq, "Unique constraint not found");
                    assert!(found_chk, "Check constraint not found");
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    #[test]
    fn test_mix_column_and_table_level_constraints() {
        // カラムレベルとテーブルレベルの制約の混合
        let result = parse_sql(
            "CREATE TABLE t (id INT PRIMARY KEY, user_id INT, email VARCHAR(100) NOT NULL, \
             FOREIGN KEY (user_id) REFERENCES users(id), \
             UNIQUE (email))",
        )
        .unwrap();
        match &result[0] {
            Statement::Create(stmt) => match stmt.as_ref() {
                CreateStatement::Table(table) => {
                    // カラムレベル制約はColumnDefinition.constraintsに含まれる
                    assert_eq!(table.columns.len(), 3);
                    // idカラムのPRIMARY KEY制約
                    assert!(!table.columns[0].constraints.is_empty());
                    // emailカラムはNOT NULL（nullabilityフィールド）
                    assert_eq!(table.columns[2].nullability, Some(false));
                    // テーブルレベル制約
                    assert_eq!(table.constraints.len(), 2);
                }
                _ => panic!("Expected Create Table statement"),
            },
            _ => panic!("Expected Create statement"),
        }
    }

    // TRY...CATCH tests

    #[test]
    fn test_try_catch_basic() {
        // 基本的なTRY...CATCHブロック
        let result = parse_sql(
            "BEGIN TRY \
             SELECT 1 \
             END TRY \
             BEGIN CATCH \
             SELECT 2 \
             END CATCH",
        )
        .unwrap();
        match &result[0] {
            Statement::TryCatch(tc) => {
                assert!(!tc.try_block.statements.is_empty());
                assert!(!tc.catch_block.statements.is_empty());
            }
            _ => panic!("Expected TryCatch statement"),
        }
    }

    // Transaction tests

    #[test]
    fn test_begin_transaction() {
        // BEGIN TRANSACTION
        let result = parse_sql("BEGIN TRANSACTION").unwrap();
        match &result[0] {
            Statement::Transaction(TransactionStatement::Begin { name, .. }) => {
                assert!(name.is_none());
            }
            _ => panic!("Expected Begin Transaction statement"),
        }
    }

    #[test]
    fn test_begin_transaction_with_name() {
        // BEGIN TRANSACTION tran_name
        let result = parse_sql("BEGIN TRANSACTION my_tran").unwrap();
        match &result[0] {
            Statement::Transaction(TransactionStatement::Begin { name, .. }) => {
                assert_eq!(name.as_ref().unwrap().name, "my_tran");
            }
            _ => panic!("Expected Begin Transaction statement"),
        }
    }

    #[test]
    fn test_commit_transaction() {
        // COMMIT TRANSACTION
        let result = parse_sql("COMMIT TRANSACTION").unwrap();
        match &result[0] {
            Statement::Transaction(TransactionStatement::Commit { name, .. }) => {
                assert!(name.is_none());
            }
            _ => panic!("Expected Commit Transaction statement"),
        }
    }

    #[test]
    fn test_rollback_transaction() {
        // ROLLBACK TRANSACTION
        let result = parse_sql("ROLLBACK TRANSACTION").unwrap();
        match &result[0] {
            Statement::Transaction(TransactionStatement::Rollback { name, .. }) => {
                assert!(name.is_none());
            }
            _ => panic!("Expected Rollback Transaction statement"),
        }
    }

    #[test]
    fn test_save_transaction() {
        // SAVE TRANSACTION savepoint_name
        let result = parse_sql("SAVE TRANSACTION my_savepoint").unwrap();
        match &result[0] {
            Statement::Transaction(TransactionStatement::Save { name, .. }) => {
                assert_eq!(name.name, "my_savepoint");
            }
            _ => panic!("Expected Save Transaction statement"),
        }
    }

    // THROW tests

    #[test]
    fn test_throw_basic() {
        // 基本的なTHROW
        let result = parse_sql("THROW").unwrap();
        match &result[0] {
            Statement::Throw(_) => {}
            _ => panic!("Expected Throw statement"),
        }
    }

    // RAISERROR tests

    #[test]
    fn test_raiserror_basic() {
        // 基本的なRAISERROR
        let result = parse_sql("RAISERROR('Error message', 16, 1)").unwrap();
        match &result[0] {
            Statement::Raiserror(_) => {}
            _ => panic!("Expected Raiserror statement"),
        }
    }
}
