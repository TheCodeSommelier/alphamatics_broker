# Broker

## Database schema ownership

The backend owns all shared PostgreSQL schema changes through its Prisma migration workflow. The broker reads the existing `Unit` table and does not run database migrations or create schema objects. Do not run a separate migration tool against the shared database from this repository.

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

## Broker and NATS deployments

The broker runs in its own ASG. Its `compose.yaml` contains only the broker and
connects to the existing singleton NATS host through the `NATS_URL` in the AWS
secret. That URL must use the NATS host's private address or DNS name, not the
old Docker service name `nats`.

NATS remains on its existing host and is managed separately by the manual
**Deploy NATS** GitHub Actions workflow. Its `nats-compose.yaml` uses the
existing `alphamatics-broker-nats-data` volume. The workflow's
`migrate_legacy_nats` option stops the old broker-project NATS container before
starting the separate NATS project. Use that option only for the one-time
migration after checking the current container and volume on the NATS host.
The broker ASG deployment does not update or restart NATS.

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
The ASG deploy writes them, `AWS_SECRET_ID`, `AWS_REGION`, `ENV`, and the immutable
`BROKER_IMAGE` digest to `/opt/broker/.image.env` on each new broker instance.
`compose.yaml` contains only the broker. It does not manage the singleton NATS
host or its JetStream volume. For local Compose runs, provide the required byte
limits through the shell environment or a local `.env` file.

`nats-compose.yaml` is for manual NATS lifecycle on the existing singleton host.
Before using it, inspect the running NATS container and its Compose project;
do not start a second NATS container on the same port. Confirm that host already
uses the `alphamatics-broker-nats-data` volume. The broker's `NATS_URL` in the AWS secret
must resolve to the singleton NATS private address, not the old Compose service
name `nats`. Do not run the broker ASG deployment against the NATS host.

This protects data from container replacement, not EC2 termination. AWS deletes
a root EBS volume on instance termination by default. For data that must survive
instance replacement, disable `DeleteOnTermination` for the root volume and/or
take tested EBS snapshots.

### One-time JetStream data copy from the old bind mount

If NATS still uses an old bind mount, stop its running container using the
currently deployed Compose project and copy the data into the named volume
before running the manual NATS workflow:

```sh
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
and `CHANGELOG.md`, tags the release, creates `release/vX.Y.Z`, builds one ARM64
image, and deploys its digest to the staging broker ASG. It waits for the NLB
TCP:4001 target to become healthy. The broker never runs database migrations.

Production promotion is disabled until the repository variable
`PRODUCTION_PROMOTION_ENABLED` is set to `true`. Before enabling it, configure
the `production` GitHub environment with required reviewers and its AWS role,
region, ECR repository, secret ID, and NATS limit values. A successful staging
job then reaches the GitHub environment approval gate; approval deploys the
same image digest to the production broker ASG.

The staging broker security group must allow TCP:4001 from its NLB security
group and egress to the NATS host on TCP:4222, PostgreSQL on TCP:5432, Redis
on the port in the staging secret, and AWS endpoints on TCP:443. The NATS host
security group must allow TCP:4222 from the staging broker security group.
The production environment needs its own corresponding rules.

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

To roll back, use the previous working launch template version with an ASG
instance refresh, then verify the NLB target health. Retain the previous
version until the new release is verified.

> Good luck boys I am pretty sure I will not be able to keep this up for very long...
