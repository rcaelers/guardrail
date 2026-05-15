-- Rollback for 20260515000000__add_symbol_annotations
REMOVE FIELD IF EXISTS version ON TABLE symbols;
REMOVE FIELD IF EXISTS channel ON TABLE symbols;
REMOVE FIELD IF EXISTS commit ON TABLE symbols;
REMOVE FIELD IF EXISTS build_tag ON TABLE symbols;
