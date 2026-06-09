-- =============================================================================
-- UC-3: Incomplete SQL Fragments
-- Purpose: Test LSP behavior during real-time typing (incomplete statements)
-- These represent what a developer sees mid-edit in the editor.
-- =============================================================================

-- Typing a SELECT statement
SELECT

SELECT *

SELECT * FROM

SELECT * FROM users

SELECT * FROM users WHERE

SELECT * FROM users WHERE id =

SELECT * FROM users WHERE id = 1

SELECT id, name

SELECT id, name FROM

SELECT id, name FROM users WHERE

SELECT id, name FROM users WHERE status =

-- Typing an INSERT statement
INSERT

INSERT INTO

INSERT INTO users

INSERT INTO users (

INSERT INTO users (id, name

INSERT INTO users (id, name)

INSERT INTO users (id, name) VALUES

INSERT INTO users (id, name) VALUES (

INSERT INTO users (id, name) VALUES (1,

INSERT INTO users (id, name) VALUES (1, 'test'

INSERT INTO users (id, name) VALUES (1, 'test')

-- Typing a CREATE TABLE
CREATE

CREATE TABLE

CREATE TABLE users

CREATE TABLE users (

CREATE TABLE users (id

CREATE TABLE users (id INT

CREATE TABLE users (id INT,

CREATE TABLE users (id INT, name

CREATE TABLE users (id INT, name VARCHAR

CREATE TABLE users (id INT, name VARCHAR(

CREATE TABLE users (id INT, name VARCHAR(100

CREATE TABLE users (id INT, name VARCHAR(100),

CREATE TABLE users (id INT, name VARCHAR(100), email

CREATE TABLE users (id INT, name VARCHAR(100), email VARCHAR

CREATE TABLE users (id INT, name VARCHAR(100), email VARCHAR(255)

CREATE TABLE users (id INT, name VARCHAR(100), email VARCHAR(255))

-- Typing a stored procedure
CREATE PROC

CREATE PROCEDURE

CREATE PROCEDURE get_users

CREATE PROCEDURE get_users @

CREATE PROCEDURE get_users @status

CREATE PROCEDURE get_users @status INT

CREATE PROCEDURE get_users @status INT AS

CREATE PROCEDURE get_users @status INT AS SELECT

CREATE PROCEDURE get_users @status INT AS SELECT *

CREATE PROCEDURE get_users @status INT AS SELECT * FROM

CREATE PROCEDURE get_users @status INT AS SELECT * FROM users

-- Typing a DECLARE block
DECLARE

DECLARE @

DECLARE @x

DECLARE @x INT

DECLARE @x INT,

DECLARE @x INT, @y

DECLARE @x INT, @y VARCHAR

DECLARE @x INT, @y VARCHAR(100)

-- Typing IF statement
IF

IF @

IF @x

IF @x =

IF @x = 1

IF @x = 1 SELECT

IF @x = 1 SELECT *

IF @x = 1 BEGIN

IF @x = 1 BEGIN SELECT

IF @x = 1 BEGIN SELECT *

IF @x = 1 BEGIN SELECT * FROM

IF @x = 1 BEGIN SELECT * FROM users

IF @x = 1 BEGIN SELECT * FROM users END

-- Typing WHILE
WHILE

WHILE @count

WHILE @count <

WHILE @count < 10

WHILE @count < 10 BEGIN

WHILE @count < 10 BEGIN SET

WHILE @count < 10 BEGIN SET @count

WHILE @count < 10 BEGIN SET @count =

WHILE @count < 10 BEGIN SET @count = @count

WHILE @count < 10 BEGIN SET @count = @count + 1

-- Typing TRY...CATCH
BEGIN

BEGIN TRY

BEGIN TRY INSERT

BEGIN TRY INSERT INTO

BEGIN TRY INSERT INTO users

BEGIN TRY INSERT INTO users VALUES

BEGIN TRY INSERT INTO users VALUES (1, 'test')

BEGIN TRY INSERT INTO users VALUES (1, 'test') END

BEGIN TRY INSERT INTO users VALUES (1, 'test') END TRY

BEGIN TRY INSERT INTO users VALUES (1, 'test') END TRY BEGIN

BEGIN TRY INSERT INTO users VALUES (1, 'test') END TRY BEGIN CATCH

BEGIN TRY INSERT INTO users VALUES (1, 'test') END TRY BEGIN CATCH SELECT

BEGIN TRY INSERT INTO users VALUES (1, 'test') END TRY BEGIN CATCH SELECT 'error'

BEGIN TRY INSERT INTO users VALUES (1, 'test') END TRY BEGIN CATCH SELECT 'error' END

BEGIN TRY INSERT INTO users VALUES (1, 'test') END TRY BEGIN CATCH SELECT 'error' END CATCH

-- Typing UPDATE
UPDATE

UPDATE users

UPDATE users SET

UPDATE users SET name

UPDATE users SET name =

UPDATE users SET name = 'test'

UPDATE users SET name = 'test' WHERE

UPDATE users SET name = 'test' WHERE id

UPDATE users SET name = 'test' WHERE id = 1

-- Typing DELETE
DELETE

DELETE FROM

DELETE FROM users

DELETE FROM users WHERE

DELETE FROM users WHERE id

DELETE FROM users WHERE id =

DELETE FROM users WHERE id = 1

-- Typing a JOIN
SELECT * FROM users

SELECT * FROM users JOIN

SELECT * FROM users JOIN orders

SELECT * FROM users JOIN orders ON

SELECT * FROM users JOIN orders ON users.id

SELECT * FROM users JOIN orders ON users.id =

SELECT * FROM users JOIN orders ON users.id = orders.user_id

SELECT * FROM users JOIN orders ON users.id = orders.user_id WHERE

-- Mid-expression
SELECT id, name, price * quantity

SELECT id, name, price * quantity AS

SELECT id, name, price * quantity AS total

SELECT id, name, price * quantity AS total FROM

SELECT id, name, price * quantity AS total FROM orders

SELECT id, name, price * quantity AS total FROM orders WHERE

SELECT id, name, price * quantity AS total FROM orders WHERE total >

-- Empty lines and whitespace only
-- (below are blank lines representing editor state during editing)
-- Typing a subquery
SELECT * FROM (SELECT

SELECT * FROM (SELECT id

SELECT * FROM (SELECT id FROM

SELECT * FROM (SELECT id FROM users)

SELECT * FROM (SELECT id FROM users) AS

SELECT * FROM (SELECT id FROM users) AS sub

-- SET variable mid-way
SET @counter =

SET @counter = @counter +

SET @counter = @counter + 1

-- Transaction mid-way
BEGIN TRANSACTION

BEGIN TRANSACTION my_txn

COMMIT

COMMIT TRANSACTION

COMMIT TRANSACTION my_txn

ROLLBACK

ROLLBACK TRANSACTION

-- EXEC mid-way
EXEC

EXEC sp_help

EXEC sp_helpdb

EXEC sp_who

EXEC @proc_name

EXEC @proc_name @param1

EXEC @proc_name @param1 = 'value'

-- RETURN mid-way
RETURN

RETURN 0

RETURN @status

-- ALTER TABLE mid-way
ALTER

ALTER TABLE

ALTER TABLE users

ALTER TABLE users ADD

ALTER TABLE users ADD email

ALTER TABLE users ADD email VARCHAR

ALTER TABLE users ADD email VARCHAR(255)

ALTER TABLE users DROP

ALTER TABLE users DROP COLUMN

ALTER TABLE users DROP COLUMN old_column

-- CREATE INDEX mid-way
CREATE

CREATE INDEX

CREATE INDEX idx_name

CREATE INDEX idx_name ON

CREATE INDEX idx_name ON users

CREATE INDEX idx_name ON users(email)

CREATE UNIQUE INDEX

CREATE UNIQUE INDEX idx_email

CREATE UNIQUE INDEX idx_email ON

CREATE UNIQUE INDEX idx_email ON users

CREATE UNIQUE INDEX idx_email ON users(email)
