-- UC-2: Migration SQL (T-SQL to be converted to MySQL/PostgreSQL)
-- Schema definition + data manipulation typical of migration work

-- Schema
CREATE TABLE customers (
    customer_id   INT          NOT NULL,
    customer_name VARCHAR(100) NOT NULL,
    email         VARCHAR(255) NULL,
    status        VARCHAR(20)  DEFAULT 'active',
    created_at    DATETIME     DEFAULT GETDATE(),
    CONSTRAINT pk_customers PRIMARY KEY (customer_id)
)

CREATE TABLE orders (
    order_id      INT          NOT NULL,
    customer_id   INT          NOT NULL,
    order_date    DATETIME     NOT NULL,
    order_total   NUMERIC(12,2) DEFAULT 0,
    status        VARCHAR(20)  DEFAULT 'pending',
    CONSTRAINT pk_orders PRIMARY KEY (order_id)
)

CREATE INDEX idx_orders_customer ON orders (customer_id)
CREATE INDEX idx_orders_date ON orders (order_date)

-- Data manipulation
INSERT INTO customers (customer_id, customer_name, email, status)
VALUES (1, 'Acme Corp', 'contact@acme.com', 'active')

INSERT INTO customers (customer_id, customer_name, email, status)
VALUES (2, 'Beta Inc', 'info@beta.com', 'active')

INSERT INTO customers (customer_id, customer_name, email, status)
VALUES (3, 'Gamma LLC', NULL, 'inactive')

UPDATE orders
SET status = 'completed'
WHERE order_date < '2024-01-01'
  AND status = 'pending'

DELETE FROM orders
WHERE status = 'cancelled'
  AND order_date < '2023-01-01'

-- Complex query with JOINs
SELECT
    c.customer_name,
    o.order_id,
    o.order_total,
    (SELECT COUNT(*)
     FROM orders o2
     WHERE o2.customer_id = c.customer_id) AS order_count
FROM customers c
INNER JOIN orders o ON c.customer_id = o.customer_id
WHERE o.order_total > 1000
  AND o.status IN ('completed', 'pending')
ORDER BY o.order_total DESC

-- Transaction example
BEGIN TRANSACTION
    INSERT INTO orders (order_id, customer_id, order_date, order_total)
    VALUES (100, 1, GETDATE(), 500.00)

    UPDATE customers
    SET status = 'vip'
    WHERE customer_id = 1
COMMIT TRANSACTION

-- Batch separator
GO

-- Variables and control flow
DECLARE @batch_size INT
SET @batch_size = 1000

WHILE EXISTS (SELECT 1 FROM orders WHERE status = 'pending')
BEGIN
    UPDATE TOP (@batch_size) orders
    SET status = 'processing'
    WHERE status = 'pending'

    WAITFOR DELAY '00:00:01'
END
