# syntax=docker/dockerfile:1.23
FROM docker.io/library/rust:alpine AS builder
# add only what's necessary
COPY --parents src/ Cargo.lock Cargo.toml /source/
WORKDIR /source
# compile
RUN --mount=type=cache,target=/usr/local/cargo/registry,id=alpine_cargo_dir \
    --mount=type=cache,target=/source/target,id=cleaner_target \
    cargo build --release --locked && \
    cp /source/target/release/devops-registry-cleaner / && \
    ln -s /devops-registry-cleaner /sleep

# prod image
FROM scratch
USER 1000:1000
COPY --from=builder /devops-registry-cleaner /sleep /
ENTRYPOINT ["/devops-registry-cleaner"]
