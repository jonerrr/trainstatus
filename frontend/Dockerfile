FROM node:25-alpine@sha256:bdf2cca6fe3dabd014ea60163eca3f0f7015fbd5c7ee1b0e9ccb4ced6eb02ef4 AS build

RUN npm install -g pnpm

WORKDIR /app
COPY package.json pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY . .
RUN pnpm build
RUN pnpm prune --production

FROM node:25-alpine@sha256:bdf2cca6fe3dabd014ea60163eca3f0f7015fbd5c7ee1b0e9ccb4ced6eb02ef4

WORKDIR /app
COPY --chown=node:node --from=build /app/build build/
COPY --chown=node:node --from=build /app/node_modules node_modules/
COPY --chown=node:node package.json .
# EXPOSE 3000
ENV NODE_ENV=production
USER node
CMD ["node", "build"]