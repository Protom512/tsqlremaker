-- UPDATE fixtures for mysql-emitter integration tests.
UPDATE users SET name = 'Bob' WHERE id = 1;
UPDATE accounts SET balance = balance + 100 WHERE id = 5;
