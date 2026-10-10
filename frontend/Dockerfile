FROM ghcr.io/pnpm/pnpm:12@sha256:fd95b696aff515e5c58f59248435c29d783fce85cfc9a28c9db3b994bffa44dd AS base

# TODO: find a better way to make container rootless with pnpm base image
ENV PNPM_HOME="/app/.pnpm-global"
ENV PATH="$PNPM_HOME/bin:$PATH"

# renovate: datasource=node-version depName=node
RUN pnpm runtime set node 26 -g

RUN chmod -R a+rX "$PNPM_HOME"

COPY . /app
WORKDIR /app

FROM base AS prod-deps
RUN --mount=type=cache,id=pnpm,target=/var/cache/pnpm \
    pnpm install --store-dir /var/cache/pnpm --prod --frozen-lockfile

FROM base AS build
RUN --mount=type=cache,id=pnpm,target=/var/cache/pnpm \
    pnpm install --store-dir /var/cache/pnpm --frozen-lockfile
RUN pnpm run build

FROM base

ENV NODE_ENV=production

COPY --chown=10001:10001 --from=prod-deps /app/node_modules /app/node_modules
COPY --chown=10001:10001 --from=build /app/build /app/build

USER 10001:10001

CMD ["node", "build"]