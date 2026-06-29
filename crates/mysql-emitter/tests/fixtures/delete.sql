-- DELETE fixtures for mysql-emitter integration tests.
DELETE FROM users WHERE id = 1;
DELETE FROM logs WHERE level = 'debug';
