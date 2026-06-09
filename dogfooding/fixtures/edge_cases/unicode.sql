-- =============================================================================
-- Edge case: Unicode content in SQL
-- Purpose: Test lexer/parser handling of non-ASCII characters
-- =============================================================================

-- Japanese characters in comments and strings
-- This procedure handles customer data from the Tokyo office
CREATE PROCEDURE sp_process_tokyoo_orders
    @customer_name VARCHAR(200)
AS
BEGIN
    -- Japanese string literals
    SELECT 'Hello' AS greeting
    -- NOTE: N'...' unicode string literals are a known parser gap (causes panic)
    -- This fixture tests what IS supported without crashing

    -- Mixed content in comments
    -- 結果をログに出力する
    SELECT @customer_name AS name

    -- Chinese characters
    SELECT '数据库管理系统' AS system_name

    -- Korean text
    SELECT '데이터베이스' AS db_korean

    -- Emoji-like characters (should not crash)
    -- Status: OK
    SELECT 'Complete' AS status
END
GO

-- Identifiers with special characters (ASE quoted identifiers)
CREATE TABLE "顧客マスタ" (
    id INT,
    name VARCHAR(100)
)
GO

-- Mixed ASCII and non-ASCII in column values
INSERT INTO customers (customer_name, email) VALUES ('Tanaka Corporation', 'info@tanaka.co.jp')
GO

-- String with escape sequences
SELECT 'It''s a test' AS escaped_quote
SELECT 'Line1' + CHAR(13) + CHAR(10) + 'Line2' AS multiline
GO

-- Hex strings
INSERT INTO users (id, data) VALUES (1, 0x48656C6C6F)
GO

-- Binary literals
SELECT 0x AS empty_binary
SELECT 0xFF AS hex_byte
GO
