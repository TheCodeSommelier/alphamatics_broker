#!/bin/bash
set -euo pipefail

# Fresh Amazon Linux 2023 ARM64 host for one NATS JetStream server.
# This is a singleton bootstrap. Do not set an NATS ASG above one instance
# until clustering, node identities, route access, and stream replicas exist.
exec > >(tee -a /var/log/alphamatics-nats-bootstrap.log) 2>&1

dnf install -y docker xfsprogs
systemctl enable --now docker
docker info >/dev/null

mkdir -p /usr/local/lib/docker/cli-plugins
curl -fSL \
  https://github.com/docker/compose/releases/download/v2.40.3/docker-compose-linux-aarch64 \
  -o /usr/local/lib/docker/cli-plugins/docker-compose
chmod +x /usr/local/lib/docker/cli-plugins/docker-compose
docker compose version

# The launch template maps its dedicated EBS data disk as /dev/sdf. Amazon
# Linux exposes a stable symlink to the underlying NVMe device at that path.
data_device=''
for ((attempt=0; attempt<60; attempt++)); do
  if [[ -b /dev/sdf ]]; then
    data_device="$(readlink -f /dev/sdf)"
    break
  fi
  sleep 2
done
[[ -n "$data_device" && -b "$data_device" ]] || {
  echo 'NATS data EBS volume is missing at /dev/sdf' >&2
  exit 1
}

# Fail closed if the device mapping unexpectedly points at the root disk.
root_source="$(findmnt -n -o SOURCE /)"
root_disk="/dev/$(lsblk -n -o PKNAME "$root_source" | head -n 1)"
[[ "$data_device" != "$root_disk" &&
   "$(lsblk -n -o TYPE "$data_device" | head -n 1)" == disk &&
   "$(lsblk -n -o NAME "$data_device" | wc -l)" -eq 1 ]] || {
  echo 'NATS data device is not a separate whole disk' >&2
  exit 1
}

filesystem="$(blkid -s TYPE -o value "$data_device" || true)"
if [[ -z "$filesystem" ]]; then
  mkfs.xfs "$data_device"
elif [[ "$filesystem" != xfs ]]; then
  echo "NATS data device has an unexpected filesystem: $filesystem" >&2
  exit 1
fi

data_uuid="$(blkid -s UUID -o value "$data_device")"
mkdir -p /nats_data
if ! grep -q "^UUID=$data_uuid[[:space:]]" /etc/fstab; then
  printf 'UUID=%s /nats_data xfs defaults 0 2\n' "$data_uuid" >> /etc/fstab
fi
if ! mountpoint -q /nats_data; then
  mount /nats_data
fi
mountpoint -q /nats_data
# The launch template provisions a new data disk. Mark it ready only after
# the mount succeeds; existing hosts get this marker after their data is copied.
touch /nats_data/.alphamatics-nats-data-ready

mkdir -p /opt/broker

cat > /opt/broker/nats-compose.yaml <<'COMPOSE'
name: alphamatics-nats

services:
  nats:
    container_name: nats
    image: nats:2
    command: ["-js", "-sd", "/data"]
    volumes:
      - /nats_data:/data
    ports:
      - "4222:4222"
      - "127.0.0.1:8222:8222"
    restart: unless-stopped
COMPOSE

cd /opt/broker
docker compose -f nats-compose.yaml config --quiet
docker compose -f nats-compose.yaml pull nats
docker compose -f nats-compose.yaml up -d nats

# NATS sends an INFO frame as soon as its client port is ready.
for ((attempt=0; attempt<60; attempt++)); do
  if (
    exec 3<>/dev/tcp/127.0.0.1/4222
    IFS= read -r -t 3 line <&3
    [[ "$line" == INFO\ * ]]
  ) 2>/dev/null; then
    echo 'NATS client port is ready'
    exit 0
  fi
  sleep 5
done

echo 'NATS did not become ready on TCP:4222' >&2
docker compose -f nats-compose.yaml logs --tail 80 nats >&2
exit 1
