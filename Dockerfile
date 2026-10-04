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
    ln -s /devops-registry-cleaner /sleep && \
    ln -s /devops-registry-cleaner /sh

# prod image
FROM scratch
USER 1000:1000
# HACK: coolify is fucking stupid and can't do scheduled tasks if the container has no shell
#       ... so let's just symlink sh to the main program binary
COPY --from=builder /devops-registry-cleaner /sleep /sh /
ENV PATH=/
ENTRYPOINT ["/devops-registry-cleaner"]
