# Broker

## JetStream persistence and retention

The NATS container stores JetStream data at `/data`. Docker Compose requires
`NATS_DATA_DIR` to point to an existing host directory mounted from the encrypted
EBS data volume; it deliberately does not create the directory or fall back to
the EC2 root disk.

Configure Compose and the broker with:

```dotenv
NATS_DATA_DIR=/mnt/nats
NATS_MAX_AGE_HOURS=72
NATS_TELEMATICS_MAX_BYTES=<required byte limit>
NATS_COMMANDS_MAX_BYTES=<required byte limit>
```

`NATS_MAX_AGE_HOURS` defaults to 72. Both byte limits are required and must be
positive integers. Size them from measured traffic and EBS capacity, leaving
room for JetStream storage overhead and the 50/70/85% disk alarms. The broker
creates or updates both streams with file storage and enforces the age, byte,
and existing 10,000,000-message limits; whichever limit is reached first wins.

The host must mount the EBS volume and create `NATS_DATA_DIR` with permissions
that allow the NATS container to write before running `docker compose up`.
The deployment workflow also refuses to start NATS unless `NATS_DATA_DIR` is a
real mount point, preventing an unmounted EBS volume from silently falling back
to the EC2 root disk.

Set `NATS_DATA_DIR`, `NATS_TELEMATICS_MAX_BYTES`, and
`NATS_COMMANDS_MAX_BYTES` as GitHub Environment variables for both staging and
production. `NATS_MAX_AGE_HOURS` is optional and defaults to 72.

## Git commits

How to commit a breaking change:

`feat!: I added something that will blow the older versions to the outer stratosphere`

How to commit a minor change:

`feat: I added something that will not go boom boom...`

How to commit a patch:

`fix: I fixed the things that are making prod go belly up`

> Good luck boys I am pretty sure I will not be able to keep this up for very long...
