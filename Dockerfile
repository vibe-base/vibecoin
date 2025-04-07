FROM rust:1.67-slim-bullseye as builder

WORKDIR /usr/src/vibecoin

# Install dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    build-essential \
    libssl-dev \
    pkg-config \
    libclang-dev \
    cmake \
    git \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the source code
COPY . .

# Build the application
RUN cargo build --release

# Create a new stage with a minimal image
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    libssl1.1 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /usr/src/vibecoin/target/release/vibecoin /usr/local/bin/vibecoin

# Create a directory for the data
RUN mkdir -p /data/vibecoin

# Set the working directory
WORKDIR /data/vibecoin

# Expose the ports
EXPOSE 30333 8545 9100

# Set the entrypoint
ENTRYPOINT ["vibecoin"]

# Set the default command
CMD ["--config", "/data/vibecoin/config.toml"]
