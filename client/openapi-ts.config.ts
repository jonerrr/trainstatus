import { defineConfig } from "@hey-api/openapi-ts";

export default defineConfig({
  input: "http://localhost:5173/api/openapi.json",
  output: "src/client",
  plugins: ["@hey-api/typescript"],
});
