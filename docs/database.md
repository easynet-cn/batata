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

### Config CAS lock sharding

`EmbeddedPersistService` is used only for the combination **embedded backend
x standalone topology** (`batata.sql.init.platform` empty and
`batata.standalone: true` / `-m standalone`). RocksDB has no compare-and-set,
so a config publish holds a mutex across "read the current md5 -> check
`casMd5` -> write the batch". A single process-wide mutex serialized every
publish, including writes to unrelated dataIds.

The guard is now a set of independent mutexes selected by a stable hash of
`(dataId, groupId, tenantId)`. Gray publishes take the same shard as the main
config for one key, so the two write paths stay mutually exclusive. CAS
semantics, key layout and snapshot format are unchanged. Measured with a
same-session, same-parameter A/B (3 rounds, 15s, c20/t2/w8, fresh store, one
release binary with the shard count injected): at the default count the
single-key load is unchanged within noise, while 64-key and 256-key write
loads improve by +34.8% and +46.3% respectively. Shard counts 16/32/64/128
are within noise of one another on the single-key and 64-key loads; 128 is
+4.6% over 64 only on the 256-key load, so 64 remains the default.

```yaml
batata.persistence.embedded.cas_shards: 64
```

The count defaults to **64** and accepts `1..=1024`. It is also available as
`BATATA_PERSISTENCE_EMBEDDED_CAS_SHARDS` or `--batata.persistence.embedded.cas_shards=N`.
Out-of-range values fall back to 64 with a startup warning. The value is read
**once** when the persistence service is built (no hot reload): changing it
requires a restart. Each process builds its own lock array, so there is no
cluster-wide value to keep in sync; this mode is standalone by definition.
Use it to tune for the deployment's write concurrency:
deployments with a very large config key space may raise it to 128, while `1`
reproduces the old single-lock behaviour for A/B comparison.

This optimization does **not** apply to cluster deployments. Cluster mode also
uses RocksDB, but config writes are ordered by the Raft log in `apply_config`
and need no process-local mutex; MySQL/PostgreSQL deployments use
`SELECT FOR UPDATE` row locks. Both modes ignore this setting. See
[Architecture](architecture.md) and [Configuration](configuration.md) for the
persistence matrix and the key reference.

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
