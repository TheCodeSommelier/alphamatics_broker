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
**Deploy NATS** GitHub Actions workflow. Its `nats-compose.yaml` mounts the
dedicated EBS volume at `/nats_data`. The workflow requires the mount and its
`.alphamatics-nats-data-ready` marker before starting NATS. The workflow's
`migrate_legacy_nats` option stops the old broker-project NATS container before
starting the separate NATS project; it does not copy JetStream data from the
old Docker volume. Use that option only for that one-time container migration.
The broker ASG deployment does not update or restart NATS.

### NATS launch templates

`scripts/nats-user-data-launch-template.sh` is self-contained Amazon Linux
2023 ARM64 user data. It identifies the extra EBS disk mapped as `/dev/sdf`,
formats it as XFS only if blank, mounts it by UUID at `/nats_data`, and starts
`nats:2` with JetStream using Docker and Compose. It waits for the NATS
client greeting on TCP:4222. The stage and production NATS launch templates
use this script, `t4g.micro`, their dedicated NATS IAM profiles and security
groups, an 8 GiB root disk, and encrypted data disks of 5 GiB (stage) and
15 GiB (production). The data disks have `DeleteOnTermination=false`.
Creating a launch template does not launch a server or move existing data.

Each new instance starts with an empty JetStream store on its own data disk.
The data disk survives termination, but a replacement does not automatically
reattach it. Do not launch the stage template alongside the current stage NATS
host or use either template in a multi-instance ASG yet. First plan storage
recovery, unique node identities, NATS routes on TCP:6222, and stream replica
counts for a cluster. Until then, keep each environment to one NATS server.

## JetStream persistence and retention

The NATS container stores JetStream data at `/data`, backed by the host's
dedicated EBS mount at `/nats_data`. The current staging host still uses the
Docker volume `alphamatics-broker-nats-data` on its root disk until that data is
copied to the new mount during a controlled NATS stop.

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

The EBS data volume survives NATS container recreation and instance
termination, but a replacement instance needs that volume attached and mounted
before NATS starts. Take tested EBS snapshots; `DeleteOnTermination=false` does
not make a singleton highly available.

Set the NATS limits as GitHub environment variables for each deployment stage.
The ASG deploy writes them, `AWS_SECRET_ID`, `AWS_REGION`, `ENV`, and the immutable
`BROKER_IMAGE` digest to `/opt/broker/.image.env` on each new broker instance.
`compose.yaml` contains only the broker. It does not manage the singleton NATS
host or its JetStream disk. For local Compose runs, provide the required byte
limits through the shell environment or a local `.env` file.

`nats-compose.yaml` is for manual NATS lifecycle on the singleton host. Confirm
`/nats_data` is mounted before using it. The broker's `NATS_URL` in the AWS
secret must resolve to the NATS host's private address, not the old Compose
service name `nats`.

### Moving the existing stage data

The stage host currently has JetStream data in
`/var/lib/docker/volumes/alphamatics-broker-nats-data/_data`. Take a snapshot of
its root EBS volume before migrating. Create and attach the 5 GiB data volume
in the same Availability Zone, format and mount it at `/nats_data`, then stop
NATS, copy the old data including hidden files into the mounted directory, and
start NATS using the updated Compose file. Verify the streams and broker
connection before removing the old Docker volume. Create the
`.alphamatics-nats-data-ready` marker on the mounted disk only after the copy;
the new GitHub workflow requires it before deploying.

## Git commits

Merges to `main` are versioned automatically by `@sladg/release-utils` from
conventional commit messages. The workflow updates `Cargo.toml`, `Cargo.lock`,
and `CHANGELOG.md`, tags the release, creates `release/vX.Y.Z`, builds one ARM64
image, and deploys its digest to the staging broker ASG. It waits for the NLB
TCP:4001 target to become healthy. The broker never runs database migrations.

Production is deployed only by manually running **Promote broker release to
production** in GitHub Actions from `main`. Enter the healthy staged
`release/vX.Y.Z` branch. The workflow checks that the branch's ECR image digest
matches the running staging ASG release and that all its NLB targets are
healthy. It then deploys that same digest to the production broker ASG without
building another image or redeploying staging. Configure the `production`
GitHub environment with its AWS role, region, ECR repository, secret ID, and
NATS limit values. Required reviewers may add a second approval step. The
repository variable `PRODUCTION_PROMOTION_ENABLED` is no longer used.

For a rollback to an older release, select **redeploy_staging** when starting
the manual workflow. It first deploys the selected image to staging and checks
that every staging NLB target is healthy before using that digest in production.
Leave this option off for a normal production promotion.

Deploy the production backend and its Prisma migrations before promoting a
broker release to a new production database. The broker requires the `Unit`
table at startup and cannot create it itself.

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
