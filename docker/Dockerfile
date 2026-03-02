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
RUN cargo build --release -p redis-web --bins

# Prepare config (same as before)
RUN sed -i 's/"daemonize": true/"daemonize": false/' redis-web.prod.json && \
    sed -i 's#"logfile": ".*"#"logfile": null#' redis-web.prod.json

# Runtime stage
FROM alpine:3.20

# Install runtime dependencies
# libssl3/libgcc: Required for binary execution (dynamic linking)
# ca-certificates: Required for TLS connections to Redis
RUN apk add --no-cache libssl3 libgcc ca-certificates

# Create a non-root user
RUN adduser -D -g '' redisweb

WORKDIR /app

# Copy the build artifacts and configs
COPY --from=builder /app/target/release/redis-web /usr/local/bin/redis-web
COPY --from=builder /app/target/release/webdis /usr/local/bin/webdis
COPY --from=builder /app/redis-web.prod.json /etc/redis-web.prod.json
COPY --from=builder /app/webdis.prod.json /etc/webdis.prod.json

# Use the non-root user
USER redisweb

# Expose the port
EXPOSE 7379

# Run the application
CMD ["redis-web", "/etc/redis-web.prod.json"]
