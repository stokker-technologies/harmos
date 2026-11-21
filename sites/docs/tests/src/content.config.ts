/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/**
 * Load Diagram Fixtures
 */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
import { docsSchema } from "@astrojs/starlight/schema";
import { glob } from "astro/loaders";
import { defineCollection } from "astro:content";

// Local Imports

/* === Re-Exports === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Public Functions === */

/* === Private Functions === */

/* === Entry Point === */

export const collections = {
  docs: defineCollection({
    loader: glob({ base: `./fixtures/${process.env.DOCS_FIXTURE ?? "valid"}`, pattern: "*.md" }),
    schema: docsSchema(),
  }),
};
