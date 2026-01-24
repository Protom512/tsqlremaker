//! Criterion ベンチマークスイート
//!
//! T-SQL パーサーのパフォーマンス測定。

// ベンチマークコードでは unwrap を許可
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tsql_parser::parse;

/// 簡単な SELECT 文のベンチマーク
fn bench_simple_select(c: &mut Criterion) {
    let sql = "SELECT id, name, email FROM users WHERE status = 'active'";

    c.bench_function("simple_select", |b| {
        b.iter(|| {
            black_box(parse(black_box(sql))).unwrap();
        });
    });
}

/// 複雑な JOIN クエリのベンチマーク
fn bench_complex_join(c: &mut Criterion) {
    let sql = r#"
        SELECT u.id, u.name, o.order_id, p.product_name, o.order_date
        FROM users u
        INNER JOIN orders o ON u.id = o.user_id
        LEFT JOIN order_items oi ON o.order_id = oi.order_id
        LEFT JOIN products p ON oi.product_id = p.product_id
        WHERE u.status = 'active' AND o.order_date >= '2024-01-01'
        ORDER BY o.order_date DESC
    "#;

    c.bench_function("complex_join", |b| {
        b.iter(|| {
            black_box(parse(black_box(sql))).unwrap();
        });
    });
}

/// 集計関数を含むクエリのベンチマーク
fn bench_aggregate_query(c: &mut Criterion) {
    let sql = r#"
        SELECT
            department,
            COUNT(*) as emp_count,
            AVG(salary) as avg_salary,
            MIN(salary) as min_salary,
            MAX(salary) as max_salary,
            SUM(bonus) as total_bonus
        FROM employees
        WHERE status = 'active'
        GROUP BY department
        HAVING COUNT(*) > 5
        ORDER BY emp_count DESC
    "#;

    c.bench_function("aggregate_query", |b| {
        b.iter(|| {
            black_box(parse(black_box(sql))).unwrap();
        });
    });
}

/// 複数のステートメントのベンチマーク
fn bench_multiple_statements(c: &mut Criterion) {
    let sql = r#"
        SELECT * FROM users WHERE id = 1;
        SELECT * FROM orders WHERE user_id = 1;
        SELECT * FROM products WHERE category = 'electronics';
        UPDATE users SET last_login = GETDATE() WHERE id = 1;
        DELETE FROM temp_data WHERE created_at < '2020-01-01';
    "#;

    c.bench_function("multiple_statements", |b| {
        b.iter(|| {
            black_box(parse(black_box(sql))).unwrap();
        });
    });
}

/// 入力サイズによるパフォーマンス変化
fn bench_input_size_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("input_size");

    for size in [10, 50, 100, 500, 1000].iter() {
        let mut sql = String::new();
        for i in 0..*size {
            sql.push_str(&format!("SELECT {} as id, 'name_{}' as name; ", i, i));
        }

        let sql_len = sql.len();
        group.throughput(Throughput::Bytes(sql_len as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &sql, |b, sql| {
            b.iter(|| {
                black_box(parse(black_box(sql))).unwrap();
            });
        });
    }

    group.finish();
}

/// 深い入れ子の式のベンチマーク
fn bench_deeply_nested_expression(c: &mut Criterion) {
    let sql = "SELECT a + b * c - (d / e) % f FROM t";

    c.bench_function("deeply_nested_expression", |b| {
        b.iter(|| {
            black_box(parse(black_box(sql))).unwrap();
        });
    });
}

/// CASE 式のベンチマーク
fn bench_case_expression(c: &mut Criterion) {
    let sql = r#"
        SELECT
            CASE
                WHEN score >= 90 THEN 'A'
                WHEN score >= 80 THEN 'B'
                WHEN score >= 70 THEN 'C'
                WHEN score >= 60 THEN 'D'
                ELSE 'F'
            END as grade
        FROM students
    "#;

    c.bench_function("case_expression", |b| {
        b.iter(|| {
            black_box(parse(black_box(sql))).unwrap();
        });
    });
}

/// 大きな IN リストのベンチマーク
fn bench_large_in_list(c: &mut Criterion) {
    let mut values = Vec::new();
    for i in 1..=100 {
        values.push(i.to_string());
    }
    let sql = format!("SELECT * FROM users WHERE id IN ({})", values.join(", "));

    c.bench_function("large_in_list", |b| {
        b.iter(|| {
            black_box(parse(black_box(&sql))).unwrap();
        });
    });
}

criterion_group!(
    parser_benches,
    bench_simple_select,
    bench_complex_join,
    bench_aggregate_query,
    bench_multiple_statements,
    bench_input_size_scaling,
    bench_deeply_nested_expression,
    bench_case_expression,
    bench_large_in_list
);

criterion_main!(parser_benches);
