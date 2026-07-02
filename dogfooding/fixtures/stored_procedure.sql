-- UC-1: Everyday stored procedure development
-- A realistic SAP ASE stored procedure with multiple statement types

CREATE PROCEDURE sp_get_customer_orders
    @customer_id INT,
    @start_date DATETIME = NULL,
    @status VARCHAR(20) = 'active'
AS
BEGIN
    DECLARE @total_orders INT
    DECLARE @total_amount NUMERIC(12,2)
    DECLARE @avg_amount FLOAT

    -- Validate input
    IF @customer_id IS NULL
    BEGIN
        RAISERROR 15000 'Customer ID is required'
        RETURN -1
    END

    -- Count orders
    SELECT @total_orders = COUNT(*)
    FROM orders
    WHERE customer_id = @customer_id

    -- Sum amounts
    SELECT @total_amount = ISNULL(SUM(order_total), 0)
    FROM orders
    WHERE customer_id = @customer_id
      AND order_date >= ISNULL(@start_date, '1900-01-01')

    -- Calculate average
    IF @total_orders > 0
    BEGIN
        SET @avg_amount = @total_amount / @total_orders
        PRINT 'Average order: ' + CONVERT(VARCHAR, @avg_amount)
    END
    ELSE
    BEGIN
        SET @avg_amount = 0
        PRINT 'No orders found'
    END

    -- Return results
    SELECT
        o.order_id,
        o.order_date,
        o.order_total,
        o.status
    FROM orders o
    WHERE o.customer_id = @customer_id
      AND o.status = @status
    ORDER BY o.order_date DESC

    -- Error handling
    BEGIN TRY
        INSERT INTO order_audit (customer_id, checked_at, order_count)
        VALUES (@customer_id, GETDATE(), @total_orders)
    END TRY
    BEGIN CATCH
        RAISERROR 15001 'Audit logging failed'
    END CATCH

    RETURN 0
END
GO

-- Trigger example
CREATE TRIGGER tr_order_insert
ON orders FOR INSERT AS
BEGIN
    DECLARE @new_id INT
    SELECT @new_id = order_id FROM inserted

    INSERT INTO order_log (order_id, action, created_at)
    VALUES (@new_id, 'INSERT', GETDATE())
END
GO

-- View example
CREATE VIEW v_customer_summary AS
SELECT
    c.customer_id,
    c.customer_name,
    COUNT(o.order_id) AS order_count
FROM customers c
LEFT JOIN orders o ON c.customer_id = o.customer_id
GROUP BY c.customer_id, c.customer_name
