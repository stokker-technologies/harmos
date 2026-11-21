// Harmos — SPDX-License-Identifier: Apache-2.0
// One Documentation Entry: the Guides and References of One Source, Served Under /<entry>/.

import { fileURLToPath } from "node:url";
import { unified } from "@astrojs/markdown-remark";
import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";
import { remarkDiagrams } from "./src/diagrams";
import { remarkGuides } from "./src/guides";

// The Docs Task Selects the Entry and Source Revision; Direct Runs Preview the Development Entry.
const root = fileURLToPath(new URL("../..", import.meta.url));
const entry = process.env.DOCS_ENTRY ?? "dev";
const base = `/${entry}/`;
const source = process.env.DOCS_SOURCE ?? "https://github.com/stokker-technologies/harmos/blob/dev/";

export default defineConfig({
  base,
  outDir: `../../target/docs/${entry}`,
  markdown: {
    processor: unified({ remarkPlugins: [remarkGuides({ root, base, source }), remarkDiagrams()] }),
  },
  integrations: [
    starlight({
      title: "Harmos",
      logo: {
        light: "./assets/logos/transparent-on-light.svg",
        dark: "./assets/logos/transparent-on-dark.svg",
        replacesTitle: false,
      },
      customCss: ["./src/theme.css"],
      components: {
        ThemeProvider: "./src/components/ThemeProvider.astro",
        ThemeSelect: "./src/components/ReleaseSelect.astro",
      },
      expressiveCode: {
        customizeTheme(theme) {
          theme.fg = "var(--foreground)";
          theme.colors["editor.foreground"] = "var(--foreground)";
          theme.settings = [
            { scope: ["comment", "punctuation.definition.comment"], settings: { foreground: "var(--muted-foreground)" } },
            { scope: ["keyword", "storage", "constant.language"], settings: { foreground: "var(--link)" } },
            { scope: ["string", "entity.name", "support", "constant.numeric"], settings: { foreground: "var(--foreground)" } },
          ];
          return theme;
        },
        styleOverrides: {
          borderRadius: "var(--brand-radius)",
          borderColor: "var(--border)",
          codeBackground: "var(--muted)",
          codeForeground: "var(--foreground)",
          codeSelectionBackground: "var(--input)",
          frames: {
            editorBackground: "var(--muted)",
            terminalBackground: "var(--muted)",
            editorActiveTabBackground: "var(--muted)",
            editorActiveTabForeground: "var(--foreground)",
            editorTabBarBackground: "var(--background)",
            terminalTitlebarBackground: "var(--background)",
            terminalTitlebarForeground: "var(--foreground)",
            inlineButtonForeground: "var(--foreground)",
            inlineButtonBorder: "var(--input)",
            frameBoxShadowCssValue: "none",
          },
        },
      },
      social: [{ icon: "github", label: "Source", href: "https://github.com/stokker-technologies/harmos" }],
      sidebar: [
        { label: "Start", items: ["index", "schematic", "examples"] },
        {
          label: "Guides",
          items: [{ autogenerate: { directory: "../../docs/guides" } }],
        },
        { label: "SDKs", items: [{ autogenerate: { directory: "../../docs/sdks" } }] },
        { label: "Operations Reference", items: [{ autogenerate: { directory: "../../target/docs/generated/reference/operations" } }] },
        { label: "Project", items: [{ autogenerate: { directory: "../../docs/project" } }, "contributing"] },
      ],
    }),
  ],
});
