# Running Migrator CLI

sea-orm-cli generate entity --with-copy-enums -o src/entity --with-serde=both --model-extra-derives 'macros::DeriveDtoModel'
sea-orm-cli migrate fresh
