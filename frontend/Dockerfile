FROM node:25-alpine as build

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

FROM node:25-alpine

WORKDIR /app/frontend
COPY --from=build /app/node_modules /app/node_modules
COPY --from=build /app/frontend/node_modules ./node_modules
COPY --from=build /app/frontend/build ./build/
COPY --from=build /app/frontend/package.json ./package.json
EXPOSE 3000
ENV NODE_ENV=production
CMD ["node", "build"]