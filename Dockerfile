# Static-musl build of the peiosutils binaries for Provium guest
# injection.
#
# Why Docker: the Nix-managed rustc on host doesn't expose the musl
# target std cleanly. Same pattern as pkm/crates/libp-test/Dockerfile.
#
# The build context is the peios workspace root (/home/jack/projects/peios/).
# The pu_* crates depend -- via uucore and pu_revstrm -- on the libp-rs
# workspace, which in turn consumes the generated peios-uapi crate at
# pkm/uapi/rust. All three trees are copied into the context.
#
# Build (from /home/jack/projects/peios/):
#   docker build -f peiosutils/Dockerfile \
#                -o type=local,dest=peiosutils/dist .
# Output:
#   peiosutils/dist/cp
#   peiosutils/dist/logonse
#   peiosutils/dist/ls
#   peiosutils/dist/mkdir
#   peiosutils/dist/mkfifo
#   peiosutils/dist/mknod
#   peiosutils/dist/mv
#   peiosutils/dist/nohup
#   peiosutils/dist/regman
#   peiosutils/dist/revstrm
#   peiosutils/dist/rm
#   peiosutils/dist/sd
#   peiosutils/dist/shred
#   peiosutils/dist/test
#   peiosutils/dist/token
#   peiosutils/dist/touch

FROM rust:1-alpine AS builder

RUN apk add --no-cache musl-dev

WORKDIR /build
# Path-dep closure of the pu_* crates after the 2026-05 uapi migration:
# the libp-rs workspace (libp-* crates inherit its package fields) and the
# standalone generated peios-uapi crate it consumes from pkm/uapi/rust.
# Copy manifests + sources only -- not the 350M libp-rs/target.
COPY pkm/uapi/rust/ pkm/uapi/rust/
COPY libp-rs/Cargo.toml libp-rs/Cargo.toml
COPY libp-rs/crates/ libp-rs/crates/
COPY peiosutils/ peiosutils/

WORKDIR /build/peiosutils
RUN cargo build --release --target x86_64-unknown-linux-musl \
    -p pu_cp --bin cp \
    -p pu_logonse --bin logonse \
    -p pu_ls --bin ls \
    -p pu_mkdir --bin mkdir \
    -p pu_mkfifo --bin mkfifo \
    -p pu_mknod --bin mknod \
    -p pu_mv --bin mv \
    -p pu_nohup --bin nohup \
    -p pu_regman --bin regman \
    -p pu_revstrm --bin revstrm \
    -p pu_rm --bin rm \
    -p pu_sd --bin sd \
    -p pu_shred --bin shred \
    -p pu_test --bin test \
    -p pu_token --bin token \
    -p pu_touch --bin touch

FROM scratch
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/cp /cp
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/logonse /logonse
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/ls /ls
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/mkdir /mkdir
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/mkfifo /mkfifo
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/mknod /mknod
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/mv /mv
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/nohup /nohup
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/regman /regman
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/revstrm /revstrm
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/rm /rm
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/sd /sd
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/shred /shred
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/test /test
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/token /token
COPY --from=builder /build/peiosutils/target/x86_64-unknown-linux-musl/release/touch /touch
