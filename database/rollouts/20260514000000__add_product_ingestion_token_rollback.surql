-- Rollback for 20260514000000__add_product_ingestion_token
REMOVE FIELD IF EXISTS ingestion_token ON TABLE products;
REMOVE INDEX IF EXISTS idx_products_ingestion_token ON TABLE products;
