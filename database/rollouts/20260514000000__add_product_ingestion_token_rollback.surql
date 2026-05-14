-- Rollback for 20260514000000__add_product_ingestion_token
REMOVE FIELD IF EXISTS product_token ON TABLE products;
REMOVE INDEX IF EXISTS idx_products_product_token ON TABLE products;
