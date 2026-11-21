/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/**
 * Preview File Diagram Fixtures
 */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
import { defineConfig } from "astro/config";

// Local Imports
import configuration from "../../astro.config.ts";

/* === Re-Exports === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Public Functions === */

/* === Private Functions === */

/* === Entry Point === */

export default defineConfig({
  base: configuration.base,
  markdown: configuration.markdown,
  integrations: configuration.integrations,
  outDir: "../../../../../target/docs/live-fixtures",
  cacheDir: "../../../../../target/docs/live-fixture-cache",
});
