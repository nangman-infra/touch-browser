FROM rust:1.90-bookworm AS builder

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl git \
  && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://deb.nodesource.com/setup_20.x | bash -
RUN apt-get update \
  && apt-get install -y --no-install-recommends nodejs \
  && rm -rf /var/lib/apt/lists/*
RUN corepack enable

WORKDIR /app

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml tsconfig.base.json biome.json ./
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY adapters ./adapters
COPY contracts ./contracts
COPY core ./core
COPY doc ./doc
COPY fixtures ./fixtures
COPY scripts ./scripts
COPY README.md ./

RUN pnpm install --frozen-lockfile
RUN cargo build --release -p touch-browser-cli

FROM mcr.microsoft.com/playwright:v1.54.0-noble AS runtime

WORKDIR /app

COPY --from=builder /app /app

ENV PATH="/app/node_modules/.bin:${PATH}"
ENV TOUCH_BROWSER_TELEMETRY_DB="/data/telemetry.sqlite"
ENV TOUCH_BROWSER_HEALTHCHECK_COMMAND="touch-browser serve"

VOLUME ["/data"]

RUN ln -sf /app/target/release/touch-browser /usr/local/bin/touch-browser

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 CMD ["node", "scripts/pilot-healthcheck.mjs"]

CMD ["touch-browser", "serve"]
