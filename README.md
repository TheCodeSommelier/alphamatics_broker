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

Set `NATS_TELEMATICS_MAX_BYTES` and `NATS_COMMANDS_MAX_BYTES` as GitHub
Environment variables for both staging and production. `NATS_MAX_AGE_HOURS` is
optional and defaults to 72.

This protects data from container replacement, not EC2 termination. AWS deletes
a root EBS volume on instance termination by default. For data that must survive
instance replacement, disable `DeleteOnTermination` for the root volume and/or
take tested EBS snapshots.

### Migrating from the old bind mount

The first deployment with the named volume starts with an empty volume. The old
bind-mounted directory is not deleted. To retain its JetStream contents, stop
NATS and copy the directory once before deploying this Compose configuration:

```sh
docker compose stop nats
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

How to commit a breaking change:

`feat!: I added something that will blow the older versions to the outer stratosphere`

How to commit a minor change:

`feat: I added something that will not go boom boom...`

How to commit a patch:

`fix: I fixed the things that are making prod go belly up`

> Good luck boys I am pretty sure I will not be able to keep this up for very long...
