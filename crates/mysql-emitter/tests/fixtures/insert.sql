-- INSERT fixtures for mysql-emitter integration tests.
INSERT INTO users (id, name) VALUES (1, 'Alice');
INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob');
INSERT INTO archive (id) SELECT id FROM source;
