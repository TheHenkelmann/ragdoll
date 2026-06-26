# Ragdoll documentation

Start here if you want the full picture. For the project overview and pitch, see
the [root README](../README.md).

## Read in order (first time)

1. [getting-started.md](getting-started.md) — zero to your first query
2. [concepts.md](concepts.md) — releases, stages, planes, auth (the mental model)
3. [ingestion.md](ingestion.md) — getting content in
4. [querying.md](querying.md) — getting ranked results out

## Reference (look up as needed)

| Topic | Doc |
|---|---|
| Chunking internals & tuning | [chunking.md](chunking.md) |
| Running the server, UI, models, analytics | [operations.md](operations.md) |
| Environment variables | [configuration.md](configuration.md) |
| Components & data flow | [architecture.md](architecture.md) |
| Common traps, in one place | [pitfalls.md](pitfalls.md) |

## Hands-on

- [tutorial/data_ingestion_tutorial.ipynb](tutorial/data_ingestion_tutorial.ipynb)
  — runnable end-to-end ingest + retrieval against the bundled
  `test_documents/`. Run it from `docs/tutorial/` with the server up.
