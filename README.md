# Broker

## JetStream persistence and retention

The NATS container stores JetStream data at `/data`, backed by the stable Docker
named volume `alphamatics-broker-nats-data`. Docker keeps the volume in its data
root on the host filesystem, so on a normal EBS-backed EC2 instance the data
lives on the instance's root EBS volume. No separately mounted EBS data volume
or `NATS_DATA_DIR` setting is required.

Configure Compose and the broker with:

```dotenv
NATS_MAX_AGE_HOURS=72
NATS_TELEMATICS_MAX_BYTES=<required byte limit>
NATS_COMMANDS_MAX_BYTES=<required byte limit>
```

`NATS_MAX_AGE_HOURS` defaults to 72. Both byte limits are required and must be
positive integers. Size them from measured traffic and EBS capacity, leaving
room for JetStream storage overhead and the 50/70/85% disk alarms. The broker
creates or updates both streams with file storage and enforces the age, byte,
and existing 10,000,000-message limits; whichever limit is reached first wins.

The named volume survives container recreation and normal deployments. Do not
run `docker compose down --volumes` or remove/prune this volume unless deleting
the JetStream data is intentional.

Set these values in `/opt/broker/.env` on each host. That file supplies the
broker container's runtime environment; `.image.env` is deployment metadata and
contains only `BROKER_IMAGE`. `NATS_MAX_AGE_HOURS` is optional and defaults to
72.

Use `-f compose.yaml` for manual lifecycle commands when the host still has a
legacy `docker-compose.yml`, for example:

```sh
docker compose -f compose.yaml down
docker compose -f compose.yaml --env-file .image.env up -d broker
```

This protects data from container replacement, not EC2 termination. AWS deletes
a root EBS volume on instance termination by default. For data that must survive
instance replacement, disable `DeleteOnTermination` for the root volume and/or
take tested EBS snapshots.

### Migrating from the old bind mount

The first deployment with the named volume starts with an empty volume. The old
bind-mounted directory is not deleted. To retain its JetStream contents, stop
NATS and copy the directory once before deploying this Compose configuration:

```sh
docker compose -f compose.yaml stop nats
docker volume create alphamatics-broker-nats-data
docker run --rm \
  --mount type=bind,src=/previous/nats/data,dst=/from,readonly \
  --mount type=volume,src=alphamatics-broker-nats-data,dst=/to \
  alpine sh -c 'cp -a /from/. /to/'
```

Replace `/previous/nats/data` with the previous `NATS_DATA_DIR`, then run the
normal deployment. Verify the old directory and the new stream state before
removing any old storage.

## Git commits

Merges to `main` are versioned automatically by `@sladg/release-utils` from
conventional commit messages. The workflow updates `Cargo.toml`, `Cargo.lock`,
and `CHANGELOG.md`, tags the release, creates `release/vX.Y.Z`, and deploys it
to staging. Production deploys and rollbacks are manual workflow runs from an
existing release branch.

| Commit | Bump |
|---|---|
| contains `BREAKING CHANGE` (subject or body) | major |
| `feat`, `perf`, `refactor`, `style`, `test`, `ci`, `build` | minor |
| `fix`, `chore`, `revert`, `docs` | patch |
| anything else | ignored |

Mark a breaking change with a footer; the release utility does not recognise
the `feat!:` shorthand:

```sh
git commit -m "feat: change the wire protocol" \
           -m "BREAKING CHANGE: older device firmware is no longer supported"
```

To roll back, run the *Deploy Broker* workflow from the last good
`release/vX.Y.Z` branch and select the target environment.

> Good luck boys I am pretty sure I will not be able to keep this up for very long...
