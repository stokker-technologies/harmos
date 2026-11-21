// Harmos — SPDX-License-Identifier: Apache-2.0
// The Guides Collection Reads docs/ and Generated References in Place.

import { docsSchema } from "@astrojs/starlight/schema";
import { defineCollection } from "astro:content";
import { guides } from "./guides";

export const collections = {
  docs: defineCollection({ loader: guides(), schema: docsSchema() }),
};
