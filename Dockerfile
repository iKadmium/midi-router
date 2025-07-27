# Multi-stage Dockerfile for Rust MIDI Router
# Stage 1: Build the application
FROM rust:latest as builder

# Use build arguments to determine target architecture
ARG TARGETPLATFORM
ARG BUILDPLATFORM

# Install musl target for static linking based on target platform
RUN case "$TARGETPLATFORM" in \
    "linux/amd64") RUST_TARGET="x86_64-unknown-linux-musl" ;; \
    "linux/arm64") RUST_TARGET="aarch64-unknown-linux-musl" ;; \
    "linux/arm/v7") RUST_TARGET="armv7-unknown-linux-musleabihf" ;; \
    *) echo "Unsupported platform: $TARGETPLATFORM" && exit 1 ;; \
    esac && \
    rustup target add $RUST_TARGET && \
    echo "RUST_TARGET=$RUST_TARGET" > /tmp/rust_target

# Create app directory
WORKDIR /app

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src/ ./src/

# Create a non-root user for running the application
RUN groupadd -r appuser && useradd -r -g appuser -s /bin/false appuser

# Build the application in release mode with static linking
RUN . /tmp/rust_target && cargo build --release --target $RUST_TARGET

# Copy the binary from the builder stage
RUN . /tmp/rust_target && cp /app/target/$RUST_TARGET/release/midi-router /tmp/midi-router

# Stage 2: Create minimal runtime image
FROM scratch

# Copy passwd and group files to define the user
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

# Copy the binary from the builder stage
COPY --from=builder /tmp/midi-router /midi-router

# Copy config directory (if needed at runtime)
COPY config/ /config/

# Run as the created user
USER appuser:appuser

# Set the binary as the entrypoint
ENTRYPOINT ["/midi-router"]