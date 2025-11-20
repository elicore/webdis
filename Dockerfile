# Base stage with cargo-chef installed
FROM rust:alpine3.20 AS chef
RUN apk add --no-cache musl-dev openssl-dev pkgconfig make gcc
RUN cargo install cargo-chef
WORKDIR /app

# Planner stage: Computes the "recipe" (lockfile equivalent)
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage: Caches dependencies and builds the app
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# Build dependencies - this is the cached layer
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY . .
RUN cargo build --release

# Prepare config (same as before)
RUN sed -i 's/"daemonize": true/"daemonize": false/' webdis.prod.json && \
    sed -i 's/"logfile": ".*"/"logfile": null/' webdis.prod.json

# Runtime stage
FROM alpine:3.20

# Install runtime dependencies
# libssl3/libgcc: Required for binary execution (dynamic linking)
# ca-certificates: Required for TLS connections to Redis
RUN apk add --no-cache libssl3 libgcc ca-certificates

# Create a non-root user
RUN adduser -D -g '' webdis

WORKDIR /app

# Copy the build artifact and config
COPY --from=builder /app/target/release/webdis /usr/local/bin/webdis
COPY --from=builder /app/webdis.prod.json /etc/webdis.prod.json

# Use the non-root user
USER webdis

# Expose the port
EXPOSE 7379

# Run the application
CMD ["webdis", "/etc/webdis.prod.json"]
