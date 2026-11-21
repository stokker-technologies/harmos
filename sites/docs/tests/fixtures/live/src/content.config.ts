/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/**
 * Load Live Diagram Fixtures
 */

/* === Imports === */

// Standard Library Imports

// Third Party Imports
import { defineCollection } from "astro:content";
import { docsSchema } from "@astrojs/starlight/schema";
import { glob } from "astro/loaders";

// Local Imports
import { diagramLoader } from "../../../../src/diagrams.ts";

/* === Re-Exports === */

/* === Constants === */

/* === Types === */

/* === Classes === */

/* === Public Functions === */

/* === Private Functions === */

/* === Entry Point === */

export const collections = {
  docs: defineCollection({
    loader: diagramLoader(glob({ base: "../valid", pattern: "*.md" })),
    schema: docsSchema(),
  }),
};
