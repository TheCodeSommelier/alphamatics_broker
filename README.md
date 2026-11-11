# Broker

## AWS secrets at startup

The broker loads the JSON secret `alphamatics/dev` from AWS Secrets Manager before
connecting to its services. Set `AWS_SECRET_ID` in the runtime environment to use
a different secret. The deploy workflow reads it from the GitHub
`AWS_SECRET_ID` variable for the selected environment and passes it into the
broker container. Values found in the secret take precedence over matching
environment variables; keys absent from the secret still come from the runtime
environment or `.env`. The process needs AWS credentials with
`secretsmanager:GetSecretValue` permission and a configured AWS Region. Startup
fails if the secret cannot be loaded or is not a JSON object of strings.

Code can read the loaded values through `secrets_manager::secrets()` or use
`secrets_manager::var("DATABASE_URL")` for the secret-first environment fallback.
Secret values are not printed at startup.

For Redis, `REDIS_URL` may contain just the host. The broker reads `REDIS_PORT`,
`REDIS_USER`, and `REDIS_PASS` separately from the same secret and adds them to
the in-memory connection settings. An explicit port in `REDIS_URL` takes
precedence over `REDIS_PORT`.

## Separate broker and NATS deployments

The broker and NATS now have separate Compose projects. `compose.yaml` contains
only the broker; `nats-compose.yaml` contains only NATS. Both join the external
`alphamatics-broker-nats` Docker network, so the broker can continue using a
`NATS_URL` with the hostname `nats`. The broker release workflow deploys only
the broker. NATS is updated only by manually running the **Deploy NATS**
workflow for the desired environment.

The broker workflow publishes one image tag with `linux/amd64` and
`linux/arm64` variants. Docker pulls the variant matching the EC2 host. The
official `nats:2` image also provides ARM64, so the NATS Compose file needs no
platform override.

On an existing host, merge the broker change first. That deploy creates the
shared network and attaches the old, running NATS container to it before
recreating the broker. Check that the broker reconnects to NATS, then run
**Deploy NATS** with `migrate_legacy_nats` enabled. This stops and removes the
old NATS container and starts NATS from `nats-compose.yaml` using the same
named volume. Expect a brief NATS interruption during this one-time migration.
After that, leave `migrate_legacy_nats` off for routine NATS deploys.

Before the migration, confirm the broker's `NATS_URL` uses `nats` as its host.
The workflow checks that the existing data volume is present before stopping
the old container. On a fresh host, create the shared resources once before
the first deploy:

```sh
docker network create alphamatics-broker-nats
docker volume create alphamatics-broker-nats-data
```

For manual restarts on the host, use the two Compose files separately:

```sh
cd /opt/broker
docker compose -f nats-compose.yaml up -d nats
docker compose -f compose.yaml --env-file .image.env up -d broker
```

Do not use `docker compose down` with the legacy broker project before the old
NATS container has been migrated: it can stop NATS. Do not use
`--remove-orphans` while that old container still exists.

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

Set the NATS limits as GitHub environment variables for each deployment stage.
The deploy workflow writes them, `AWS_SECRET_ID`, `ENV`, and `BROKER_IMAGE` to
`/opt/broker/.image.env`; Compose passes the configuration values to the broker
container. The host `/opt/broker/.env` file is no longer used. For local Compose
runs, provide the required byte limits through the shell environment or a local
`.env` file. `NATS_MAX_AGE_HOURS` is optional and defaults to 72.

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
