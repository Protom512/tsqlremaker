use tsql_lexer::Lexer;

fn main() {
    let sql = "1 = 1";
    let lexer = Lexer::new(sql);
    for token in lexer {
        println!("{:?}", token.kind);
    }
}
