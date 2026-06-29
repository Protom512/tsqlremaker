-- SELECT fixtures for mysql-emitter integration tests.
-- Each statement is parseable by tsql-parser and convertible to common_sql::ast.
SELECT * FROM users;
SELECT id, name FROM users WHERE id = 1;
SELECT * FROM users ORDER BY name ASC;
SELECT DISTINCT id FROM users;
SELECT id, name FROM users WHERE id > 100 AND status = 'active';
