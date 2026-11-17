// Harmos — SPDX-License-Identifier: Apache-2.0
// One Documentation Entry: the Guides and References of One Source, Served Under /<entry>/.

import { fileURLToPath } from "node:url";
import { unified } from "@astrojs/markdown-remark";
import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";
import { remarkGuides } from "./src/guides";

// The Docs Task Selects the Entry and Source Revision; Direct Runs Preview the Development Entry.
const root = fileURLToPath(new URL("..", import.meta.url));
const entry = process.env.DOCS_ENTRY ?? "dev";
const base = `/${entry}/`;
const source = process.env.DOCS_SOURCE ?? "https://github.com/stokker-technologies/harmos/blob/dev/";

export default defineConfig({
  base,
  outDir: `../target/docs/${entry}`,
  markdown: {
    processor: unified({ remarkPlugins: [remarkGuides({ root, base, source })] }),
  },
  integrations: [
    starlight({
      title: "Harmos",
      logo: {
        src: "./assets/logos/mark.svg",
        replacesTitle: false,
      },
      customCss: ["./src/theme.css"],
      components: { ThemeSelect: "./src/components/ReleaseSelect.astro" },
      social: [{ icon: "github", label: "Source", href: "https://github.com/stokker-technologies/harmos" }],
      sidebar: [
        { label: "Start", items: ["index", "schematic", "examples"] },
        {
          label: "Guides",
          items: ["storage", "plugin-manager", "access-status"],
        },
        { label: "SDKs", items: ["rust-sdk", "python-sdk", "csharp-sdk", "cpp-sdk"] },
        { label: "Operations Reference", items: [{ autogenerate: { directory: "reference/operations" } }] },
        { label: "Project", items: ["architecture", "tooling", "contributing"] },
      ],
    }),
  ],
});
