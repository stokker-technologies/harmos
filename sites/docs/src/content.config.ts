// Harmos — SPDX-License-Identifier: Apache-2.0
// The Guides Collection Reads docs/ and Generated References in Place.

import { docsSchema } from "@astrojs/starlight/schema";
import { defineCollection } from "astro:content";
import { diagramLoader } from "./diagrams";
import { guides } from "./guides";

export const collections = {
  // Guides Re-Render When a D2 File They Reference Changes.
  docs: defineCollection({ loader: diagramLoader(guides()), schema: docsSchema() }),
};
