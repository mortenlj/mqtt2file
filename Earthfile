VERSION 0.6

FROM rust:1.59

WORKDIR /code

ds-qoriq-sdk:
    WORKDIR /tmp/ds-qoriq-sdk
    RUN wget https://global.download.synology.com/download/ToolChain/toolkit/6.2/qoriq/ds.qoriq-6.2.env.txz
    RUN tar xvf ds.qoriq-6.2.env.txz
    SAVE ARTIFACT /tmp/ds-qoriq-sdk/usr/local/powerpc-e500v2-linux-gnuspe

build-powerpc-unknown-linux-gnuspe:
    ARG target=powerpc-unknown-linux-gnuspe
    COPY --dir +ds-qoriq-sdk/ /ds-qoriq-sdk/
    ENV CARGO_TARGET_POWERPC_UNKNOWN_LINUX_GNUSPE_LINKER=/ds-qoriq-sdk/powerpc-e500v2-linux-gnuspe/bin/powerpc-e500v2-linux-gnuspe-gcc
    ENV RUSTFLAGS="-Ctarget-cpu=e500"
    RUN ${CARGO_TARGET_POWERPC_UNKNOWN_LINUX_GNUSPE_LINKER} --version
    RUN rustup toolchain add nightly
    RUN rustup component add rust-src --toolchain nightly-x86_64-unknown-linux-gnu
    COPY --dir src Cargo.lock Cargo.toml .
    RUN cargo +nightly build -Z build-std --target ${target} --release
    SAVE ARTIFACT target/${target}/release/mqtt2file AS LOCAL target/mqtt2file.${target}
    SAVE IMAGE --cache-hint

prepare:
    RUN cargo install cross
    COPY --dir src Cargo.lock Cargo.toml .

build-tier1:
    FROM +prepare
    ARG target
    WITH DOCKER
        RUN cross build --target ${target} --release
    END
    SAVE ARTIFACT --if-exists target/${target}/release/mqtt2file.exe AS LOCAL target/mqtt2file.${target}.exe
    SAVE ARTIFACT --if-exists target/${target}/release/mqtt2file AS LOCAL target/mqtt2file.${target}
    SAVE IMAGE --cache-hint

build:
    FOR target IN x86_64-unknown-linux-gnu x86_64-pc-windows-gnu aarch64-unknown-linux-gnu
        BUILD +build-tier1 --target=${target}
    END
    BUILD +build-powerpc-unknown-linux-gnuspe
