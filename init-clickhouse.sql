-- ClickHouse initialization script
-- Note: ClickHouse has different syntax and features compared to PostgreSQL

-- Create the test database
CREATE DATABASE IF NOT EXISTS test;

USE test;

-- Users table
CREATE TABLE IF NOT EXISTS users (
  id UInt32,
  name String,
  email String,
  created_at DateTime DEFAULT now(),
  updated_at DateTime DEFAULT now(),
  is_active UInt8 DEFAULT 1
) ENGINE = MergeTree()
ORDER BY id;

INSERT INTO users (id, name, email) VALUES
  (1, 'Alpha', 'alpha@example.com'),
  (2, 'Beta', 'beta@example.com'),
  (3, 'Gamma', 'gamma@example.com'),
  (4, 'Delta', 'delta@example.com'),
  (5, 'Echo', 'echo@example.com'),
  (6, 'Foxtrot', 'foxtrot@example.com');

-- Companies table
CREATE TABLE IF NOT EXISTS companies (
  id UInt32,
  name String,
  industry String,
  founded_year UInt16,
  headquarters String,
  website String,
  employee_count UInt32,
  annual_revenue Decimal64(2),
  is_public UInt8 DEFAULT 0,
  created_at DateTime DEFAULT now()
) ENGINE = MergeTree()
ORDER BY id;

INSERT INTO companies (id, name, industry, founded_year, headquarters, website, employee_count, annual_revenue, is_public) VALUES
  (1, 'TechCorp Solutions', 'Technology', 2010, 'San Francisco, CA', 'https://techcorp.com', 1250, 89500000.00, 1),
  (2, 'Global Manufacturing Inc', 'Manufacturing', 1985, 'Detroit, MI', 'https://globalmfg.com', 5600, 230000000.00, 1),
  (3, 'Green Energy Partners', 'Renewable Energy', 2015, 'Austin, TX', 'https://greenenergy.com', 340, 12800000.00, 0),
  (4, 'Digital Marketing Hub', 'Marketing', 2018, 'New York, NY', 'https://dmhub.com', 85, 5200000.00, 0),
  (5, 'Healthcare Innovations', 'Healthcare', 2005, 'Boston, MA', 'https://healthinnovate.com', 890, 45600000.00, 0);

-- Categories table (simplified - ClickHouse doesn't support self-referential foreign keys well)
CREATE TABLE IF NOT EXISTS categories (
  id UInt32,
  name String,
  description String,
  parent_id Nullable(UInt32),
  sort_order Int32 DEFAULT 0,
  is_active UInt8 DEFAULT 1
) ENGINE = MergeTree()
ORDER BY id;

INSERT INTO categories (id, name, description, parent_id, sort_order) VALUES
  (1, 'Electronics', 'Electronic devices and components', NULL, 1),
  (2, 'Computers', 'Desktop and laptop computers', 1, 1),
  (3, 'Mobile Devices', 'Phones, tablets, and accessories', 1, 2),
  (4, 'Home & Garden', 'Home improvement and gardening supplies', NULL, 2),
  (5, 'Furniture', 'Indoor and outdoor furniture', 4, 1),
  (6, 'Tools', 'Hand tools and power tools', 4, 2),
  (7, 'Books', 'Physical and digital books', NULL, 3),
  (8, 'Fiction', 'Novels and short stories', 7, 1),
  (9, 'Non-Fiction', 'Educational and reference books', 7, 2);

-- Products table
CREATE TABLE IF NOT EXISTS products (
  id UInt32,
  sku String,
  name String,
  description String,
  category_id UInt32,
  price Decimal64(2),
  cost Decimal64(2),
  stock_quantity Int32 DEFAULT 0,
  min_stock_level Int32 DEFAULT 5,
  weight_kg Decimal32(2),
  dimensions_cm String,
  manufacturer String,
  warranty_months UInt16 DEFAULT 12,
  is_discontinued UInt8 DEFAULT 0,
  created_at DateTime DEFAULT now(),
  updated_at DateTime DEFAULT now()
) ENGINE = MergeTree()
ORDER BY (id, sku);

INSERT INTO products (id, sku, name, description, category_id, price, cost, stock_quantity, min_stock_level, weight_kg, dimensions_cm, manufacturer, warranty_months) VALUES
  (1, 'LAPTOP001', 'UltraBook Pro 15', 'High-performance laptop with 16GB RAM and 512GB SSD', 2, 1299.99, 850.00, 25, 5, 1.8, '35x24x2', 'TechCorp', 24),
  (2, 'PHONE001', 'SmartPhone X', 'Latest smartphone with advanced camera system', 3, 899.99, 600.00, 150, 20, 0.18, '15x7x1', 'MobileTech', 12),
  (3, 'CHAIR001', 'Ergonomic Office Chair', 'Adjustable office chair with lumbar support', 5, 249.99, 125.00, 45, 10, 15.5, '60x60x120', 'ComfortSeating', 36),
  (4, 'DRILL001', 'Cordless Power Drill', '18V cordless drill with 2 batteries', 6, 89.99, 45.00, 78, 15, 1.2, '25x8x20', 'PowerTools Pro', 24),
  (5, 'BOOK001', 'The Art of Programming', 'Comprehensive guide to software development', 9, 49.99, 25.00, 200, 25, 0.8, '24x17x3', 'Tech Publishers', 0);

-- Order statuses table
CREATE TABLE IF NOT EXISTS order_statuses (
  id UInt32,
  name String,
  description String,
  sort_order Int32 DEFAULT 0
) ENGINE = MergeTree()
ORDER BY id;

INSERT INTO order_statuses (id, name, description, sort_order) VALUES
  (1, 'pending', 'Order received, awaiting processing', 1),
  (2, 'processing', 'Order is being prepared', 2),
  (3, 'shipped', 'Order has been shipped', 3),
  (4, 'delivered', 'Order has been delivered', 4),
  (5, 'cancelled', 'Order has been cancelled', 5),
  (6, 'returned', 'Order has been returned', 6);

-- Orders table
CREATE TABLE IF NOT EXISTS orders (
  id UInt32,
  order_number String,
  user_id UInt32,
  company_id UInt32,
  status_id UInt32 DEFAULT 1,
  order_date DateTime DEFAULT now(),
  ship_date Nullable(DateTime),
  total_amount Decimal64(2),
  tax_amount Decimal64(2) DEFAULT 0,
  shipping_amount Decimal64(2) DEFAULT 0,
  discount_amount Decimal64(2) DEFAULT 0,
  shipping_address String,
  billing_address String,
  notes String
) ENGINE = MergeTree()
ORDER BY (id, order_date);

INSERT INTO orders (id, order_number, user_id, company_id, status_id, order_date, total_amount, tax_amount, shipping_amount, shipping_address, billing_address) VALUES
  (1, 'ORD-2024-001', 1, 1, 3, '2024-01-15 10:30:00', 1549.98, 124.00, 25.99, '123 Main St, Anytown, ST 12345', '123 Main St, Anytown, ST 12345'),
  (2, 'ORD-2024-002', 2, 2, 4, '2024-01-18 14:22:00', 899.99, 72.00, 15.99, '456 Oak Ave, Somewhere, ST 67890', '456 Oak Ave, Somewhere, ST 67890'),
  (3, 'ORD-2024-003', 3, 1, 2, '2024-01-20 09:15:00', 339.97, 27.20, 12.99, '789 Pine Rd, Elsewhere, ST 54321', '789 Pine Rd, Elsewhere, ST 54321'),
  (4, 'ORD-2024-004', 4, 3, 1, '2024-01-22 16:45:00', 139.98, 11.20, 8.99, '321 Elm St, Nowhere, ST 98765', '321 Elm St, Nowhere, ST 98765');

-- Order items table
CREATE TABLE IF NOT EXISTS order_items (
  id UInt32,
  order_id UInt32,
  product_id UInt32,
  quantity UInt32,
  unit_price Decimal64(2),
  total_price Decimal64(2),
  discount_percent Decimal32(2) DEFAULT 0
) ENGINE = MergeTree()
ORDER BY (order_id, id);

INSERT INTO order_items (id, order_id, product_id, quantity, unit_price, total_price) VALUES
  (1, 1, 1, 1, 1299.99, 1299.99),
  (2, 1, 3, 1, 249.99, 249.99),
  (3, 2, 2, 1, 899.99, 899.99),
  (4, 3, 3, 1, 249.99, 249.99),
  (5, 3, 4, 1, 89.99, 89.99),
  (6, 4, 4, 1, 89.99, 89.99),
  (7, 4, 5, 1, 49.99, 49.99);

-- User roles table
CREATE TABLE IF NOT EXISTS user_roles (
  id UInt32,
  name String,
  description String,
  permissions Array(String),
  is_active UInt8 DEFAULT 1,
  created_at DateTime DEFAULT now()
) ENGINE = MergeTree()
ORDER BY id;

INSERT INTO user_roles (id, name, description, permissions) VALUES
  (1, 'admin', 'Full system administrator', ['users.create', 'users.read', 'users.update', 'users.delete', 'orders.create', 'orders.read', 'orders.update', 'orders.delete']),
  (2, 'manager', 'Department manager with limited admin rights', ['users.read', 'users.update', 'orders.read', 'orders.update', 'products.create', 'products.update']),
  (3, 'employee', 'Regular employee access', ['orders.read', 'products.read', 'customers.read']),
  (4, 'customer', 'Customer portal access', ['orders.read.own', 'profile.update']);

-- User role assignments (many-to-many)
CREATE TABLE IF NOT EXISTS user_role_assignments (
  id UInt32,
  user_id UInt32,
  role_id UInt32,
  assigned_at DateTime DEFAULT now(),
  assigned_by UInt32
) ENGINE = MergeTree()
ORDER BY (user_id, role_id);

INSERT INTO user_role_assignments (id, user_id, role_id, assigned_by) VALUES
  (1, 1, 1, 1), -- Alpha is admin
  (2, 2, 2, 1), -- Beta is manager
  (3, 3, 3, 1), -- Gamma is employee
  (4, 4, 4, 1), -- Delta is customer
  (5, 5, 3, 1), -- Echo is employee
  (6, 6, 4, 1); -- Foxtrot is customer

-- Product reviews table
CREATE TABLE IF NOT EXISTS product_reviews (
  id UInt32,
  product_id UInt32,
  user_id UInt32,
  rating UInt8,
  title String,
  review_text String,
  is_verified_purchase UInt8 DEFAULT 0,
  helpful_votes UInt32 DEFAULT 0,
  created_at DateTime DEFAULT now(),
  updated_at DateTime DEFAULT now()
) ENGINE = MergeTree()
ORDER BY (product_id, created_at);

INSERT INTO product_reviews (id, product_id, user_id, rating, title, review_text, is_verified_purchase, helpful_votes) VALUES
  (1, 1, 2, 5, 'Excellent laptop!', 'Great performance, long battery life, highly recommended for professional work.', 1, 12),
  (2, 1, 3, 4, 'Very good but pricey', 'Solid build quality and fast performance, but quite expensive.', 1, 8),
  (3, 2, 4, 5, 'Amazing camera quality', 'The camera on this phone is incredible, takes professional-quality photos.', 1, 15),
  (4, 3, 5, 4, 'Comfortable office chair', 'Very comfortable for long work sessions, good lumbar support.', 1, 6),
  (5, 4, 6, 5, 'Perfect for DIY projects', 'Powerful drill with long-lasting batteries, great value for money.', 1, 9);

-- System logs table
CREATE TABLE IF NOT EXISTS system_logs (
  id UInt32,
  log_level String,
  message String,
  module String,
  user_id Nullable(UInt32),
  ip_address Nullable(IPv4),
  user_agent String,
  created_at DateTime DEFAULT now()
) ENGINE = MergeTree()
ORDER BY (created_at, log_level);

INSERT INTO system_logs (id, log_level, message, module, user_id, ip_address) VALUES
  (1, 'INFO', 'User logged in successfully', 'authentication', 1, '192.168.1.100'),
  (2, 'INFO', 'Order created successfully', 'orders', 2, '192.168.1.101'),
  (3, 'WARNING', 'Failed login attempt', 'authentication', NULL, '192.168.1.102'),
  (4, 'INFO', 'Product updated', 'products', 1, '192.168.1.100'),
  (5, 'ERROR', 'Database connection timeout', 'database', NULL, NULL);

-- Create a materialized view for order summaries
CREATE MATERIALIZED VIEW IF NOT EXISTS order_summary
ENGINE = MergeTree()
ORDER BY order_date
AS
SELECT
  o.id,
  o.order_number,
  u.name as customer_name,
  u.email as customer_email,
  c.name as company_name,
  os.name as status,
  o.order_date,
  o.total_amount,
  count() as item_count
FROM orders o
INNER JOIN users u ON o.user_id = u.id
LEFT JOIN companies c ON o.company_id = c.id
INNER JOIN order_statuses os ON o.status_id = os.id
LEFT JOIN order_items oi ON o.id = oi.order_id
GROUP BY
  o.id,
  o.order_number,
  u.name,
  u.email,
  c.name,
  os.name,
  o.order_date,
  o.total_amount;

-- Create some analytics tables specific to ClickHouse
CREATE TABLE IF NOT EXISTS user_events (
  user_id UInt32,
  event_type String,
  event_data String,
  timestamp DateTime DEFAULT now()
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (user_id, timestamp);

-- Insert some sample events
INSERT INTO user_events (user_id, event_type, event_data) VALUES
  (1, 'login', '{"ip": "192.168.1.100", "device": "desktop"}'),
  (2, 'page_view', '{"page": "/products", "duration": 45}'),
  (3, 'search', '{"query": "laptop", "results": 15}'),
  (1, 'purchase', '{"order_id": 1, "amount": 1549.98}'),
  (4, 'logout', '{"session_duration": 1200}');

-- Create a table for real-time metrics
CREATE TABLE IF NOT EXISTS metrics (
  metric_name String,
  metric_value Float64,
  tags Array(String),
  timestamp DateTime DEFAULT now()
) ENGINE = MergeTree()
PARTITION BY toDate(timestamp)
ORDER BY (metric_name, timestamp)
TTL timestamp + INTERVAL 30 DAY;

-- Insert some sample metrics
INSERT INTO metrics (metric_name, metric_value, tags) VALUES
  ('cpu_usage', 45.2, ['server:web01', 'env:production']),
  ('memory_usage', 72.8, ['server:web01', 'env:production']),
  ('request_count', 1523, ['endpoint:/api/users', 'method:GET']),
  ('response_time', 125.4, ['endpoint:/api/users', 'method:GET']),
  ('active_users', 342, ['app:mobile', 'region:us-west']);