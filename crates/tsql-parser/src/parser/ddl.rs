//! DDL (Data Definition Language) パーサーメソッド
//!
//! CREATE, ALTER, および関連するデータ型・制約のパース処理。

use crate::ast::*;
use crate::error::{ParseError, ParseResult};
use crate::expression::ExpressionParser;
use tsql_token::{Span, TokenKind};

impl<'src> super::Parser<'src> {
    /// CREATE文を解析
    pub(super) fn parse_create_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // CREATE

        // CREATE UNIQUE INDEX
        if self.buffer.check(TokenKind::Unique) {
            self.buffer.consume()?;
            if !self.buffer.check(TokenKind::Index) {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::Index],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.position,
                ));
            }
            self.buffer.consume()?;
            return self.parse_create_index(start, true);
        }

        match self.buffer.current()?.kind {
            TokenKind::Table => {
                self.buffer.consume()?;
                self.parse_create_table(start)
            }
            TokenKind::Index => {
                self.buffer.consume()?;
                self.parse_create_index(start, false)
            }
            TokenKind::View => {
                self.buffer.consume()?;
                self.parse_create_view(start)
            }
            TokenKind::Procedure | TokenKind::Proc => {
                self.buffer.consume()?;
                self.parse_create_procedure(start)
            }
            TokenKind::Trigger => {
                self.buffer.consume()?;
                self.parse_create_trigger(start)
            }
            _ => Err(ParseError::unexpected_token(
                vec![
                    TokenKind::Table,
                    TokenKind::Unique,
                    TokenKind::Index,
                    TokenKind::View,
                    TokenKind::Procedure,
                    TokenKind::Trigger,
                ],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            )),
        }
    }

    /// ALTER TABLE文を解析
    pub(super) fn parse_alter_statement(&mut self) -> ParseResult<Statement> {
        let start = self.buffer.current()?.span.start;
        self.buffer.consume()?; // ALTER

        if !self.buffer.check(TokenKind::Table) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Table],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?; // TABLE

        let table = self.parse_identifier()?;

        let operation = {
            let cur = self.buffer.current()?;
            let is_add = cur.kind == TokenKind::Ident && cur.text.eq_ignore_ascii_case("ADD");
            let is_drop = cur.kind == TokenKind::Drop;
            let is_alter = cur.kind == TokenKind::Alter;
            let is_column = cur.kind == TokenKind::Ident && cur.text.eq_ignore_ascii_case("COLUMN");

            if is_add {
                self.buffer.consume()?; // ADD
                let name = self.parse_identifier()?;
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
                            self.buffer.current()?.position,
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

                AlterTableOperation::AddColumn(AddColumnDefinition {
                    name,
                    data_type,
                    nullability,
                    identity,
                })
            } else if is_drop {
                self.buffer.consume()?; // DROP
                                        // Optional COLUMN keyword (identifier "COLUMN")
                if self.buffer.current()?.kind == TokenKind::Ident
                    && self.buffer.current()?.text.eq_ignore_ascii_case("COLUMN")
                {
                    self.buffer.consume()?;
                }
                let name = self.parse_identifier()?;
                AlterTableOperation::DropColumn(name)
            } else if is_alter || is_column {
                // ALTER COLUMN or just COLUMN (ALTER is a keyword, COLUMN is an ident)
                if is_alter {
                    self.buffer.consume()?; // ALTER
                }
                self.buffer.consume()?; // COLUMN
                let name = self.parse_identifier()?;
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
                            self.buffer.current()?.position,
                        ));
                    }
                    self.buffer.consume()?;
                    Some(false)
                } else {
                    None
                };

                AlterTableOperation::AlterColumn(AlterColumnDefinition {
                    name,
                    data_type,
                    nullability,
                })
            } else {
                return Err(ParseError::unexpected_token(
                    vec![TokenKind::Ident, TokenKind::Drop],
                    self.buffer.current()?.kind,
                    self.buffer.current()?.position,
                ));
            }
        };

        let end_span = self.buffer.current()?.span;
        Ok(Statement::AlterTable(Box::new(AlterTableStatement {
            span: Span {
                start,
                end: end_span.end,
            },
            table,
            operation,
        })))
    }

    /// CREATE TABLEを解析
    pub(super) fn parse_create_table(&mut self, start: u32) -> ParseResult<Statement> {
        let name = self.parse_identifier()?;
        // 一時テーブルは `#` または `##` で始まる識別子
        let temporary = name.name.starts_with('#');

        if !self.buffer.check(TokenKind::LParen) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::LParen],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
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
                let token_span = token.position;
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
                            self.buffer.current()?.position,
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
                                    self.buffer.current()?.position,
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
                self.buffer.current()?.position,
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
    pub(super) fn is_table_constraint_start(&self) -> bool {
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
    pub(super) fn parse_table_constraint(&mut self) -> ParseResult<TableConstraint> {
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
                        self.buffer.current()?.position,
                    ));
                }
                self.buffer.consume()?;

                // カラムリストをパース
                if !self.buffer.check(TokenKind::LParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::LParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.position,
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
                        self.buffer.current()?.position,
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
                        self.buffer.current()?.position,
                    ));
                }
                self.buffer.consume()?;

                // カラムリストをパース
                if !self.buffer.check(TokenKind::LParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::LParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.position,
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
                        self.buffer.current()?.position,
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
                        self.buffer.current()?.position,
                    ));
                }
                self.buffer.consume()?;

                let mut expr_parser = ExpressionParser::new(&mut self.buffer);
                let expr = expr_parser.parse()?;

                if !self.buffer.check(TokenKind::RParen) {
                    return Err(ParseError::unexpected_token(
                        vec![TokenKind::RParen],
                        self.buffer.current()?.kind,
                        self.buffer.current()?.position,
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
                self.buffer.current()?.position,
            )),
        }
    }

    /// データ型を解析
    pub(super) fn parse_data_type(&mut self) -> ParseResult<DataType> {
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
                                format!("Invalid number for VARCHAR length: {num_str}"),
                                token.position,
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
                            self.buffer.current()?.position,
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
                                format!("Invalid number for CHAR length: {num_str}"),
                                token.position,
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
                            self.buffer.current()?.position,
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
                                format!("Invalid number for DECIMAL precision: {num_str}"),
                                token.position,
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
                                    format!("Invalid number for DECIMAL scale: {num_str}"),
                                    token.position,
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
                            self.buffer.current()?.position,
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
            TokenKind::Float => DataType::Float,
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
                        let token = self.buffer.current()?;
                        let num_str = token.text;
                        let n = num_str.parse::<u32>().map_err(|_| {
                            ParseError::invalid_syntax(
                                format!("Invalid number for BINARY length: {num_str}"),
                                token.position,
                            )
                        })?;
                        self.buffer.consume()?;
                        n
                    } else {
                        1
                    };
                    if !self.buffer.check(TokenKind::RParen) {
                        return Err(ParseError::unexpected_token(
                            vec![TokenKind::RParen],
                            self.buffer.current()?.kind,
                            self.buffer.current()?.position,
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
                    self.buffer.current()?.position,
                ))
            }
        })
    }

    /// CREATE INDEXを解析
    pub(super) fn parse_create_index(
        &mut self,
        start: u32,
        unique: bool,
    ) -> ParseResult<Statement> {
        let name = self.parse_identifier()?;

        if !self.buffer.check(TokenKind::On) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::On],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?;

        let table = self.parse_identifier()?;
        if !self.buffer.check(TokenKind::LParen) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::LParen],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
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
                unique,
            },
        ))))
    }

    /// CREATE VIEWを解析
    pub(super) fn parse_create_view(&mut self, start: u32) -> ParseResult<Statement> {
        let name = self.parse_identifier()?;

        if !self.buffer.check(TokenKind::As) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::As],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?;

        let select_stmt = self.parse_select_statement()?;
        let select = match select_stmt {
            Statement::Select(s) => s,
            _ => {
                return Err(ParseError::invalid_syntax(
                    "Expected SELECT statement".to_string(),
                    self.buffer.current()?.position,
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
    pub(super) fn parse_create_procedure(&mut self, start: u32) -> ParseResult<Statement> {
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
                self.buffer.current()?.position,
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

    /// CREATE TRIGGER文を解析
    ///
    /// T-SQL構文: CREATE TRIGGER name ON table FOR {INSERT|UPDATE|DELETE [, ...]} AS body
    pub(super) fn parse_create_trigger(&mut self, start: u32) -> ParseResult<Statement> {
        let name = self.parse_identifier()?;

        // ON
        if !self.buffer.check(TokenKind::On) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::On],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?;

        let table = self.parse_identifier()?;

        // FOR (not a registered keyword — comes as Ident)
        let cur = self.buffer.current()?;
        if cur.kind != TokenKind::Ident || !cur.text.eq_ignore_ascii_case("FOR") {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::Ident],
                cur.kind,
                cur.position,
            ));
        }
        self.buffer.consume()?;

        // Trigger events: INSERT, UPDATE, DELETE (comma-separated)
        let mut events = Vec::new();
        loop {
            let event = self.parse_trigger_event()?;
            events.push(event);
            if !self.buffer.consume_if(TokenKind::Comma)? {
                break;
            }
        }

        if events.is_empty() {
            return Err(ParseError::invalid_syntax(
                "Expected at least one trigger event (INSERT, UPDATE, or DELETE)".to_string(),
                self.buffer.current()?.position,
            ));
        }

        // AS
        if !self.buffer.check(TokenKind::As) {
            return Err(ParseError::unexpected_token(
                vec![TokenKind::As],
                self.buffer.current()?.kind,
                self.buffer.current()?.position,
            ));
        }
        self.buffer.consume()?;

        // Trigger body (BEGIN...END or single statement)
        let body = if self.buffer.check(TokenKind::Begin) {
            let block = self.parse_block()?;
            vec![block]
        } else {
            vec![self.parse_statement()?]
        };

        let end_span = self.buffer.current()?.span;
        Ok(Statement::Create(Box::new(CreateStatement::Trigger(
            TriggerDefinition {
                span: Span {
                    start,
                    end: end_span.end,
                },
                name,
                table,
                events,
                body,
            },
        ))))
    }

    /// トリガーイベント種別を解析 (INSERT, UPDATE, DELETE)
    pub(super) fn parse_trigger_event(&mut self) -> ParseResult<TriggerEvent> {
        let current = self.buffer.current()?;
        if current.kind == TokenKind::Insert {
            self.buffer.consume()?;
            Ok(TriggerEvent::Insert)
        } else if current.kind == TokenKind::Update {
            self.buffer.consume()?;
            Ok(TriggerEvent::Update)
        } else if current.kind == TokenKind::Delete {
            self.buffer.consume()?;
            Ok(TriggerEvent::Delete)
        } else {
            Err(ParseError::unexpected_token(
                vec![TokenKind::Insert, TokenKind::Update, TokenKind::Delete],
                current.kind,
                current.position,
            ))
        }
    }

    /// パラメータ定義を解析
    ///
    /// T-SQL構文: @parameter_name [AS] data_type [ = default_value ] [OUTPUT]
    pub(super) fn parse_parameter_definition(&mut self) -> ParseResult<ParameterDefinition> {
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
}
