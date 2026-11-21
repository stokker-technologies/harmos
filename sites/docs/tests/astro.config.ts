/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/**
 * Build Isolated Diagram Fixtures
 */

/* === Imports === */

// Standard Library Imports
import { fileURLToPath } from "node:url";

// Third Party Imports
import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";

// Local Imports
import configuration from "../astro.config";

/* === Re-Exports === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Public Functions === */

/* === Private Functions === */

/* === Entry Point === */

export default defineConfig({
  base: configuration.base,
  outDir: "../../../target/docs/fixtures",
  cacheDir: `../../../target/docs/fixture-cache/${process.env.DOCS_FIXTURE ?? "valid"}`,
  markdown: configuration.markdown,
  integrations: [
    starlight({
      title: "Diagram Fixtures",
      customCss: [fileURLToPath(new URL("../src/theme.css", import.meta.url))],
      components: { ThemeProvider: fileURLToPath(new URL("../src/components/ThemeProvider.astro", import.meta.url)) },
    }),
  ],
});
