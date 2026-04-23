-- V23__drop_bank_sync.sql
-- Retire GoCardless bank-sync surface. After this migration the schema has no
-- bank-sync tables. Historical 'sync' source rows are relabelled so they
-- survive as importable data without the dead source value.

UPDATE ledger_transaction
    SET source = 'csv_import_legacy'
    WHERE source = 'sync';

DROP TABLE IF EXISTS gocardless_institution_cache;
DROP TABLE IF EXISTS bank_account;
