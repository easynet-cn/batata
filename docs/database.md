# Database

Batata supports MySQL, PostgreSQL, and embedded RocksDB (no external database).
Only databases supported by [SeaORM](https://www.sea-ql.org/SeaORM/) can be used;
Oracle is **not** supported.

Schema management is fully automatic: Batata creates and migrates its schema on
startup via SeaORM migrations. Manually importing DDL is neither required nor
recommended.

## MySQL

Point Batata at an empty database — it creates the schema on first startup:

```yaml
batata.sql.init.platform: mysql
batata.db.url: "mysql://user:password@localhost:3306/batata"
batata.db.migration.enabled: true
```

Or via CLI:

```bash
./target/release/batata-server \
  --db-url="mysql://user:password@localhost:3306/batata" \
  --batata.sql.init.platform=mysql \
  --batata.db.migration.enabled=true
```

## PostgreSQL

```yaml
batata.sql.init.platform: postgresql
batata.db.url: "postgres://user:password@localhost:5432/batata"
batata.db.migration.enabled: true
```

## Embedded Mode (No Database)

For development or single-node deployments, no external database is required:

```yaml
batata.sql.init.platform:
batata.standalone: true
```

Or start with: `--batata.sql.init.platform=embedded`

## Database Migration

Automatic migration is provided by the `batata-migration` crate:

```yaml
batata.db.migration.enabled: true
```

When enabled, Batata creates or updates the schema on startup. This is the
recommended approach for new deployments and upgrades.

## Generate Database Entities

```bash
# For MySQL:
sea-orm-cli generate entity \
  -u "mysql://user:pass@localhost:3306/batata" \
  -o ./crates/batata-persistence/src/entity \
  --with-serde both

# For PostgreSQL:
sea-orm-cli generate entity \
  -u "postgres://user:pass@localhost:5432/batata" \
  -o ./crates/batata-persistence/src/entity \
  --with-serde both
```

## Reference DDL (Nacos)

Upstream Nacos schema files are kept under
[`docs/compat/nacos/conf/`](../compat/nacos/conf/) **for comparison only** —
they are not used by Batata at runtime. See
[`docs/compat/nacos/config-mapping.md`](../compat/nacos/config-mapping.md).
