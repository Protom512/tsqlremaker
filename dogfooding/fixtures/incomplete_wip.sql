-- UC-3: Incomplete / work-in-progress SQL (real-time editing experience)

-- Incomplete SELECT (no FROM yet)
SELECT id, name, email

-- Typo in keyword
SELCT * FROM users

-- Incomplete CREATE TABLE
CREATE TABLE products (
    id INT,
    name VARCHAR(100

-- Missing semicolons and partial statements
DECLARE @x INT
SET @x =
SELECT @x

-- Incomplete IF block
IF @status = 'active'
    UPDATE users SET

-- Nested incomplete
CREATE PROCEDURE sp_test AS
BEGIN
    IF 1 = 1
    BEGIN
        SELECT * FROM

-- Empty lines







-- Only keywords
SELECT
FROM
WHERE

-- Comment-only
-- This is a work in progress
/* TODO: implement this later */

-- Temp tables
SELECT * INTO #temp_users FROM users WHERE status = 'active'
SELECT * FROM #temp_users

-- Global temp tables
CREATE TABLE ##global_cache (
    key VARCHAR(100),
    value TEXT
)
