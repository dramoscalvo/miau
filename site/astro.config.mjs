import { defineConfig } from "astro/config";

export default defineConfig({
  site: "https://dramoscalvo.github.io",
  base: "/miau",
  output: "static",
  trailingSlash: "always",
  i18n: {
    defaultLocale: "en",
    locales: ["en", "es"],
    routing: { prefixDefaultLocale: false },
  },
});
