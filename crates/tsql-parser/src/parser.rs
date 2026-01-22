//! パーサーモジュール
//!
//! T-SQLの構文解析を行うメインパーサー。

use crate::ast::*;
use crate::buffer::TokenBuffer;
use crate::error::{ParseError, ParseResult};
use crate::expression::ExpressionParser;
use tsql_lexer::Lexer;
use tsql_token::{Span, TokenKind};

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
            max_depth: 1000,
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
            // BEGINブロック
            TokenKind::Begin => self.parse_block(),
            // BREAK文
            TokenKind::Break => self.parse_break_statement(),
            // CONTINUE文
            TokenKind::Continue => self.parse_continue_statement(),
            // RETURN文
            TokenKind::Return => self.parse_return_statement(),
            // GOバッチ区切り
            TokenKind::Ident if self.is_go_keyword() => self.parse_batch_separator(),
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
            top = Some(expr_parser.parse()?);
        }

        // カラムリスト
        let mut columns = Vec::new();
        loop {
            columns.push(self.parse_select_item()?);
            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }

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
        let mut group_by = Vec::new();
        if self.buffer.check(TokenKind::Group) {
            self.buffer.consume()?;
            if !self.buffer.check(TokenKind::By) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::By],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?;
            loop {
                let mut expr_parser = ExpressionParser::new(&mut self.buffer);
                group_by.push(expr_parser.parse()?);
                if !self.buffer.consume_if(TokenKind::Comma)? {
                    break;
                }
            }
        }

        // HAVING
        let having = if self.buffer.check(TokenKind::Having) {
            self.buffer.consume()?;
            let mut expr_parser = ExpressionParser::new(&mut self.buffer);
            Some(expr_parser.parse()?)
        } else {
            None
        };

        // ORDER BY
        let mut order_by = Vec::new();
        if self.buffer.check(TokenKind::Order) {
            self.buffer.consume()?;
            if !self.buffer.check(TokenKind::By) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::By],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.span,
                ));
            }
            self.buffer.consume()?;
            loop {
                order_by.push(self.parse_order_by_item()?);
                if !self.buffer.consume_if(TokenKind::Comma)? {
                    break;
                }
            }
        }

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
        let tables = vec![self.parse_table_reference()?];
        let joins = Vec::new();
        Ok(FromClause { tables, joins })
    }

    /// テーブル参照を解析
    fn parse_table_reference(&mut self) -> ParseResult<TableReference> {
        let start = self.buffer.current()?.span.start;
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
            assignments.push(Assignment { column, value });
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
        let temporary = name.name.starts_with('#') || name.name.starts_with('[');

        self.buffer.consume()?; // LEFT PAREN

        let mut columns = Vec::new();
        let mut constraints = Vec::new();

        while !self.buffer.check(TokenKind::RParen) {
            let token = self.buffer.current()?;
            match token.kind {
                TokenKind::Ident | TokenKind::QuotedIdent => {
                    // カラム定義か制約
                    let name = self.parse_identifier()?;
                    if self.buffer.check(TokenKind::Constraint) || self.is_constraint_keyword() {
                        // テーブル制約
                        let constraint = self.parse_table_constraint(name)?;
                        constraints.push(constraint);
                    } else {
                        // カラム定義
                        let data_type = self.parse_data_type()?;
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

                        let identity = self.buffer.check(TokenKind::Identity);
                        if identity {
                            self.buffer.consume()?;
                        }

                        columns.push(ColumnDefinition {
                            name,
                            data_type,
                            nullability,
                            default_value: None,
                            identity,
                        });
                    }
                }
                _ => {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::Ident, TokenKind::RParen],
                        token.kind,
                        token.span,
                    ));
                }
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

    /// 制約キーワードかチェック
    fn is_constraint_keyword(&self) -> bool {
        let kind = match self.buffer.current() {
            Ok(t) => t.kind,
            Err(_) => return false,
        };
        matches!(
            kind,
            TokenKind::Primary | TokenKind::Foreign | TokenKind::Unique | TokenKind::Check
        )
    }

    /// テーブル制約を解析
    fn parse_table_constraint(&mut self, name: Identifier) -> ParseResult<TableConstraint> {
        match self.buffer.current()?.kind {
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
                self.buffer.consume()?; // LEFT PAREN
                let mut columns = Vec::new();
                while !self.buffer.check(TokenKind::RParen) {
                    columns.push(self.parse_identifier()?);
                    if !self.buffer.consume_if(TokenKind::Comma)? {
                        break;
                    }
                }
                self.buffer.consume()?; // RIGHT PAREN
                Ok(TableConstraint::PrimaryKey { columns })
            }
            _ => Ok(TableConstraint::Unique {
                columns: vec![name],
            }),
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
                    // 数値を解析（簡易版）
                    let len = if self.buffer.check(TokenKind::Number) {
                        let n = self.buffer.current()?.text.parse().unwrap_or(255);
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
                DataType::Char(len)
            }
            TokenKind::Decimal | TokenKind::Numeric => {
                let (precision, scale) = if self.buffer.check(TokenKind::LParen) {
                    self.buffer.consume()?;
                    let p = if self.buffer.check(TokenKind::Number) {
                        let p = self.buffer.current()?.text.parse().unwrap_or(18) as u8;
                        self.buffer.consume()?;
                        Some(p)
                    } else {
                        None
                    };
                    let scale = if self.buffer.check(TokenKind::Comma) {
                        self.buffer.consume()?;
                        if self.buffer.check(TokenKind::Number) {
                            let s = self.buffer.current()?.text.parse().unwrap_or(0) as u8;
                            self.buffer.consume()?;
                            Some(s)
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
            _ => DataType::Int,
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
        let mut parameters = Vec::new();
        if self.buffer.check(TokenKind::LParen) {
            self.buffer.consume()?;
            while !self.buffer.check(TokenKind::RParen) {
                parameters.push(self.parse_parameter_definition()?);
                if !self.buffer.consume_if(TokenKind::Comma)? {
                    break;
                }
            }
            self.buffer.consume()?;
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
    fn parse_parameter_definition(&mut self) -> ParseResult<ParameterDefinition> {
        let name = self.parse_identifier()?;
        let data_type = self.parse_data_type()?;
        let mut default_value = None;
        let mut is_output = false;

        if self.buffer.check(TokenKind::Eq) || self.buffer.check(TokenKind::Default) {
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

        let name = if current.kind == TokenKind::QuotedIdent {
            // [name] の形式
            &current.text[1..current.text.len() - 1]
        } else if current.kind == TokenKind::Ident || current.kind == TokenKind::LocalVar {
            current.text
        } else {
            return Err(ParseError::unexpected_token(
                vec![
                    TokenKind::Ident,
                    TokenKind::QuotedIdent,
                    TokenKind::LocalVar,
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

    /// 現在のトークンがGOキーワードか判定
    fn is_go_keyword(&self) -> bool {
        if self.mode == ParserMode::SingleStatement {
            return false;
        }
        let current = self.buffer.current();
        match current {
            Ok(token) => token.kind == TokenKind::Ident && token.text.eq_ignore_ascii_case("go"),
            _ => false,
        }
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
            Statement::Create(stmt) => match &**stmt {
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
}
