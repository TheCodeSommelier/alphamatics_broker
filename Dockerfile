# syntax=docker/dockerfile:1

# Comments are provided throughout this file to help you get started.
# If you need more help, visit the Dockerfile reference guide at
# https://docs.docker.com/go/dockerfile-reference/

# Want to help us make this template better? Share your feedback here: https://forms.gle/ybq9Krt8jtBL3iCk7

ARG RUST_VERSION=1.86.0
ARG APP_NAME=broker

################################################################################
# Create a stage for building the application.

FROM rust:${RUST_VERSION}-bookworm AS build
ARG APP_NAME
WORKDIR /opt/app

# Install host build dependencies.
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --locked --release
RUN cp /opt/app/target/release/${APP_NAME} /bin/server

################################################################################
# Create a new stage for running the application that contains the minimal
# runtime dependencies for the application. This often uses a different base
# image from the build stage where the necessary files are copied from the build
# stage.
#
# The final image uses Wolfi for a smaller, frequently-patched runtime base.
FROM cgr.dev/chainguard/wolfi-base:latest AS final

ENV ADDR="0.0.0.0:4001"
RUN apk add --no-cache ca-certificates-bundle libcrypto3 libssl3

# Create a non-privileged user that the app will run under.
ARG UID=10001
USER ${UID}

# Copy the executable from the "build" stage.
COPY --from=build /bin/server /bin/

# Expose the port that the application listens on.
EXPOSE 4001

# What the container should run when it is started.
CMD [ "/bin/server" ]
