VERSION 0.6

FROM rust:1.59

WORKDIR /code

# Constants, do not override
ARG cross_version=0.2.1

ds-qoriq-sdk:
    WORKDIR /tmp/ds-qoriq-sdk
    RUN wget --no-verbose https://global.download.synology.com/download/ToolChain/toolkit/6.2/qoriq/ds.qoriq-6.2.env.txz
    RUN tar xf ds.qoriq-6.2.env.txz
    SAVE ARTIFACT /tmp/ds-qoriq-sdk/usr/local/powerpc-e500v2-linux-gnuspe

build-powerpc-unknown-linux-gnuspe:
    ARG target=powerpc-unknown-linux-gnuspe
    ARG version

    COPY --dir +ds-qoriq-sdk/ /ds-qoriq-sdk/

    ENV PKG_CONFIG_SYSROOT_DIR=/ds-qoriq-sdk/usr/local/powerpc-e500v2-linux-gnuspe/powerpc-e500v2-linux-gnuspe/sysroot/
    ENV TOOLKIT_BIN=/ds-qoriq-sdk/powerpc-e500v2-linux-gnuspe/bin
    ENV CARGO_TARGET_POWERPC_UNKNOWN_LINUX_GNUSPE_LINKER=${TOOLKIT_BIN}/powerpc-e500v2-linux-gnuspe-gcc

    ENV CMAKE_C_COMPILER=${TOOLKIT_BIN}/powerpc-e500v2-linux-gnuspe-gcc
    ENV CMAKE_CXX_COMPILER=${TOOLKIT_BIN}/powerpc-e500v2-linux-gnuspe-g++
    ENV CMAKE_ASM_COMPILER=${TOOLKIT_BIN}/powerpc-e500v2-linux-gnuspe-gcc
    ENV CC_powerpc_unknown_linux_gnuspe=${TOOLKIT_BIN}/powerpc-e500v2-linux-gnuspe-gcc

    ENV RUSTFLAGS="-Ctarget-cpu=e500"

    RUN apt-get --yes update && apt-get --yes install cmake
    RUN ${CARGO_TARGET_POWERPC_UNKNOWN_LINUX_GNUSPE_LINKER} --version
    RUN rustup toolchain add nightly
    RUN rustup component add rust-src --toolchain nightly-x86_64-unknown-linux-gnu

    COPY --dir src Cargo.lock Cargo.toml .
    RUN cargo +nightly build -Z build-std --target ${target} --release

    SAVE ARTIFACT target/${target}/release/mqtt2file AS LOCAL target/mqtt2file.${version}.${target}
    SAVE IMAGE --cache-hint

prepare:
    RUN cargo install cross --version ${cross_version}
    COPY --dir src Cargo.lock Cargo.toml .

build-tier1:
    FROM +prepare
    ARG target
    ARG version

    WITH DOCKER \
        --pull rustembedded/cross:${target}-${cross_version}
        RUN cross build --target ${target} --release
    END
    SAVE ARTIFACT --if-exists target/${target}/release/mqtt2file.exe AS LOCAL target/mqtt2file.${version}.${target}.exe
    SAVE ARTIFACT --if-exists target/${target}/release/mqtt2file AS LOCAL target/mqtt2file.${version}.${target}
    SAVE IMAGE --cache-hint

build:
    FOR target IN x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
        BUILD +build-tier1 --target=${target}
    END
    BUILD +build-powerpc-unknown-linux-gnuspe
