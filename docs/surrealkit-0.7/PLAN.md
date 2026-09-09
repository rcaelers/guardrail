# Moving surrealkit from 0.5.8 to 0.7.0

Status: **not started.** Investigated 2026-09-07, nothing committed beyond this
directory. The Containerfile is still pinned to 0.5.8 and every rollout manifest
is still in 0.5.8 format.

## Why we are stuck on 0.5.8

`Containerfile` pins `cargo-binstall --version 0.5.8 surrealkit`. Bumping it
breaks the production schema deployment, for two independent reasons.

### 1. The manifest format changed

0.7.0 cannot parse any of the 12 manifests in `database/rollouts/`:

```
TOML parse error at line 8 -- missing field `sql`
```

0.6.3 reads 1 of 12. The break is between 0.5.8 and 0.6.x.

Converting is mechanical -- `kind = "apply_schema"` becomes
`kind = "apply_files"`, and the `entities = []` line on that step goes away --
and all 12 then parse under 0.7.0. It is one-way: **0.5.8 cannot read the
converted manifests**, so the manifests and the image pin must change together.

### 2. Every table is defined twice

This is the real blocker, and it is our schema, not a surrealkit bug.

`database/schema/guardrail.surql` defines each table once for its structure and
again at the bottom to attach permissions:

```surql
DEFINE TABLE annotations SCHEMAFULL;                 -- structure
...
DEFINE TABLE OVERWRITE annotations SCHEMAFULL        -- permissions
    PERMISSIONS FOR select WHERE ...;
```

All 13 tables follow that pattern. 0.7.0 added a unique index on its entity
catalog, so the second definition collides:

```
Error: Database index `by_ns_key` already contains ['schema', 'table::annotations']
```

This happens on a **completely fresh database**, so it is not leftover 0.5.8
state. Reduced to a minimum:

| schema | 0.7.0 `rollout baseline` |
| --- | --- |
| define, then `DEFINE TABLE OVERWRITE` | collision |
| single definition carrying `PERMISSIONS` | works |

Under 0.7.0 `sync` still works, but `rollout baseline` and `rollout complete`
both fail. Production runs `ops/surrealkit-rollout-start.sh` and
`ops/surrealkit-rollout-complete.sh`, so a bump would apply the schema in
`start` and then wedge the rollout in `running_complete` on every change.

## What is already done

`guardrail.consolidated.surql.txt` in this directory is the consolidated
schema: each table defined once, carrying its own `PERMISSIONS`, and the
trailing "Table permissions" block removed. 202 statements become 189.

It was verified equivalent, not just eyeballed. Both schemas were applied to
separate fresh SurrealDB instances and the results compared:

- `INFO FOR DB` identical
- `INFO FOR TABLE` identical for all 13 tables, permissions included

and with it, 0.7.0 does everything it could not before:

```
setup                        ok
sync                         applied
rollout baseline             Seeded 183 managed objects
rollout plan/start/complete  completed
```

It is saved as `.surql.txt` on purpose: anything named `*.surql` under
`database/schema/` is picked up by `surrealkit sync`, and a second schema file
there would be applied to the database.

**It is a snapshot from 2026-09-07 and tracks `guardrail.surql` at commit
8e77fd7.** If the schema has moved on, regenerate rather than trusting it; the
transformation is in the "Regenerating" section below.

## Remaining work

Do it in one change; the pieces are not independently deployable.

1. **Consolidate the schema.** Replace `database/schema/guardrail.surql` with
   the consolidated form. Re-verify equivalence against the then-current schema
   using the two-database diff above -- this is the RLS layer and a mistake
   silently widens access.

2. **Decide manifests: convert or start fresh.**
   - *Start fresh* (recommended). The 12 historical rollouts are all applied in
     production; their only remaining job is the hash chain. Delete them, take a
     fresh `rollout baseline` on the consolidated schema, and let 0.7.0 generate
     everything from then on.
   - *Convert.* Rewrite all 12 to `apply_files`. Keeps history readable and
     nothing else. The schema hashes are unaffected either way: 0.5.8 and 0.7.0
     compute the same values (both produced `0ce597b5...` -> `ad0fc70a...` for
     the same change), and `__rollout` records store schema hashes rather than
     manifest checksums, so recorded state stays valid.

3. **Bump the pin** in `Containerfile` to 0.7.0.

4. **Production cutover.** Prod's `__entity` catalog was written by 0.5.8 and
   holds the duplicate rows, so it has to be cleared and re-baselined in the
   same window as the image bump. This is the only step that cannot simply be
   rolled back, and the one to rehearse against a copy of the production
   database first.

5. **Check the jobs still behave.** Both scripts select the lexicographically
   last manifest (`find | sort | tail -n 1`) and run `surrealkit rollout lint`
   first under `set -eu`, so a manifest the deployed binary cannot parse aborts
   the job before it touches anything. After a fresh baseline there may be no
   manifest at all; both scripts already handle that ("No rollout manifests
   found ... skipping").

## Regenerating the consolidated schema

The permissions block is a self-contained run of `DEFINE TABLE OVERWRITE`
statements at the end of the file. For each, move the text after the table name
into that table's earlier `DEFINE TABLE <name> SCHEMAFULL;`, then delete the
block and its heading.

To re-verify equivalence, apply the old and new schema to two fresh SurrealDB
containers and diff `INFO FOR DB` and `INFO FOR TABLE <t>` for every table.
They must be identical.

## Traps worth remembering

- **Generate manifests with the deployed binary, not the local one.** The local
  `surrealkit` is 0.7.0 and emits `apply_files`, which today's deployed 0.5.8
  cannot read. Until the bump lands, use the deployed image:
  `docker run --rm -v "$PWD:/repo" -w /repo --entrypoint surrealkit \
  ghcr.io/rcaelers/guardrail-schema-sync:<deployed-tag> rollout plan --name X`
- **`--folder` is ignored when it follows the subcommand.** `rollout lint X
  --folder /somewhere` silently reads `./database` instead. Run from the
  directory whose `database/` you mean.
- **`rollout plan` refuses whenever the snapshot is stale**, naming entities
  that have nothing to do with your change. Refresh it by running
  `setup`/`sync`/`rollout baseline` against a throwaway database and copying
  `database/snapshots/*.json` back. `baseline` is one-shot per database, which
  is why it has to be a fresh one.
- **A hand-written manifest leaves the snapshot stale**, so the next `plan`
  refuses again for the same reason.
