#!/bin/bash
set -euo pipefail

exec > >(tee -a /var/log/alphamatics-bootstrap.log) 2>&1

# Host prerequisites only. deploy.sh appends the immutable broker release.
dnf install -y docker
systemctl enable --now docker
docker info >/dev/null

# Install Docker Compose for the ARM64 launch template.
mkdir -p /usr/local/lib/docker/cli-plugins

curl -fSL \
  https://github.com/docker/compose/releases/download/v2.40.3/docker-compose-linux-aarch64 \
  -o /usr/local/lib/docker/cli-plugins/docker-compose

chmod +x /usr/local/lib/docker/cli-plugins/docker-compose

# Verify dependencies
docker --version
docker compose version
command -v aws

echo "AlphaMatics broker bootstrap completed successfully"

# A launch template version created by deploy.sh appends Compose, the pinned
# ECR image digest, and the broker startup command.
