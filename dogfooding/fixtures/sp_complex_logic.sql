-- =============================================================================
-- UC-1: Complex Stored Procedure with DECLARE/IF/WHILE/TRY...CATCH/Temp Tables
-- Purpose: Test parser accuracy on realistic multi-hundred-line stored procedures
-- Line count target: 500+
-- =============================================================================

CREATE PROCEDURE sp_process_monthly_billing
    @billing_period_start DATETIME,
    @billing_period_end DATETIME,
    @customer_filter VARCHAR(100) = NULL,
    @dry_run BIT = 0,
    @debug_mode BIT = 0,
    @max_errors INT = 100,
    @processed_count INT OUTPUT,
    @error_count INT OUTPUT,
    @total_amount NUMERIC(18, 2) OUTPUT
AS
BEGIN
    -- =========================================================================
    -- Phase 1: Initialization and Validation
    -- =========================================================================
    DECLARE @current_datetime DATETIME
    DECLARE @period_name VARCHAR(50)
    DECLARE @validation_ok BIT
    DECLARE @error_message VARCHAR(500)
    DECLARE @existing_count INT
    DECLARE @batch_id INT
    DECLARE @work_start DATETIME

    SET @current_datetime = GETDATE()
    SET @validation_ok = 1
    SET @processed_count = 0
    SET @error_count = 0
    SET @total_amount = 0

    -- Validate input parameters
    IF @billing_period_start IS NULL
    BEGIN
        SET @error_message = 'billing_period_start cannot be NULL'
        RAISERROR(@error_message, 16, 1)
        RETURN -1
    END

    IF @billing_period_end IS NULL
    BEGIN
        SET @error_message = 'billing_period_end cannot be NULL'
        RAISERROR(@error_message, 16, 1)
        RETURN -1
    END

    IF @billing_period_start >= @billing_period_end
    BEGIN
        SET @error_message = 'billing_period_start must be before billing_period_end'
        RAISERROR(@error_message, 16, 1)
        RETURN -1
    END

    -- Generate period name for logging
    SELECT @period_name = CONVERT(VARCHAR(7), @billing_period_start, 120)

    -- Check if billing already processed for this period
    SELECT @existing_count = COUNT(*)
    FROM billing_batches
    WHERE period_start = @billing_period_start
      AND period_end = @billing_period_end
      AND status = 'COMPLETED'

    IF @existing_count > 0
    BEGIN
        IF @debug_mode = 1
        BEGIN
            SELECT 'Billing already processed for period: ' + @period_name AS message
        END
        RETURN 0
    END

    -- =========================================================================
    -- Phase 2: Create temp tables for staging
    -- =========================================================================
    CREATE TABLE #eligible_customers (
        customer_id INT NOT NULL,
        customer_name VARCHAR(200) NOT NULL,
        account_type VARCHAR(50) NOT NULL,
        billing_tier VARCHAR(20) NOT NULL,
        primary_contact_id INT NULL,
        registration_date DATETIME NOT NULL,
        is_active BIT NOT NULL,
        region_code VARCHAR(10) NOT NULL
    )

    CREATE TABLE #billing_items (
        item_id INT IDENTITY,
        customer_id INT NOT NULL,
        service_code VARCHAR(50) NOT NULL,
        service_description VARCHAR(500) NOT NULL,
        quantity NUMERIC(18, 4) NOT NULL,
        unit_price NUMERIC(18, 4) NOT NULL,
        line_total NUMERIC(18, 2) NOT NULL,
        discount_pct NUMERIC(5, 2) NULL,
        tax_rate NUMERIC(5, 4) NULL,
        effective_date DATETIME NOT NULL,
        billing_category VARCHAR(30) NOT NULL
    )

    CREATE TABLE #error_log (
        error_id INT IDENTITY,
        customer_id INT NULL,
        error_number INT NOT NULL,
        error_severity INT NOT NULL,
        error_message VARCHAR(1000) NOT NULL,
        error_context VARCHAR(500) NULL,
        occurred_at DATETIME NOT NULL
    )

    CREATE TABLE #processing_summary (
        region_code VARCHAR(10) NOT NULL,
        account_type VARCHAR(50) NOT NULL,
        customer_count INT NOT NULL,
        total_charges NUMERIC(18, 2) NOT NULL,
        total_discounts NUMERIC(18, 2) NOT NULL,
        total_tax NUMERIC(18, 2) NOT NULL,
        net_amount NUMERIC(18, 2) NOT NULL
    )

    -- =========================================================================
    -- Phase 3: Load eligible customers
    -- =========================================================================
    SET @work_start = GETDATE()

    BEGIN TRY
        INSERT INTO #eligible_customers (
            customer_id, customer_name, account_type, billing_tier,
            primary_contact_id, registration_date, is_active, region_code
        )
        SELECT
            c.customer_id,
            c.customer_name,
            c.account_type,
            c.billing_tier,
            c.primary_contact_id,
            c.registration_date,
            c.is_active,
            c.region_code
        FROM customers c
        WHERE c.is_active = 1
          AND c.registration_date < @billing_period_end
          AND c.account_status = 'ACTIVE'
          AND (@customer_filter IS NULL OR c.customer_name LIKE '%' + @customer_filter + '%')

        IF @debug_mode = 1
        BEGIN
            SELECT 'Loaded ' + CONVERT(VARCHAR, @@ROWCOUNT) + ' eligible customers' AS debug_info
        END
    END TRY
    BEGIN CATCH
        SET @error_count = @error_count + 1
        INSERT INTO #error_log (customer_id, error_number, error_severity, error_message, error_context, occurred_at)
        VALUES (NULL, 1001, 16, 'Failed to load eligible customers: ' + ERROR_MESSAGE(), 'Phase 3 - Customer Loading', GETDATE())

        IF @error_count >= @max_errors
        BEGIN
            RAISERROR('Maximum error count exceeded during customer loading', 16, 1)
            RETURN -2
        END
    END CATCH

    -- =========================================================================
    -- Phase 4: Generate billing items per customer
    -- =========================================================================
    DECLARE @cust_id INT
    DECLARE @cust_name VARCHAR(200)
    DECLARE @cust_tier VARCHAR(20)
    DECLARE @cust_region VARCHAR(10)
    DECLARE @cust_type VARCHAR(50)
    DECLARE @item_quantity NUMERIC(18, 4)
    DECLARE @item_price NUMERIC(18, 4)
    DECLARE @line_total NUMERIC(18, 2)
    DECLARE @discount_pct NUMERIC(5, 2)
    DECLARE @tax_rate NUMERIC(5, 4)
    DECLARE @svc_code VARCHAR(50)
    DECLARE @svc_desc VARCHAR(500)
    DECLARE @billing_cat VARCHAR(30)

    DECLARE customer_cursor CURSOR FOR
        SELECT customer_id, customer_name, billing_tier, region_code, account_type
        FROM #eligible_customers
        ORDER BY customer_id

    OPEN customer_cursor
    FETCH NEXT FROM customer_cursor INTO @cust_id, @cust_name, @cust_tier, @cust_region, @cust_type

    WHILE @@FETCH_STATUS = 0
    BEGIN
        BEGIN TRY
            -- Base service charges
            IF @cust_tier = 'PREMIUM'
            BEGIN
                SET @svc_code = 'SVC_PREM_BASE'
                SET @svc_desc = 'Premium tier monthly base service'
                SET @item_price = 299.99
                SET @billing_cat = 'BASE_SERVICE'
            END
            ELSE IF @cust_tier = 'STANDARD'
            BEGIN
                SET @svc_code = 'SVC_STD_BASE'
                SET @svc_desc = 'Standard tier monthly base service'
                SET @item_price = 99.99
                SET @billing_cat = 'BASE_SERVICE'
            END
            ELSE IF @cust_tier = 'BASIC'
            BEGIN
                SET @svc_code = 'SVC_BASIC_BASE'
                SET @svc_desc = 'Basic tier monthly base service'
                SET @item_price = 29.99
                SET @billing_cat = 'BASE_SERVICE'
            END
            ELSE
            BEGIN
                -- Unknown tier, skip
                SET @svc_code = NULL
            END

            IF @svc_code IS NOT NULL
            BEGIN
                SET @item_quantity = 1
                SET @line_total = @item_price * @item_quantity

                -- Calculate discount based on tenure
                DECLARE @tenure_months INT
                SELECT @tenure_months = DATEDIFF(MONTH, registration_date, @billing_period_end)
                FROM #eligible_customers
                WHERE customer_id = @cust_id

                SET @discount_pct = 0
                IF @tenure_months >= 60
                    SET @discount_pct = 15.00
                ELSE IF @tenure_months >= 36
                    SET @discount_pct = 10.00
                ELSE IF @tenure_months >= 12
                    SET @discount_pct = 5.00

                IF @discount_pct > 0
                BEGIN
                    SET @line_total = @line_total * (1 - @discount_pct / 100)
                END

                -- Determine tax rate by region
                SET @tax_rate = 0
                IF @cust_region = 'JP'
                    SET @tax_rate = 0.1000
                ELSE IF @cust_region = 'US'
                    SET @tax_rate = 0.0000
                ELSE IF @cust_region = 'EU'
                    SET @tax_rate = 0.2100
                ELSE IF @cust_region = 'SG'
                    SET @tax_rate = 0.0900
                ELSE IF @cust_region = 'AU'
                    SET @tax_rate = 0.1000

                INSERT INTO #billing_items (
                    customer_id, service_code, service_description,
                    quantity, unit_price, line_total, discount_pct,
                    tax_rate, effective_date, billing_category
                )
                VALUES (
                    @cust_id, @svc_code, @svc_desc,
                    @item_quantity, @item_price, @line_total, @discount_pct,
                    @tax_rate, @billing_period_start, @billing_cat
                )
            END

            -- Usage-based charges from metering data
            DECLARE @usage_count INT
            DECLARE @usage_rate NUMERIC(18, 4)
            DECLARE @overage_count INT

            SELECT @usage_count = ISNULL(SUM(api_call_count), 0),
                   @usage_rate = ISNULL(base_rate, 0.0010)
            FROM customer_usage u
            LEFT JOIN rate_cards r ON u.service_plan = r.plan_code
            WHERE u.customer_id = @cust_id
              AND u.usage_date BETWEEN @billing_period_start AND @billing_period_end

            IF @usage_count > 0
            BEGIN
                -- Calculate overage for Premium tier (10000 included calls)
                SET @overage_count = 0
                IF @cust_tier = 'PREMIUM' AND @usage_count > 10000
                    SET @overage_count = @usage_count - 10000
                ELSE IF @cust_tier = 'STANDARD' AND @usage_count > 5000
                    SET @overage_count = @usage_count - 5000
                ELSE IF @cust_tier = 'BASIC' AND @usage_count > 1000
                    SET @overage_count = @usage_count - 1000

                IF @overage_count > 0
                BEGIN
                    DECLARE @overage_charge NUMERIC(18, 2)
                    SET @overage_charge = @overage_count * @usage_rate * 1.5

                    INSERT INTO #billing_items (
                        customer_id, service_code, service_description,
                        quantity, unit_price, line_total, discount_pct,
                        tax_rate, effective_date, billing_category
                    )
                    VALUES (
                        @cust_id, 'SVC_OVERAGE', 'API call overage charges',
                        @overage_count, @usage_rate * 1.5, @overage_charge, NULL,
                        @tax_rate, @billing_period_start, 'USAGE'
                    )
                END
            END

            -- Storage charges
            DECLARE @storage_gb NUMERIC(18, 4)
            DECLARE @storage_rate NUMERIC(18, 4)

            SELECT @storage_gb = ISNULL(SUM(storage_used_mb), 0) / 1024.0,
                   @storage_rate = 0.05
            FROM storage_metrics
            WHERE customer_id = @cust_id
              AND metric_date BETWEEN @billing_period_start AND @billing_period_end

            IF @storage_gb > 0
            BEGIN
                DECLARE @storage_charge NUMERIC(18, 2)
                SET @storage_charge = @storage_gb * @storage_rate

                INSERT INTO #billing_items (
                    customer_id, service_code, service_description,
                    quantity, unit_price, line_total, discount_pct,
                    tax_rate, effective_date, billing_category
                )
                VALUES (
                    @cust_id, 'SVC_STORAGE', 'Cloud storage usage',
                    @storage_gb, @storage_rate, @storage_charge, NULL,
                    @tax_rate, @billing_period_start, 'USAGE'
                )
            END

            SET @processed_count = @processed_count + 1

        END TRY
        BEGIN CATCH
            SET @error_count = @error_count + 1
            INSERT INTO #error_log (customer_id, error_number, error_severity, error_message, error_context, occurred_at)
            VALUES (@cust_id, ERROR_NUMBER(), ERROR_SEVERITY(),
                    'Customer billing failed: ' + ERROR_MESSAGE(),
                    'Phase 4 - Customer ' + ISNULL(CONVERT(VARCHAR, @cust_id), 'UNKNOWN'),
                    GETDATE())

            IF @error_count >= @max_errors
            BEGIN
                CLOSE customer_cursor
                DEALLOCATE customer_cursor
                RAISERROR('Maximum error count exceeded during billing generation', 16, 1)
                RETURN -3
            END
        END CATCH

        FETCH NEXT FROM customer_cursor INTO @cust_id, @cust_name, @cust_tier, @cust_region, @cust_type
    END

    CLOSE customer_cursor
    DEALLOCATE customer_cursor

    -- =========================================================================
    -- Phase 5: Calculate totals and populate summary
    -- =========================================================================
    BEGIN TRY
        INSERT INTO #processing_summary (
            region_code, account_type, customer_count,
            total_charges, total_discounts, total_tax, net_amount
        )
        SELECT
            ec.region_code,
            ec.account_type,
            COUNT(DISTINCT bi.customer_id),
            SUM(bi.line_total),
            SUM(CASE WHEN bi.discount_pct > 0 THEN bi.line_total * bi.discount_pct / 100 ELSE 0 END),
            SUM(CASE WHEN bi.tax_rate > 0 THEN bi.line_total * bi.tax_rate ELSE 0 END),
            SUM(bi.line_total) +
                SUM(CASE WHEN bi.tax_rate > 0 THEN bi.line_total * bi.tax_rate ELSE 0 END) -
                SUM(CASE WHEN bi.discount_pct > 0 THEN bi.line_total * bi.discount_pct / 100 ELSE 0 END)
        FROM #billing_items bi
        INNER JOIN #eligible_customers ec ON bi.customer_id = ec.customer_id
        GROUP BY ec.region_code, ec.account_type

        -- Calculate grand total
        SELECT @total_amount = SUM(net_amount)
        FROM #processing_summary

        IF @debug_mode = 1
        BEGIN
            SELECT 'Grand total: ' + CONVERT(VARCHAR, @total_amount) AS debug_info
        END
    END TRY
    BEGIN CATCH
        SET @error_count = @error_count + 1
        INSERT INTO #error_log (customer_id, error_number, error_severity, error_message, error_context, occurred_at)
        VALUES (NULL, 1002, 16, 'Summary calculation failed: ' + ERROR_MESSAGE(), 'Phase 5 - Summary', GETDATE())
    END CATCH

    -- =========================================================================
    -- Phase 6: Persist results (unless dry run)
    -- =========================================================================
    IF @dry_run = 0
    BEGIN
        BEGIN TRANSACTION

        BEGIN TRY
            -- Create billing batch record
            INSERT INTO billing_batches (
                period_start, period_end, period_name, status,
                total_customers, total_amount, created_by, created_at
            )
            VALUES (
                @billing_period_start, @billing_period_end, @period_name, 'PROCESSING',
                @processed_count, @total_amount, SYSTEM_USER, GETDATE()
            )

            SET @batch_id = @@IDENTITY

            -- Persist billing items
            INSERT INTO billing_items_archive (
                batch_id, customer_id, service_code, service_description,
                quantity, unit_price, line_total, discount_pct,
                tax_rate, effective_date, billing_category, created_at
            )
            SELECT
                @batch_id, customer_id, service_code, service_description,
                quantity, unit_price, line_total, discount_pct,
                tax_rate, effective_date, billing_category, GETDATE()
            FROM #billing_items

            -- Update customer balances
            UPDATE c
            SET c.outstanding_balance = c.outstanding_balance + bi.line_total,
                c.last_billed_date = @billing_period_end,
                c.updated_at = GETDATE()
            FROM customers c
            INNER JOIN (
                SELECT customer_id, SUM(line_total) AS line_total
                FROM #billing_items
                GROUP BY customer_id
            ) bi ON c.customer_id = bi.customer_id

            -- Mark batch as completed
            UPDATE billing_batches
            SET status = 'COMPLETED',
                completed_at = GETDATE()
            WHERE batch_id = @batch_id

            COMMIT TRANSACTION
        END TRY
        BEGIN CATCH
            ROLLBACK TRANSACTION

            SET @error_count = @error_count + 1
            INSERT INTO #error_log (customer_id, error_number, error_severity, error_message, error_context, occurred_at)
            VALUES (NULL, 1003, 16, 'Transaction failed: ' + ERROR_MESSAGE(), 'Phase 6 - Persistence', GETDATE())

            IF @batch_id IS NOT NULL
            BEGIN
                UPDATE billing_batches
                SET status = 'FAILED',
                    error_message = ERROR_MESSAGE(),
                    completed_at = GETDATE()
                WHERE batch_id = @batch_id
            END
        END CATCH
    END
    ELSE
    BEGIN
        -- Dry run mode: just output what would be billed
        SELECT
            bi.customer_id,
            ec.customer_name,
            bi.service_code,
            bi.service_description,
            bi.quantity,
            bi.unit_price,
            bi.line_total,
            bi.discount_pct,
            bi.tax_rate,
            bi.billing_category
        FROM #billing_items bi
        INNER JOIN #eligible_customers ec ON bi.customer_id = ec.customer_id
        ORDER BY bi.customer_id, bi.item_id
    END

    -- =========================================================================
    -- Phase 7: Output results
    -- =========================================================================
    SELECT @processed_count AS processed_count,
           @error_count AS error_count,
           @total_amount AS total_amount,
           @period_name AS period_name

    -- Summary by region
    SELECT region_code, account_type, customer_count,
           total_charges, total_discounts, total_tax, net_amount
    FROM #processing_summary
    ORDER BY region_code, account_type

    -- Error report
    IF @error_count > 0
    BEGIN
        SELECT error_id, customer_id, error_number, error_severity,
               error_message, error_context, occurred_at
        FROM #error_log
        ORDER BY error_id
    END

    -- =========================================================================
    -- Phase 8: Cleanup
    -- =========================================================================
    DROP TABLE #eligible_customers
    DROP TABLE #billing_items
    DROP TABLE #error_log
    DROP TABLE #processing_summary

    RETURN 0
END
GO

-- =============================================================================
-- Additional procedure: Void a billing batch
-- =============================================================================
CREATE PROCEDURE sp_void_billing_batch
    @batch_id INT,
    @reason VARCHAR(500),
    @voided_by VARCHAR(100) = NULL
AS
BEGIN
    DECLARE @batch_status VARCHAR(20)
    DECLARE @batch_period_start DATETIME
    DECLARE @batch_period_end DATETIME
    DECLARE @batch_total NUMERIC(18, 2)

    -- Validate batch exists and is in a voidable state
    SELECT @batch_status = status,
           @batch_period_start = period_start,
           @batch_period_end = period_end,
           @batch_total = total_amount
    FROM billing_batches
    WHERE batch_id = @batch_id

    IF @batch_status IS NULL
    BEGIN
        RAISERROR('Batch not found: %d', 16, 1, @batch_id)
        RETURN -1
    END

    IF @batch_status = 'VOIDED'
    BEGIN
        RAISERROR('Batch already voided: %d', 16, 1, @batch_id)
        RETURN -2
    END

    IF @batch_status = 'FAILED'
    BEGIN
        RAISERROR('Cannot void a failed batch: %d', 16, 1, @batch_id)
        RETURN -3
    END

    BEGIN TRANSACTION

    BEGIN TRY
        -- Reverse customer balance updates
        UPDATE c
        SET c.outstanding_balance = c.outstanding_balance - bi.line_total,
            c.updated_at = GETDATE()
        FROM customers c
        INNER JOIN billing_items_archive bi ON c.customer_id = bi.customer_id
        WHERE bi.batch_id = @batch_id

        -- Mark batch as voided
        UPDATE billing_batches
        SET status = 'VOIDED',
            voided_at = GETDATE(),
            void_reason = @reason,
            voided_by = ISNULL(@voided_by, SYSTEM_USER)
        WHERE batch_id = @batch_id

        -- Log the void action
        INSERT INTO audit_log (action_type, target_table, target_id, action_details, performed_by, performed_at)
        VALUES ('VOID_BATCH', 'billing_batches', @batch_id,
                'Voided batch for period ' + CONVERT(VARCHAR, @batch_period_start, 120) +
                ' to ' + CONVERT(VARCHAR, @batch_period_end, 120) +
                '. Amount reversed: ' + CONVERT(VARCHAR, @batch_total) +
                '. Reason: ' + @reason,
                ISNULL(@voided_by, SYSTEM_USER), GETDATE())

        COMMIT TRANSACTION

        RETURN 0
    END TRY
    BEGIN CATCH
        ROLLBACK TRANSACTION
        RAISERROR('Failed to void batch: %s', 16, 1, ERROR_MESSAGE())
        RETURN -99
    END CATCH
END
GO

-- =============================================================================
-- View: Customer billing summary
-- =============================================================================
CREATE VIEW v_customer_billing_summary AS
SELECT
    c.customer_id,
    c.customer_name,
    c.account_type,
    c.billing_tier,
    c.region_code,
    c.outstanding_balance,
    COUNT(bi.item_id) AS total_invoice_items,
    SUM(bi.line_total) AS total_billed,
    MAX(bb.period_end) AS last_billing_period,
    SUM(CASE WHEN bi.billing_category = 'BASE_SERVICE' THEN bi.line_total ELSE 0 END) AS base_service_total,
    SUM(CASE WHEN bi.billing_category = 'USAGE' THEN bi.line_total ELSE 0 END) AS usage_total
FROM customers c
LEFT JOIN billing_items_archive bi ON c.customer_id = bi.customer_id
LEFT JOIN billing_batches bb ON bi.batch_id = bb.batch_id AND bb.status = 'COMPLETED'
WHERE c.is_active = 1
GROUP BY c.customer_id, c.customer_name, c.account_type, c.billing_tier,
         c.region_code, c.outstanding_balance
GO

-- =============================================================================
-- Trigger: Validate billing item before insert
-- =============================================================================
CREATE TRIGGER trg_validate_billing_item
ON billing_items_archive
FOR INSERT
AS
BEGIN
    DECLARE @invalid_count INT

    SELECT @invalid_count = COUNT(*)
    FROM inserted i
    WHERE i.line_total <= 0
       OR i.quantity <= 0
       OR i.unit_price < 0
       OR i.customer_id IS NULL
       OR i.service_code IS NULL

    IF @invalid_count > 0
    BEGIN
        RAISERROR('Invalid billing items detected: negative amounts, zero quantities, or missing required fields', 16, 1)
    END
END
GO
