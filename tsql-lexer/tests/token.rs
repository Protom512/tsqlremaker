#[cfg(test)]
mod tests {
    use tsql_lexer::Lexer;
    use tsql_token::{Token, *};
    #[test]
    fn test_simple_select() {
        let sql = r"
            select top 1 t1.id,t2.name from table1 t1,table2 t2
            where t2.id=t1.id
            ";
        let mut var_name = Lexer::new(sql);
        assert_eq!(var_name.ch(), '\n');
        let mut tokens: Vec<Token> = vec![];
        loop {
            let mut token = var_name.next_token();
            if token.token_type() == "EOF" {
                break;
            }

            tokens.push(token);
        }
        dbg!(&tokens);

        assert_eq!(tokens[0].token_type(), SELECT);
        assert_eq!(tokens[1].token_type(), IDENT);
        assert_eq!(tokens[1].token_type(), IDENT);
    }
}
