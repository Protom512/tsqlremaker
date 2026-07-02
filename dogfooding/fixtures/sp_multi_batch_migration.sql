-- =============================================================================
-- UC-2: Multi-batch Migration Script
-- Purpose: Test parser handling of GO batches, 1000+ lines of mixed DDL/DML
-- Target: SAP ASE to MySQL migration scenario
-- =============================================================================

-- =============================================================================
-- Batch 1: Schema version tracking table
-- =============================================================================
IF NOT EXISTS (SELECT 1 FROM sysobjects WHERE name = 'schema_version' AND type = 'U')
BEGIN
    CREATE TABLE schema_version (
        version_id INT IDENTITY,
        version_number VARCHAR(20) NOT NULL,
        description VARCHAR(500) NOT NULL,
        applied_at DATETIME NOT NULL,
        applied_by VARCHAR(100) NOT NULL,
        execution_time_ms INT NULL,
        checksum VARCHAR(64) NULL
    )

    CREATE UNIQUE INDEX idx_schema_version_number ON schema_version(version_number)
END
GO

-- =============================================================================
-- Batch 2: Core lookup tables
-- =============================================================================
CREATE TABLE lookup_regions (
    region_id INT NOT NULL,
    region_code VARCHAR(10) NOT NULL,
    region_name VARCHAR(100) NOT NULL,
    continent VARCHAR(50) NOT NULL,
    is_active BIT NOT NULL,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NULL
)
GO

INSERT INTO lookup_regions (region_id, region_code, region_name, continent, is_active, created_at) VALUES (1, 'JP', 'Japan', 'Asia', 1, GETDATE())
INSERT INTO lookup_regions (region_id, region_code, region_name, continent, is_active, created_at) VALUES (2, 'US', 'United States', 'North America', 1, GETDATE())
INSERT INTO lookup_regions (region_id, region_code, region_name, continent, is_active, created_at) VALUES (3, 'GB', 'United Kingdom', 'Europe', 1, GETDATE())
INSERT INTO lookup_regions (region_id, region_code, region_name, continent, is_active, created_at) VALUES (4, 'DE', 'Germany', 'Europe', 1, GETDATE())
INSERT INTO lookup_regions (region_id, region_code, region_name, continent, is_active, created_at) VALUES (5, 'FR', 'France', 'Europe', 1, GETDATE())
INSERT INTO lookup_regions (region_id, region_code, region_name, continent, is_active, created_at) VALUES (6, 'SG', 'Singapore', 'Asia', 1, GETDATE())
INSERT INTO lookup_regions (region_id, region_code, region_name, continent, is_active, created_at) VALUES (7, 'AU', 'Australia', 'Oceania', 1, GETDATE())
INSERT INTO lookup_regions (region_id, region_code, region_name, continent, is_active, created_at) VALUES (8, 'CA', 'Canada', 'North America', 1, GETDATE())
INSERT INTO lookup_regions (region_id, region_code, region_name, continent, is_active, created_at) VALUES (9, 'KR', 'South Korea', 'Asia', 1, GETDATE())
INSERT INTO lookup_regions (region_id, region_code, region_name, continent, is_active, created_at) VALUES (10, 'IN', 'India', 'Asia', 1, GETDATE())
GO

-- =============================================================================
-- Batch 3: Product catalog tables
-- =============================================================================
CREATE TABLE product_categories (
    category_id INT IDENTITY,
    category_name VARCHAR(100) NOT NULL,
    parent_category_id INT NULL,
    description VARCHAR(500) NULL,
    sort_order INT NOT NULL,
    is_active BIT NOT NULL,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NULL,
    CONSTRAINT pk_product_categories PRIMARY KEY (category_id)
)
GO

CREATE TABLE products (
    product_id INT IDENTITY,
    sku VARCHAR(50) NOT NULL,
    product_name VARCHAR(200) NOT NULL,
    category_id INT NOT NULL,
    base_price NUMERIC(18, 4) NOT NULL,
    sale_price NUMERIC(18, 4) NULL,
    cost_price NUMERIC(18, 4) NULL,
    weight_kg NUMERIC(10, 4) NULL,
    is_digital BIT NOT NULL,
    is_active BIT NOT NULL,
    launch_date DATETIME NULL,
    discontinuation_date DATETIME NULL,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NULL,
    CONSTRAINT pk_products PRIMARY KEY (product_id),
    CONSTRAINT fk_products_category FOREIGN KEY (category_id) REFERENCES product_categories(category_id),
    CONSTRAINT uq_products_sku UNIQUE (sku)
)
GO

CREATE INDEX idx_products_category ON products(category_id)
GO

CREATE UNIQUE INDEX idx_products_sku ON products(sku)
GO

CREATE TABLE product_attributes (
    attribute_id INT IDENTITY,
    product_id INT NOT NULL,
    attribute_name VARCHAR(100) NOT NULL,
    attribute_value VARCHAR(500) NOT NULL,
    attribute_type VARCHAR(20) NOT NULL,
    CONSTRAINT pk_product_attributes PRIMARY KEY (attribute_id),
    CONSTRAINT fk_product_attributes_product FOREIGN KEY (product_id) REFERENCES products(product_id)
)
GO

CREATE INDEX idx_product_attributes_product ON product_attributes(product_id)
GO

-- =============================================================================
-- Batch 4: Seed product data
-- =============================================================================
INSERT INTO product_categories (category_name, parent_category_id, description, sort_order, is_active, created_at)
VALUES ('Electronics', NULL, 'Consumer electronics and gadgets', 1, 1, GETDATE())

INSERT INTO product_categories (category_name, parent_category_id, description, sort_order, is_active, created_at)
VALUES ('Computers', 1, 'Desktop and laptop computers', 1, 1, GETDATE())

INSERT INTO product_categories (category_name, parent_category_id, description, sort_order, is_active, created_at)
VALUES ('Mobile Devices', 1, 'Smartphones and tablets', 2, 1, GETDATE())

INSERT INTO product_categories (category_name, parent_category_id, description, sort_order, is_active, created_at)
VALUES ('Software', NULL, 'Software licenses and subscriptions', 2, 1, GETDATE())

INSERT INTO product_categories (category_name, parent_category_id, description, sort_order, is_active, created_at)
VALUES ('Cloud Services', NULL, 'Cloud computing and storage services', 3, 1, GETDATE())

INSERT INTO product_categories (category_name, parent_category_id, description, sort_order, is_active, created_at)
VALUES ('Networking', NULL, 'Network equipment and accessories', 4, 1, GETDATE())
GO

INSERT INTO products (sku, product_name, category_id, base_price, cost_price, weight_kg, is_digital, is_active, launch_date, created_at)
VALUES ('ELEC-LAP-001', 'ProBook 15 Business Laptop', 2, 1299.99, 850.00, 2.10, 0, 1, GETDATE(), GETDATE())

INSERT INTO products (sku, product_name, category_id, base_price, cost_price, weight_kg, is_digital, is_active, launch_date, created_at)
VALUES ('ELEC-LAP-002', 'UltraBook 14 Thin Laptop', 2, 1599.99, 1050.00, 1.40, 0, 1, GETDATE(), GETDATE())

INSERT INTO products (sku, product_name, category_id, base_price, cost_price, weight_kg, is_digital, is_active, launch_date, created_at)
VALUES ('ELEC-PHN-001', 'Galaxy Pro Smartphone', 3, 899.99, 550.00, 0.18, 0, 1, GETDATE(), GETDATE())

INSERT INTO products (sku, product_name, category_id, base_price, cost_price, weight_kg, is_digital, is_active, launch_date, created_at)
VALUES ('ELEC-PHN-002', 'iPhone 16 Pro', 3, 1199.99, 750.00, 0.21, 0, 1, GETDATE(), GETDATE())

INSERT INTO products (sku, product_name, category_id, base_price, cost_price, weight_kg, is_digital, is_active, launch_date, created_at)
VALUES ('SW-OFC-001', 'Office Suite Professional', 4, 299.99, 50.00, 0.00, 1, 1, GETDATE(), GETDATE())

INSERT INTO products (sku, product_name, category_id, base_price, cost_price, weight_kg, is_digital, is_active, launch_date, created_at)
VALUES ('SW-DEV-001', 'Developer IDE Enterprise', 4, 599.99, 100.00, 0.00, 1, 1, GETDATE(), GETDATE())

INSERT INTO products (sku, product_name, category_id, base_price, cost_price, weight_kg, is_digital, is_active, launch_date, created_at)
VALUES ('CLD-STR-001', 'Cloud Storage 100GB Monthly', 5, 9.99, 2.00, 0.00, 1, 1, GETDATE(), GETDATE())

INSERT INTO products (sku, product_name, category_id, base_price, cost_price, weight_kg, is_digital, is_active, launch_date, created_at)
VALUES ('CLD-CMP-001', 'Cloud Compute Instance Small', 5, 49.99, 15.00, 0.00, 1, 1, GETDATE(), GETDATE())

INSERT INTO products (sku, product_name, category_id, base_price, cost_price, weight_kg, is_digital, is_active, launch_date, created_at)
VALUES ('NET-RTR-001', 'Enterprise WiFi Router', 6, 349.99, 180.00, 0.65, 0, 1, GETDATE(), GETDATE())

INSERT INTO products (sku, product_name, category_id, base_price, cost_price, weight_kg, is_digital, is_active, launch_date, created_at)
VALUES ('NET-SWT-001', 'Managed 48-Port Switch', 6, 1299.99, 700.00, 4.50, 0, 1, GETDATE(), GETDATE())
GO

-- =============================================================================
-- Batch 5: Customer tables
-- =============================================================================
CREATE TABLE customers (
    customer_id INT IDENTITY,
    customer_code VARCHAR(20) NOT NULL,
    customer_name VARCHAR(200) NOT NULL,
    legal_name VARCHAR(300) NULL,
    account_type VARCHAR(50) NOT NULL,
    billing_tier VARCHAR(20) NOT NULL,
    account_status VARCHAR(20) NOT NULL,
    primary_contact_id INT NULL,
    region_code VARCHAR(10) NOT NULL,
    email VARCHAR(255) NULL,
    phone VARCHAR(50) NULL,
    tax_id VARCHAR(50) NULL,
    credit_limit NUMERIC(18, 2) NULL,
    outstanding_balance NUMERIC(18, 2) NOT NULL,
    registration_date DATETIME NOT NULL,
    last_order_date DATETIME NULL,
    last_billed_date DATETIME NULL,
    notes TEXT NULL,
    is_active BIT NOT NULL,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NULL,
    CONSTRAINT pk_customers PRIMARY KEY (customer_id),
    CONSTRAINT uq_customers_code UNIQUE (customer_code)
)
GO

CREATE INDEX idx_customers_region ON customers(region_code)
GO

CREATE INDEX idx_customers_type ON customers(account_type)
GO

CREATE INDEX idx_customers_status ON customers(account_status)
GO

CREATE TABLE customer_contacts (
    contact_id INT IDENTITY,
    customer_id INT NOT NULL,
    contact_type VARCHAR(30) NOT NULL,
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    email VARCHAR(255) NOT NULL,
    phone VARCHAR(50) NULL,
    is_primary BIT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT pk_customer_contacts PRIMARY KEY (contact_id),
    CONSTRAINT fk_customer_contacts_customer FOREIGN KEY (customer_id) REFERENCES customers(customer_id)
)
GO

CREATE INDEX idx_customer_contacts_customer ON customer_contacts(customer_id)
GO

-- =============================================================================
-- Batch 6: Customer addresses
-- =============================================================================
CREATE TABLE customer_addresses (
    address_id INT IDENTITY,
    customer_id INT NOT NULL,
    address_type VARCHAR(30) NOT NULL,
    address_line1 VARCHAR(200) NOT NULL,
    address_line2 VARCHAR(200) NULL,
    city VARCHAR(100) NOT NULL,
    state_province VARCHAR(100) NULL,
    postal_code VARCHAR(20) NOT NULL,
    country_code VARCHAR(10) NOT NULL,
    is_default BIT NOT NULL,
    CONSTRAINT pk_customer_addresses PRIMARY KEY (address_id),
    CONSTRAINT fk_customer_addresses_customer FOREIGN KEY (customer_id) REFERENCES customers(customer_id)
)
GO

CREATE INDEX idx_customer_addresses_customer ON customer_addresses(customer_id)
GO

-- =============================================================================
-- Batch 7: Seed customer data
-- =============================================================================
INSERT INTO customers (customer_code, customer_name, legal_name, account_type, billing_tier, account_status, region_code, email, phone, credit_limit, outstanding_balance, registration_date, is_active, created_at)
VALUES ('CUST-0001', 'Acme Corporation', 'Acme Corporation KK', 'ENTERPRISE', 'PREMIUM', 'ACTIVE', 'JP', 'billing@acme.co.jp', '+81-3-1234-5678', 500000.00, 0, GETDATE(), 1, GETDATE())

INSERT INTO customers (customer_code, customer_name, legal_name, account_type, billing_tier, account_status, region_code, email, phone, credit_limit, outstanding_balance, registration_date, is_active, created_at)
VALUES ('CUST-0002', 'TechStart Inc', 'TechStart Inc', 'STARTUP', 'STANDARD', 'ACTIVE', 'US', 'ap@techstart.com', '+1-650-555-0100', 50000.00, 0, GETDATE(), 1, GETDATE())

INSERT INTO customers (customer_code, customer_name, legal_name, account_type, billing_tier, account_status, region_code, email, phone, credit_limit, outstanding_balance, registration_date, is_active, created_at)
VALUES ('CUST-0003', 'Global Traders Ltd', 'Global Traders Limited', 'ENTERPRISE', 'PREMIUM', 'ACTIVE', 'GB', 'finance@globaltraders.co.uk', '+44-20-7946-0958', 1000000.00, 0, GETDATE(), 1, GETDATE())

INSERT INTO customers (customer_code, customer_name, legal_name, account_type, billing_tier, account_status, region_code, email, phone, credit_limit, outstanding_balance, registration_date, is_active, created_at)
VALUES ('CUST-0004', 'DataWorks GmbH', 'DataWorks GmbH', 'BUSINESS', 'STANDARD', 'ACTIVE', 'DE', 'info@dataworks.de', '+49-30-1234-5678', 200000.00, 0, GETDATE(), 1, GETDATE())

INSERT INTO customers (customer_code, customer_name, legal_name, account_type, billing_tier, account_status, region_code, email, phone, credit_limit, outstanding_balance, registration_date, is_active, created_at)
VALUES ('CUST-0005', 'CloudNine Pte Ltd', 'CloudNine Pte Ltd', 'STARTUP', 'BASIC', 'ACTIVE', 'SG', 'hello@cloudnine.sg', '+65-6789-0123', 25000.00, 0, GETDATE(), 1, GETDATE())

INSERT INTO customers (customer_code, customer_name, legal_name, account_type, billing_tier, account_status, region_code, email, phone, credit_limit, outstanding_balance, registration_date, is_active, created_at)
VALUES ('CUST-0006', 'Outback Solutions', 'Outback Solutions Pty Ltd', 'BUSINESS', 'STANDARD', 'ACTIVE', 'AU', 'accounts@outbacksol.com.au', '+61-2-9876-5432', 150000.00, 0, GETDATE(), 1, GETDATE())

INSERT INTO customers (customer_code, customer_name, legal_name, account_type, billing_tier, account_status, region_code, email, phone, credit_limit, outstanding_balance, registration_date, is_active, created_at)
VALUES ('CUST-0007', 'Maple Tech Corp', 'Maple Technology Corporation', 'ENTERPRISE', 'PREMIUM', 'ACTIVE', 'CA', 'billing@mapletech.ca', '+1-416-555-0200', 750000.00, 0, GETDATE(), 1, GETDATE())

INSERT INTO customers (customer_code, customer_name, legal_name, account_type, billing_tier, account_status, region_code, email, phone, credit_limit, outstanding_balance, registration_date, is_active, created_at)
VALUES ('CUST-0008', 'Seoul Digital', 'Seoul Digital Co Ltd', 'STARTUP', 'BASIC', 'ACTIVE', 'KR', 'info@seouldigital.kr', '+82-2-1234-5678', 30000.00, 0, GETDATE(), 1, GETDATE())

INSERT INTO customers (customer_code, customer_name, legal_name, account_type, billing_tier, account_status, region_code, email, phone, credit_limit, outstanding_balance, registration_date, is_active, created_at)
VALUES ('CUST-0009', 'Mumbai Systems', 'Mumbai Systems Private Limited', 'BUSINESS', 'STANDARD', 'ACTIVE', 'IN', 'finance@mumbaisystems.in', '+91-22-1234-5678', 100000.00, 0, GETDATE(), 1, GETDATE())

INSERT INTO customers (customer_code, customer_name, legal_name, account_type, billing_tier, account_status, region_code, email, phone, credit_limit, outstanding_balance, registration_date, is_active, created_at)
VALUES ('CUST-0010', 'Paris Digital SARL', 'Paris Digital SARL', 'ENTERPRISE', 'PREMIUM', 'ACTIVE', 'FR', 'comptabilite@parisdigital.fr', '+33-1-23-45-67-89', 600000.00, 0, GETDATE(), 1, GETDATE())
GO

-- =============================================================================
-- Batch 8: Order management tables
-- =============================================================================
CREATE TABLE orders (
    order_id INT IDENTITY,
    order_number VARCHAR(30) NOT NULL,
    customer_id INT NOT NULL,
    order_status VARCHAR(20) NOT NULL,
    order_date DATETIME NOT NULL,
    required_date DATETIME NULL,
    shipped_date DATETIME NULL,
    subtotal NUMERIC(18, 2) NOT NULL,
    tax_amount NUMERIC(18, 2) NOT NULL,
    discount_amount NUMERIC(18, 2) NOT NULL,
    shipping_amount NUMERIC(18, 2) NOT NULL,
    total_amount NUMERIC(18, 2) NOT NULL,
    payment_method VARCHAR(30) NULL,
    payment_status VARCHAR(20) NOT NULL,
    notes TEXT NULL,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NULL,
    CONSTRAINT pk_orders PRIMARY KEY (order_id),
    CONSTRAINT fk_orders_customer FOREIGN KEY (customer_id) REFERENCES customers(customer_id),
    CONSTRAINT uq_orders_number UNIQUE (order_number)
)
GO

CREATE INDEX idx_orders_customer ON orders(customer_id)
GO

CREATE INDEX idx_orders_status ON orders(order_status)
GO

CREATE INDEX idx_orders_date ON orders(order_date)
GO

CREATE TABLE order_items (
    order_item_id INT IDENTITY,
    order_id INT NOT NULL,
    product_id INT NOT NULL,
    sku VARCHAR(50) NOT NULL,
    product_name VARCHAR(200) NOT NULL,
    quantity INT NOT NULL,
    unit_price NUMERIC(18, 4) NOT NULL,
    discount_pct NUMERIC(5, 2) NOT NULL,
    line_total NUMERIC(18, 2) NOT NULL,
    CONSTRAINT pk_order_items PRIMARY KEY (order_item_id),
    CONSTRAINT fk_order_items_order FOREIGN KEY (order_id) REFERENCES orders(order_id),
    CONSTRAINT fk_order_items_product FOREIGN KEY (product_id) REFERENCES products(product_id)
)
GO

CREATE INDEX idx_order_items_order ON order_items(order_id)
GO

-- =============================================================================
-- Batch 9: Inventory tables
-- =============================================================================
CREATE TABLE inventory (
    inventory_id INT IDENTITY,
    product_id INT NOT NULL,
    warehouse_code VARCHAR(20) NOT NULL,
    quantity_on_hand INT NOT NULL,
    quantity_reserved INT NOT NULL,
    quantity_available INT NOT NULL,
    reorder_point INT NOT NULL,
    reorder_quantity INT NOT NULL,
    last_received_date DATETIME NULL,
    last_counted_date DATETIME NULL,
    unit_cost NUMERIC(18, 4) NOT NULL,
    CONSTRAINT pk_inventory PRIMARY KEY (inventory_id),
    CONSTRAINT fk_inventory_product FOREIGN KEY (product_id) REFERENCES products(product_id)
)
GO

CREATE INDEX idx_inventory_product ON inventory(product_id)
GO

CREATE INDEX idx_inventory_warehouse ON inventory(warehouse_code)
GO

-- =============================================================================
-- Batch 10: Seed inventory
-- =============================================================================
INSERT INTO inventory (product_id, warehouse_code, quantity_on_hand, quantity_reserved, quantity_available, reorder_point, reorder_quantity, last_received_date, last_counted_date, unit_cost)
SELECT product_id, 'WH-TKO', 1000, 0, 1000, 200, 500, GETDATE(), GETDATE(), cost_price
FROM products
WHERE is_digital = 0

INSERT INTO inventory (product_id, warehouse_code, quantity_on_hand, quantity_reserved, quantity_available, reorder_point, reorder_quantity, last_received_date, last_counted_date, unit_cost)
SELECT product_id, 'WH-SFO', 500, 0, 500, 100, 250, GETDATE(), GETDATE(), cost_price
FROM products
WHERE is_digital = 0
GO

-- =============================================================================
-- Batch 11: Audit and logging tables
-- =============================================================================
CREATE TABLE audit_log (
    audit_id INT IDENTITY,
    action_type VARCHAR(50) NOT NULL,
    target_table VARCHAR(100) NOT NULL,
    target_id INT NULL,
    action_details TEXT NULL,
    old_values TEXT NULL,
    new_values TEXT NULL,
    performed_by VARCHAR(100) NOT NULL,
    performed_at DATETIME NOT NULL,
    session_id VARCHAR(50) NULL,
    ip_address VARCHAR(45) NULL,
    CONSTRAINT pk_audit_log PRIMARY KEY (audit_id)
)
GO

CREATE INDEX idx_audit_log_table ON audit_log(target_table)
GO

CREATE INDEX idx_audit_log_date ON audit_log(performed_at)
GO

CREATE TABLE error_log (
    error_id INT IDENTITY,
    error_number INT NOT NULL,
    error_severity INT NOT NULL,
    error_state INT NOT NULL,
    error_procedure VARCHAR(200) NULL,
    error_line INT NULL,
    error_message VARCHAR(4000) NOT NULL,
    error_context VARCHAR(1000) NULL,
    occurred_at DATETIME NOT NULL,
    resolved BIT NOT NULL,
    resolved_at DATETIME NULL,
    resolved_by VARCHAR(100) NULL,
    CONSTRAINT pk_error_log PRIMARY KEY (error_id)
)
GO

CREATE INDEX idx_error_log_resolved ON error_log(resolved)
GO

-- =============================================================================
-- Batch 12: Rate cards and pricing
-- =============================================================================
CREATE TABLE rate_cards (
    rate_id INT IDENTITY,
    plan_code VARCHAR(50) NOT NULL,
    plan_name VARCHAR(100) NOT NULL,
    base_rate NUMERIC(18, 4) NOT NULL,
    overage_rate NUMERIC(18, 4) NOT NULL,
    included_units INT NOT NULL,
    tier VARCHAR(20) NOT NULL,
    effective_from DATETIME NOT NULL,
    effective_to DATETIME NULL,
    is_active BIT NOT NULL,
    CONSTRAINT pk_rate_cards PRIMARY KEY (rate_id),
    CONSTRAINT uq_rate_cards_plan UNIQUE (plan_code)
)
GO

INSERT INTO rate_cards (plan_code, plan_name, base_rate, overage_rate, included_units, tier, effective_from, is_active)
VALUES ('PREMIUM-API', 'Premium API Plan', 0.0010, 0.0015, 10000, 'PREMIUM', GETDATE(), 1)

INSERT INTO rate_cards (plan_code, plan_name, base_rate, overage_rate, included_units, tier, effective_from, is_active)
VALUES ('STANDARD-API', 'Standard API Plan', 0.0020, 0.0030, 5000, 'STANDARD', GETDATE(), 1)

INSERT INTO rate_cards (plan_code, plan_name, base_rate, overage_rate, included_units, tier, effective_from, is_active)
VALUES ('BASIC-API', 'Basic API Plan', 0.0050, 0.0075, 1000, 'BASIC', GETDATE(), 1)
GO

-- =============================================================================
-- Batch 13: Billing infrastructure
-- =============================================================================
CREATE TABLE billing_batches (
    batch_id INT IDENTITY,
    period_start DATETIME NOT NULL,
    period_end DATETIME NOT NULL,
    period_name VARCHAR(50) NOT NULL,
    status VARCHAR(20) NOT NULL,
    total_customers INT NOT NULL,
    total_amount NUMERIC(18, 2) NOT NULL,
    error_message VARCHAR(1000) NULL,
    created_by VARCHAR(100) NOT NULL,
    created_at DATETIME NOT NULL,
    completed_at DATETIME NULL,
    voided_at DATETIME NULL,
    void_reason VARCHAR(500) NULL,
    voided_by VARCHAR(100) NULL,
    CONSTRAINT pk_billing_batches PRIMARY KEY (batch_id)
)
GO

CREATE INDEX idx_billing_batches_status ON billing_batches(status)
GO

CREATE INDEX idx_billing_batches_period ON billing_batches(period_start, period_end)
GO

CREATE TABLE billing_items_archive (
    item_id INT IDENTITY,
    batch_id INT NOT NULL,
    customer_id INT NOT NULL,
    service_code VARCHAR(50) NOT NULL,
    service_description VARCHAR(500) NOT NULL,
    quantity NUMERIC(18, 4) NOT NULL,
    unit_price NUMERIC(18, 4) NOT NULL,
    line_total NUMERIC(18, 2) NOT NULL,
    discount_pct NUMERIC(5, 2) NULL,
    tax_rate NUMERIC(5, 4) NULL,
    effective_date DATETIME NOT NULL,
    billing_category VARCHAR(30) NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT pk_billing_items_archive PRIMARY KEY (item_id),
    CONSTRAINT fk_billing_items_batch FOREIGN KEY (batch_id) REFERENCES billing_batches(batch_id),
    CONSTRAINT fk_billing_items_customer FOREIGN KEY (customer_id) REFERENCES customers(customer_id)
)
GO

CREATE INDEX idx_billing_items_batch ON billing_items_archive(batch_id)
GO

CREATE INDEX idx_billing_items_customer ON billing_items_archive(customer_id)
GO

-- =============================================================================
-- Batch 14: Usage tracking tables
-- =============================================================================
CREATE TABLE customer_usage (
    usage_id INT IDENTITY,
    customer_id INT NOT NULL,
    usage_date DATETIME NOT NULL,
    service_plan VARCHAR(50) NOT NULL,
    api_call_count INT NOT NULL,
    storage_used_mb NUMERIC(18, 4) NOT NULL,
    compute_hours NUMERIC(18, 4) NOT NULL,
    data_transfer_gb NUMERIC(18, 4) NOT NULL,
    CONSTRAINT pk_customer_usage PRIMARY KEY (usage_id),
    CONSTRAINT fk_customer_usage_customer FOREIGN KEY (customer_id) REFERENCES customers(customer_id)
)
GO

CREATE INDEX idx_customer_usage_customer_date ON customer_usage(customer_id, usage_date)
GO

CREATE TABLE storage_metrics (
    metric_id INT IDENTITY,
    customer_id INT NOT NULL,
    metric_date DATETIME NOT NULL,
    storage_used_mb NUMERIC(18, 4) NOT NULL,
    storage_limit_mb NUMERIC(18, 4) NOT NULL,
    CONSTRAINT pk_storage_metrics PRIMARY KEY (metric_id)
)
GO

CREATE INDEX idx_storage_metrics_customer_date ON storage_metrics(customer_id, metric_date)
GO

-- =============================================================================
-- Batch 15: Views for reporting
-- =============================================================================
CREATE VIEW v_order_summary AS
SELECT
    o.order_id,
    o.order_number,
    c.customer_name,
    c.region_code,
    o.order_date,
    o.order_status,
    o.total_amount,
    COUNT(oi.order_item_id) AS item_count
FROM orders o
INNER JOIN customers c ON o.customer_id = c.customer_id
LEFT JOIN order_items oi ON o.order_id = oi.order_id
GROUP BY o.order_id, o.order_number, c.customer_name, c.region_code,
         o.order_date, o.order_status, o.total_amount
GO

CREATE VIEW v_inventory_status AS
SELECT
    p.product_id,
    p.sku,
    p.product_name,
    pc.category_name,
    i.warehouse_code,
    i.quantity_on_hand,
    i.quantity_reserved,
    i.quantity_available,
    i.reorder_point,
    CASE
        WHEN i.quantity_available <= i.reorder_point THEN 'REORDER'
        WHEN i.quantity_available <= i.reorder_point * 2 THEN 'LOW'
        ELSE 'OK'
    END AS stock_status
FROM products p
INNER JOIN product_categories pc ON p.category_id = pc.category_id
INNER JOIN inventory i ON p.product_id = i.product_id
WHERE p.is_active = 1
GO

-- =============================================================================
-- Batch 16: Stored procedures
-- =============================================================================
CREATE PROCEDURE sp_create_order
    @customer_id INT,
    @order_items_xml TEXT,
    @payment_method VARCHAR(30),
    @order_id INT OUTPUT
AS
BEGIN
    DECLARE @order_number VARCHAR(30)
    DECLARE @subtotal NUMERIC(18, 2)
    DECLARE @tax_rate NUMERIC(5, 4)
    DECLARE @tax_amount NUMERIC(18, 2)
    DECLARE @shipping_amount NUMERIC(18, 2)

    SET @subtotal = 0
    SET @tax_rate = 0.10
    SET @shipping_amount = 0
    SET @order_number = 'ORD-' + CONVERT(VARCHAR, GETDATE(), 112) + '-' +
                        RIGHT('0000' + CONVERT(VARCHAR, DATEPART(SECOND, GETDATE()) * 1000 + DATEPART(MILLISECOND, GETDATE())), 4)

    BEGIN TRANSACTION

    BEGIN TRY
        INSERT INTO orders (order_number, customer_id, order_status, order_date,
                           subtotal, tax_amount, discount_amount, shipping_amount, total_amount,
                           payment_method, payment_status, created_at)
        VALUES (@order_number, @customer_id, 'PENDING', GETDATE(),
                0, 0, 0, 0, 0,
                @payment_method, 'PENDING', GETDATE())

        SET @order_id = @@IDENTITY

        COMMIT TRANSACTION
    END TRY
    BEGIN CATCH
        ROLLBACK TRANSACTION
        RAISERROR('Failed to create order: %s', 16, 1, ERROR_MESSAGE())
        RETURN -1
    END CATCH

    RETURN 0
END
GO

-- =============================================================================
-- Batch 17: Schema version tracking
-- =============================================================================
INSERT INTO schema_version (version_number, description, applied_at, applied_by, execution_time_ms, checksum)
VALUES ('1.0.0', 'Initial schema - core tables', GETDATE(), SYSTEM_USER, 0, NULL)
GO

-- =============================================================================
-- Batch 18: Triggers for data integrity
-- =============================================================================
CREATE TRIGGER trg_order_status_change
ON orders
FOR UPDATE
AS
BEGIN
    DECLARE @old_status VARCHAR(20)
    DECLARE @new_status VARCHAR(20)

    SELECT @old_status = order_status FROM deleted
    SELECT @new_status = order_status FROM inserted

    IF @old_status <> @new_status
    BEGIN
        INSERT INTO audit_log (action_type, target_table, target_id, action_details, performed_by, performed_at)
        SELECT 'STATUS_CHANGE', 'orders', order_id,
               'Status changed from ' + @old_status + ' to ' + @new_status,
               SYSTEM_USER, GETDATE()
        FROM inserted
    END
END
GO

-- =============================================================================
-- Batch 19: Helper procedure for reporting
-- =============================================================================
CREATE PROCEDURE sp_get_customer_orders
    @customer_id INT,
    @start_date DATETIME = NULL,
    @end_date DATETIME = NULL,
    @status_filter VARCHAR(20) = NULL
AS
BEGIN
    SELECT
        o.order_id,
        o.order_number,
        o.order_date,
        o.order_status,
        o.total_amount,
        COUNT(oi.order_item_id) AS item_count,
        SUM(oi.quantity) AS total_quantity
    FROM orders o
    LEFT JOIN order_items oi ON o.order_id = oi.order_id
    WHERE o.customer_id = @customer_id
      AND (@start_date IS NULL OR o.order_date >= @start_date)
      AND (@end_date IS NULL OR o.order_date <= @end_date)
      AND (@status_filter IS NULL OR o.order_status = @status_filter)
    GROUP BY o.order_id, o.order_number, o.order_date, o.order_status, o.total_amount
    ORDER BY o.order_date DESC
END
GO

-- =============================================================================
-- Batch 20: Final verification queries
-- =============================================================================
SELECT 'schema_version' AS table_name, COUNT(*) AS row_count FROM schema_version
UNION ALL
SELECT 'lookup_regions', COUNT(*) FROM lookup_regions
UNION ALL
SELECT 'product_categories', COUNT(*) FROM product_categories
UNION ALL
SELECT 'products', COUNT(*) FROM products
UNION ALL
SELECT 'customers', COUNT(*) FROM customers
UNION ALL
SELECT 'orders', COUNT(*) FROM orders
UNION ALL
SELECT 'inventory', COUNT(*) FROM inventory
UNION ALL
SELECT 'rate_cards', COUNT(*) FROM rate_cards
UNION ALL
SELECT 'billing_batches', COUNT(*) FROM billing_batches
UNION ALL
SELECT 'audit_log', COUNT(*) FROM audit_log
GO

SELECT 'Migration complete' AS status, GETDATE() AS completed_at
GO

-- =============================================================================
-- Batch 21: Permissions and security tables
-- =============================================================================
CREATE TABLE security_roles (
    role_id INT IDENTITY,
    role_name VARCHAR(50) NOT NULL,
    description VARCHAR(200) NULL,
    is_system BIT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT pk_security_roles PRIMARY KEY (role_id),
    CONSTRAINT uq_security_roles_name UNIQUE (role_name)
)
GO

CREATE TABLE security_users (
    user_id INT IDENTITY,
    username VARCHAR(100) NOT NULL,
    email VARCHAR(255) NOT NULL,
    display_name VARCHAR(200) NOT NULL,
    is_active BIT NOT NULL,
    last_login DATETIME NULL,
    password_hash VARCHAR(256) NOT NULL,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NULL,
    CONSTRAINT pk_security_users PRIMARY KEY (user_id),
    CONSTRAINT uq_security_users_username UNIQUE (username)
)
GO

CREATE TABLE security_user_roles (
    user_role_id INT IDENTITY,
    user_id INT NOT NULL,
    role_id INT NOT NULL,
    granted_at DATETIME NOT NULL,
    granted_by VARCHAR(100) NOT NULL,
    expires_at DATETIME NULL,
    CONSTRAINT pk_security_user_roles PRIMARY KEY (user_role_id),
    CONSTRAINT fk_user_roles_user FOREIGN KEY (user_id) REFERENCES security_users(user_id),
    CONSTRAINT fk_user_roles_role FOREIGN KEY (role_id) REFERENCES security_roles(role_id)
)
GO

CREATE INDEX idx_user_roles_user ON security_user_roles(user_id)
GO

CREATE TABLE security_permissions (
    permission_id INT IDENTITY,
    permission_code VARCHAR(100) NOT NULL,
    permission_name VARCHAR(200) NOT NULL,
    resource_type VARCHAR(50) NOT NULL,
    description VARCHAR(500) NULL,
    CONSTRAINT pk_security_permissions PRIMARY KEY (permission_id),
    CONSTRAINT uq_security_permissions_code UNIQUE (permission_code)
)
GO

CREATE TABLE security_role_permissions (
    role_permission_id INT IDENTITY,
    role_id INT NOT NULL,
    permission_id INT NOT NULL,
    CONSTRAINT pk_security_role_permissions PRIMARY KEY (role_permission_id),
    CONSTRAINT fk_role_permissions_role FOREIGN KEY (role_id) REFERENCES security_roles(role_id),
    CONSTRAINT fk_role_permissions_permission FOREIGN KEY (permission_id) REFERENCES security_permissions(permission_id)
)
GO

-- =============================================================================
-- Batch 22: Seed security data
-- =============================================================================
INSERT INTO security_roles (role_name, description, is_system, created_at)
VALUES ('ADMIN', 'System Administrator', 1, GETDATE())

INSERT INTO security_roles (role_name, description, is_system, created_at)
VALUES ('MANAGER', 'Billing Manager', 1, GETDATE())

INSERT INTO security_roles (role_name, description, is_system, created_at)
VALUES ('OPERATOR', 'Billing Operator', 1, GETDATE())

INSERT INTO security_roles (role_name, description, is_system, created_at)
VALUES ('VIEWER', 'Read-only Access', 1, GETDATE())

INSERT INTO security_roles (role_name, description, is_system, created_at)
VALUES ('AUDITOR', 'Audit Viewer', 0, GETDATE())
GO

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('BILLING.CREATE', 'Create Billing Batch', 'BILLING', 'Create and execute billing batches')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('BILLING.VOID', 'Void Billing Batch', 'BILLING', 'Void completed billing batches')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('BILLING.VIEW', 'View Billing Data', 'BILLING', 'View billing reports and details')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('CUSTOMER.MANAGE', 'Manage Customers', 'CUSTOMER', 'Create, update, deactivate customers')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('CUSTOMER.VIEW', 'View Customers', 'CUSTOMER', 'View customer details')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('PRODUCT.MANAGE', 'Manage Products', 'PRODUCT', 'Create, update, deactivate products')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('PRODUCT.VIEW', 'View Products', 'PRODUCT', 'View product catalog')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('ORDER.CREATE', 'Create Orders', 'ORDER', 'Create new orders')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('ORDER.VIEW', 'View Orders', 'ORDER', 'View order history and details')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('ORDER.CANCEL', 'Cancel Orders', 'ORDER', 'Cancel pending orders')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('REPORT.VIEW', 'View Reports', 'REPORT', 'Access all reporting views')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('ADMIN.USERS', 'Manage Users', 'SYSTEM', 'Create and manage system users')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('ADMIN.ROLES', 'Manage Roles', 'SYSTEM', 'Create and manage security roles')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('AUDIT.VIEW', 'View Audit Log', 'SYSTEM', 'Read audit log entries')

INSERT INTO security_permissions (permission_code, permission_name, resource_type, description)
VALUES ('SYSTEM.CONFIG', 'System Configuration', 'SYSTEM', 'Modify system settings')
GO

-- Admin gets all permissions
INSERT INTO security_role_permissions (role_id, permission_id)
SELECT r.role_id, p.permission_id
FROM security_roles r
CROSS JOIN security_permissions p
WHERE r.role_name = 'ADMIN'
GO

-- Manager gets billing + customer + order + report permissions
INSERT INTO security_role_permissions (role_id, permission_id)
SELECT r.role_id, p.permission_id
FROM security_roles r
CROSS JOIN security_permissions p
WHERE r.role_name = 'MANAGER'
  AND p.permission_code IN ('BILLING.CREATE', 'BILLING.VOID', 'BILLING.VIEW',
                             'CUSTOMER.MANAGE', 'CUSTOMER.VIEW',
                             'ORDER.CREATE', 'ORDER.VIEW', 'ORDER.CANCEL',
                             'REPORT.VIEW')
GO

-- Operator gets limited billing + order permissions
INSERT INTO security_role_permissions (role_id, permission_id)
SELECT r.role_id, p.permission_id
FROM security_roles r
CROSS JOIN security_permissions p
WHERE r.role_name = 'OPERATOR'
  AND p.permission_code IN ('BILLING.VIEW',
                             'CUSTOMER.VIEW',
                             'ORDER.CREATE', 'ORDER.VIEW',
                             'PRODUCT.VIEW')
GO

-- Viewer gets read-only access
INSERT INTO security_role_permissions (role_id, permission_id)
SELECT r.role_id, p.permission_id
FROM security_roles r
CROSS JOIN security_permissions p
WHERE r.role_name = 'VIEWER'
  AND p.permission_code LIKE '%.VIEW'
GO

-- Auditor gets audit + view permissions
INSERT INTO security_role_permissions (role_id, permission_id)
SELECT r.role_id, p.permission_id
FROM security_roles r
CROSS JOIN security_permissions p
WHERE r.role_name = 'AUDITOR'
  AND p.permission_code IN ('AUDIT.VIEW', 'BILLING.VIEW', 'CUSTOMER.VIEW',
                             'ORDER.VIEW', 'PRODUCT.VIEW', 'REPORT.VIEW')
GO

-- =============================================================================
-- Batch 23: Notification tables
-- =============================================================================
CREATE TABLE notification_templates (
    template_id INT IDENTITY,
    template_code VARCHAR(50) NOT NULL,
    template_name VARCHAR(100) NOT NULL,
    subject VARCHAR(200) NOT NULL,
    body_template TEXT NOT NULL,
    channel VARCHAR(20) NOT NULL,
    is_active BIT NOT NULL,
    created_at DATETIME NOT NULL,
    updated_at DATETIME NULL,
    CONSTRAINT pk_notification_templates PRIMARY KEY (template_id),
    CONSTRAINT uq_notification_templates_code UNIQUE (template_code)
)
GO

CREATE TABLE notification_queue (
    notification_id INT IDENTITY,
    template_id INT NULL,
    recipient_customer_id INT NULL,
    recipient_email VARCHAR(255) NOT NULL,
    recipient_name VARCHAR(200) NULL,
    subject VARCHAR(200) NOT NULL,
    body TEXT NOT NULL,
    channel VARCHAR(20) NOT NULL,
    status VARCHAR(20) NOT NULL,
    priority INT NOT NULL,
    scheduled_at DATETIME NOT NULL,
    sent_at DATETIME NULL,
    error_message VARCHAR(1000) NULL,
    retry_count INT NOT NULL,
    created_at DATETIME NOT NULL,
    CONSTRAINT pk_notification_queue PRIMARY KEY (notification_id),
    CONSTRAINT fk_notification_queue_template FOREIGN KEY (template_id) REFERENCES notification_templates(template_id)
)
GO

CREATE INDEX idx_notification_queue_status ON notification_queue(status)
GO

CREATE INDEX idx_notification_queue_scheduled ON notification_queue(scheduled_at)
GO

-- =============================================================================
-- Batch 24: Seed notification templates
-- =============================================================================
INSERT INTO notification_templates (template_code, template_name, subject, body_template, channel, is_active, created_at)
VALUES ('BILLING_CREATED', 'Billing Created', 'New billing batch created for period {period}',
        'Dear {customer_name}, a new billing for the period {period} has been generated. Total amount: {amount}.',
        'EMAIL', 1, GETDATE())

INSERT INTO notification_templates (template_code, template_name, subject, body_template, channel, is_active, created_at)
VALUES ('BILLING_VOIDED', 'Billing Voided', 'Billing batch {batch_id} has been voided',
        'Dear {customer_name}, the billing batch {batch_id} for period {period} has been voided. Reason: {reason}.',
        'EMAIL', 1, GETDATE())

INSERT INTO notification_templates (template_code, template_name, subject, body_template, channel, is_active, created_at)
VALUES ('ORDER_CONFIRMED', 'Order Confirmed', 'Order {order_number} confirmed',
        'Dear {customer_name}, your order {order_number} has been confirmed. Total: {total_amount}.',
        'EMAIL', 1, GETDATE())

INSERT INTO notification_templates (template_code, template_name, subject, body_template, channel, is_active, created_at)
VALUES ('ORDER_SHIPPED', 'Order Shipped', 'Order {order_number} has been shipped',
        'Dear {customer_name}, your order {order_number} has been shipped.',
        'EMAIL', 1, GETDATE())

INSERT INTO notification_templates (template_code, template_name, subject, body_template, channel, is_active, created_at)
VALUES ('PAYMENT_RECEIVED', 'Payment Received', 'Payment received for invoice {invoice_number}',
        'Dear {customer_name}, we received your payment of {amount} for invoice {invoice_number}.',
        'EMAIL', 1, GETDATE())

INSERT INTO notification_templates (template_code, template_name, subject, body_template, channel, is_active, created_at)
VALUES ('LOW_STOCK_ALERT', 'Low Stock Alert', 'Low stock: {product_name}',
        'Product {product_name} (SKU: {sku}) has fallen below the reorder point. Current stock: {quantity_available}.',
        'EMAIL', 1, GETDATE())
GO

-- =============================================================================
-- Batch 25: Data warehouse / reporting tables
-- =============================================================================
CREATE TABLE dw_fact_orders (
    fact_id INT IDENTITY,
    order_id INT NOT NULL,
    customer_id INT NOT NULL,
    product_id INT NOT NULL,
    order_date_key INT NOT NULL,
    region_code VARCHAR(10) NOT NULL,
    quantity INT NOT NULL,
    unit_price NUMERIC(18, 4) NOT NULL,
    line_total NUMERIC(18, 2) NOT NULL,
    discount_amount NUMERIC(18, 2) NOT NULL,
    tax_amount NUMERIC(18, 2) NOT NULL,
    net_amount NUMERIC(18, 2) NOT NULL,
    CONSTRAINT pk_dw_fact_orders PRIMARY KEY (fact_id)
)
GO

CREATE TABLE dw_dim_dates (
    date_key INT NOT NULL,
    full_date DATETIME NOT NULL,
    year_num INT NOT NULL,
    quarter_num INT NOT NULL,
    month_num INT NOT NULL,
    week_num INT NOT NULL,
    day_of_week INT NOT NULL,
    day_of_month INT NOT NULL,
    day_of_year INT NOT NULL,
    month_name VARCHAR(20) NOT NULL,
    day_name VARCHAR(20) NOT NULL,
    is_weekend BIT NOT NULL,
    is_holiday BIT NOT NULL,
    fiscal_year INT NOT NULL,
    fiscal_quarter INT NOT NULL,
    fiscal_month INT NOT NULL,
    CONSTRAINT pk_dw_dim_dates PRIMARY KEY (date_key)
)
GO

CREATE TABLE dw_dim_customers (
    customer_key INT IDENTITY,
    customer_id INT NOT NULL,
    customer_code VARCHAR(20) NOT NULL,
    customer_name VARCHAR(200) NOT NULL,
    account_type VARCHAR(50) NOT NULL,
    billing_tier VARCHAR(20) NOT NULL,
    region_code VARCHAR(10) NOT NULL,
    registration_date DATETIME NOT NULL,
    is_current BIT NOT NULL,
    valid_from DATETIME NOT NULL,
    valid_to DATETIME NOT NULL,
    CONSTRAINT pk_dw_dim_customers PRIMARY KEY (customer_key)
)
GO

CREATE INDEX idx_dw_dim_customers_id ON dw_dim_customers(customer_id)
GO

CREATE INDEX idx_dw_dim_customers_current ON dw_dim_customers(is_current)
GO

-- =============================================================================
-- Batch 26: Procedures for data warehouse loading
-- =============================================================================
CREATE PROCEDURE sp_load_dw_fact_orders
    @start_date DATETIME,
    @end_date DATETIME
AS
BEGIN
    SET NOCOUNT ON

    DECLARE @loaded_count INT
    SET @loaded_count = 0

    BEGIN TRY
        INSERT INTO dw_fact_orders (
            order_id, customer_id, product_id, order_date_key,
            region_code, quantity, unit_price, line_total,
            discount_amount, tax_amount, net_amount
        )
        SELECT
            o.order_id,
            o.customer_id,
            oi.product_id,
            CONVERT(INT, CONVERT(VARCHAR, o.order_date, 112)),
            c.region_code,
            oi.quantity,
            oi.unit_price,
            oi.line_total,
            oi.line_total * oi.discount_pct / 100,
            oi.line_total * 0.10,
            oi.line_total - (oi.line_total * oi.discount_pct / 100) + (oi.line_total * 0.10)
        FROM orders o
        INNER JOIN order_items oi ON o.order_id = oi.order_id
        INNER JOIN customers c ON o.customer_id = c.customer_id
        WHERE o.order_date BETWEEN @start_date AND @end_date
          AND o.order_status = 'COMPLETED'

        SET @loaded_count = @@ROWCOUNT

        INSERT INTO audit_log (action_type, target_table, target_id, action_details, performed_by, performed_at)
        VALUES ('DW_LOAD', 'dw_fact_orders', NULL,
                'Loaded ' + CONVERT(VARCHAR, @loaded_count) + ' fact records for period ' +
                CONVERT(VARCHAR, @start_date, 120) + ' to ' + CONVERT(VARCHAR, @end_date, 120),
                SYSTEM_USER, GETDATE())

        RETURN @loaded_count
    END TRY
    BEGIN CATCH
        INSERT INTO error_log (error_number, error_severity, error_state, error_procedure, error_line, error_message, error_context, occurred_at, resolved)
        VALUES (ERROR_NUMBER(), ERROR_SEVERITY(), ERROR_STATE(), ERROR_PROCEDURE(), ERROR_LINE(), ERROR_MESSAGE(), 'DW fact load', GETDATE(), 0)

        RETURN -1
    END CATCH
END
GO

-- =============================================================================
-- Batch 27: System configuration table
-- =============================================================================
CREATE TABLE system_config (
    config_key VARCHAR(100) NOT NULL,
    config_value VARCHAR(2000) NOT NULL,
    config_type VARCHAR(20) NOT NULL,
    description VARCHAR(500) NULL,
    is_encrypted BIT NOT NULL,
    updated_by VARCHAR(100) NOT NULL,
    updated_at DATETIME NOT NULL,
    CONSTRAINT pk_system_config PRIMARY KEY (config_key)
)
GO

INSERT INTO system_config (config_key, config_value, config_type, description, is_encrypted, updated_by, updated_at)
VALUES ('billing.default_currency', 'JPY', 'STRING', 'Default currency for billing', 0, 'SYSTEM', GETDATE())

INSERT INTO system_config (config_key, config_value, config_type, description, is_encrypted, updated_by, updated_at)
VALUES ('billing.tax_enabled', 'true', 'BOOLEAN', 'Enable tax calculation', 0, 'SYSTEM', GETDATE())

INSERT INTO system_config (config_key, config_value, config_type, description, is_encrypted, updated_by, updated_at)
VALUES ('billing.max_batch_errors', '100', 'INTEGER', 'Maximum errors before batch abort', 0, 'SYSTEM', GETDATE())

INSERT INTO system_config (config_key, config_value, config_type, description, is_encrypted, updated_by, updated_at)
VALUES ('notification.from_address', 'noreply@example.com', 'STRING', 'Sender email address', 0, 'SYSTEM', GETDATE())

INSERT INTO system_config (config_key, config_value, config_type, description, is_encrypted, updated_by, updated_at)
VALUES ('notification.smtp_host', 'smtp.example.com', 'STRING', 'SMTP server hostname', 0, 'SYSTEM', GETDATE())

INSERT INTO system_config (config_key, config_value, config_type, description, is_encrypted, updated_by, updated_at)
VALUES ('notification.smtp_port', '587', 'INTEGER', 'SMTP server port', 0, 'SYSTEM', GETDATE())

INSERT INTO system_config (config_key, config_value, config_type, description, is_encrypted, updated_by, updated_at)
VALUES ('order.auto_confirm', 'false', 'BOOLEAN', 'Automatically confirm new orders', 0, 'SYSTEM', GETDATE())

INSERT INTO system_config (config_key, config_value, config_type, description, is_encrypted, updated_by, updated_at)
VALUES ('inventory.reorder_enabled', 'true', 'BOOLEAN', 'Enable automatic reorder alerts', 0, 'SYSTEM', GETDATE())

INSERT INTO system_config (config_key, config_value, config_type, description, is_encrypted, updated_by, updated_at)
VALUES ('dw.retention_days', '365', 'INTEGER', 'Data warehouse retention period in days', 0, 'SYSTEM', GETDATE())

INSERT INTO system_config (config_key, config_value, config_type, description, is_encrypted, updated_by, updated_at)
VALUES ('audit.retention_days', '730', 'INTEGER', 'Audit log retention period in days', 0, 'SYSTEM', GETDATE())
GO

-- =============================================================================
-- Batch 28: Final schema version update
-- =============================================================================
INSERT INTO schema_version (version_number, description, applied_at, applied_by, execution_time_ms, checksum)
VALUES ('1.1.0', 'Security, notifications, data warehouse, system config', GETDATE(), SYSTEM_USER, 0, NULL)
GO

-- =============================================================================
-- Batch 29: Comprehensive verification
-- =============================================================================
SELECT 'Tables' AS category, COUNT(*) AS count FROM sysobjects WHERE type = 'U'
UNION ALL
SELECT 'Views', COUNT(*) FROM sysobjects WHERE type = 'V'
UNION ALL
SELECT 'Procedures', COUNT(*) FROM sysobjects WHERE type = 'P'
UNION ALL
SELECT 'Triggers', COUNT(*) FROM sysobjects WHERE type = 'TR'
UNION ALL
SELECT 'Indexes', COUNT(*) FROM sysindexes WHERE indid > 0
GO

SELECT 'Migration v1.1.0 complete' AS status, GETDATE() AS completed_at
GO
