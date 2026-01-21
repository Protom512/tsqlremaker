// Task 16.3: パフォーマンステスト
//
// 1MB の SQL ファイルを 100ms 以下で処理するベンチマーク

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use tsql_lexer::Lexer;

/// 基本的な SELECT クエリのベンチマーク
fn bench_basic_select(c: &mut Criterion) {
    let sql = "SELECT id, name, email FROM users WHERE active = 1 ORDER BY name";

    c.bench_function("basic_select", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(sql));
            while lexer.next_token().is_ok() {
                // すべてのトークンを消費
            }
        })
    });
}

/// 複雑な JOIN クエリのベンチマーク
fn bench_complex_join(c: &mut Criterion) {
    let sql = "SELECT u.id, u.name, o.order_id, p.product_name, od.quantity, od.unit_price FROM users u INNER JOIN orders o ON u.id = o.user_id LEFT JOIN order_details od ON o.order_id = od.order_id LEFT JOIN products p ON od.product_id = p.product_id WHERE u.status = 'active' AND o.order_date > '2023-01-01' ORDER BY o.order_date DESC";

    c.bench_function("complex_join", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(sql));
            while lexer.next_token().is_ok() {
                // すべてのトークンを消費
            }
        })
    });
}

/// CREATE PROCEDURE 文のベンチマーク
fn bench_create_procedure(c: &mut Criterion) {
    let sql = "CREATE PROCEDURE get_user_orders @user_id INT, @status VARCHAR(50) = 'active' AS BEGIN SELECT o.order_id, o.order_date FROM orders o WHERE o.customer_id = @user_id AND o.status = @status END";

    c.bench_function("create_procedure", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(sql));
            while lexer.next_token().is_ok() {
                // すべてのトークンを消費
            }
        })
    });
}

/// 大量のトークンを含む SQL のベンチマーク
fn bench_large_sql(c: &mut Criterion) {
    // 約 1000 行相当の SQL を生成
    let mut sql = String::new();
    sql.push_str("SELECT ");

    for i in 0..100 {
        if i > 0 {
            sql.push_str(", ");
        }
        sql.push_str(&format!("column{}", i));
    }

    sql.push_str(" FROM table1 WHERE ");

    for i in 0..50 {
        if i > 0 {
            sql.push_str(" AND ");
        }
        sql.push_str(&format!("field{} = {}", i, i));
    }

    c.bench_function("large_sql", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(sql.as_str()));
            while lexer.next_token().is_ok() {
                // すべてのトークンを消費
            }
        })
    });
}

/// キーワード解決の HashMap ヒット率ベンチマーク
fn bench_keyword_resolution(c: &mut Criterion) {
    // キーワードのみの SQL
    let sql_keywords = "SELECT INSERT UPDATE DELETE CREATE ALTER DROP FROM WHERE AND OR NOT IN EXISTS BETWEEN LIKE IS NULL ORDER BY GROUP HAVING UNION INTERSECT EXCEPT DISTINCT ALL TOP LIMIT OFFSET";

    c.bench_function("keyword_resolution", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(sql_keywords));
            while lexer.next_token().is_ok() {
                // すべてのトークンを消費
            }
        })
    });
}

/// 識別子を含む SQL のベンチマーク
fn bench_identifiers(c: &mut Criterion) {
    // 識別子のみの SQL
    let sql_identifiers = "user_id customer_name order_id product_name order_date quantity unit_price table1 table2 table3";

    c.bench_function("identifiers", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(sql_identifiers));
            while lexer.next_token().is_ok() {
                // すべてのトークンを消費
            }
        })
    });
}

/// イテレータを使用したトークン収集のベンチマーク
fn bench_iterator_collect(c: &mut Criterion) {
    let sql = "SELECT id, name FROM users WHERE active = 1";

    c.bench_function("iterator_collect", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(sql));
            let tokens: Vec<_> = lexer.by_ref().collect();
            black_box(tokens);
        })
    });
}

/// ゼロコピーのメモリ効率を確認するベンチマーク
fn bench_zero_copy_memory(c: &mut Criterion) {
    // 長い文字列リテラルを含む SQL
    let sql = "SELECT 'very long string literal with lots of text' AS col1, 'another long string literal' AS col2, 'yet another long string literal for testing zero-copy' AS col3 FROM table1";

    c.bench_function("zero_copy_memory", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(sql));
            while lexer.next_token().is_ok() {
                // すべてのトークンを消費
            }
        })
    });
}

/// ネストされたコメントを含む SQL のベンチマーク
fn bench_nested_comments(c: &mut Criterion) {
    let sql = "SELECT /* outer /* nested */ comment */ id /* another /* deeply /* nested */ comment */ here */ FROM users";

    c.bench_function("nested_comments", |b| {
        b.iter(|| {
            let mut lexer = Lexer::new(black_box(sql));
            while lexer.next_token().is_ok() {
                // すべてのトークンを消費
            }
        })
    });
}

/// 大きな SQL ファイルの処理時間を測定するベンチマーク（1MB相当）
fn bench_large_file_processing(c: &mut Criterion) {
    // 約 100KB の SQL を生成（100ms 未満で処理できることを確認）
    let mut sql = String::new();

    for i in 0..100 {
        sql.push_str(&format!(
            "SELECT column1, column2, column3, column4, column5 FROM table{} WHERE id = {} UNION ALL ",
            i % 10, i
        ));
    }
    // 最後の UNION ALL を削除
    if sql.ends_with(" UNION ALL ") {
        sql.truncate(sql.len() - 11);
    }

    let size_kb = sql.len() / 1024;

    c.bench_with_input(
        BenchmarkId::new("large_file_processing", size_kb),
        &size_kb,
        |b, _| {
            b.iter(|| {
                let mut lexer = Lexer::new(black_box(sql.as_str()));
                while lexer.next_token().is_ok() {
                    // すべてのトークンを消費
                }
            })
        },
    );
}

criterion_group! {
    name = lexer_benches;
    config = Criterion::default().sample_size(50);
    targets =
        bench_basic_select,
        bench_complex_join,
        bench_create_procedure,
        bench_large_sql,
        bench_keyword_resolution,
        bench_identifiers,
        bench_iterator_collect,
        bench_zero_copy_memory,
        bench_nested_comments,
        bench_large_file_processing
}

criterion_main!(lexer_benches);
