FROM node:25-alpine@sha256:5209bcaca9836eb3448b650396213dbe9d9a34d31840c2ae1f206cb2986a8543 AS build

RUN npm install -g pnpm

WORKDIR /app
COPY pnpm-workspace.yaml pnpm-lock.yaml ./
COPY frontend/package.json ./frontend/package.json
COPY client/package.json ./client/package.json
RUN pnpm install --frozen-lockfile
COPY frontend ./frontend
COPY client ./client
RUN pnpm install --frozen-lockfile
RUN pnpm --filter frontend build

FROM node:25-alpine@sha256:5209bcaca9836eb3448b650396213dbe9d9a34d31840c2ae1f206cb2986a8543

WORKDIR /app/frontend
COPY --from=build /app/node_modules /app/node_modules
COPY --from=build /app/frontend/node_modules ./node_modules
COPY --from=build /app/frontend/build ./build/
COPY --from=build /app/frontend/package.json ./package.json
EXPOSE 3000
ENV NODE_ENV=production
CMD ["node", "build"]