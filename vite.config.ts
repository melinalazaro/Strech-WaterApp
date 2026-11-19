import { defineConfig } from "vite";

export default defineConfig({
  // Keep the placeholder as a local file rather than a data URI: the production
  // CSP permits only local image assets and does not need data: permissions.
  build: { assetsInlineLimit: 0 },
});
