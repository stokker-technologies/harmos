// Harmos — SPDX-License-Identifier: Apache-2.0
// Read the Maintained Guides and Generated References in Place, Without Copies.

import { readFileSync, statSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { glob, type Loader } from "astro/loaders";
import type { Definition, Link, Root } from "mdast";
import { visit } from "unist-util-visit";

/** Guide Roots Relative to the Site Project; Generated References Live Under target/. */
const SOURCES = ["../../docs", "../../target/docs/generated"];
/** The Product Guide Is the Landing Page of Every Entry. */
const LANDING = "product-guide";

export interface Routes {
  /** The Repository Root Holding docs/ and target/. */
  root: string;
  /** The Entry Base Path, Such as `/dev/` or `/0.1/`. */
  base: string;
  /** The Repository Browser Prefix for Files Outside the Guides, at the Built Revision. */
  source: string;
}

/** The Route Slug of One Guide Path Relative to Its Source Root. */
export function slug(path: string): string {
  const name = path.split(sep).join("/").replace(/\.md$/i, "").toLowerCase();
  return name === LANDING ? "index" : name;
}

/** Load Guides and Generated References, Taking Each Title From Its Frontmatter or First Heading. */
export function guides(): Loader {
  return {
    name: "harmos-guides",
    load: async (context) => {
      for (const base of SOURCES) {
        const source = glob({ base, pattern: "**/*.md", generateId: ({ entry }) => slug(entry) });
        // A Glob Removes Every Stored Entry It Did Not Load, So Each Source Sees Only Its Own Entries.
        const store = new Proxy(context.store, {
          get(target, property) {
            if (property === "keys") {
              return () => target.keys().filter((id) => target.get(id)?.filePath?.startsWith(`${base}/`));
            }
            const value = Reflect.get(target, property, target);
            return typeof value === "function" ? value.bind(target) : value;
          },
        });
        await source.load({
          ...context,
          store,
          parseData: (entry) => {
            const path = entry.filePath ? resolve(fileURLToPath(context.config.root), entry.filePath) : undefined;
            return context.parseData({ ...entry, data: { title: path ? title(path, entry.id) : entry.id, ...entry.data } });
          },
        });
      }
    },
  };
}

/** Drop the Leading Heading That Became the Page Title and Route Relative Links. */
export function remarkGuides(routes: Routes) {
  const docs = resolve(routes.root, "docs");
  return () => (tree: Root, file: { path?: string }) => {
    if (!file.path || !inside(docs, resolve(file.path))) {
      return;
    }
    const heading = tree.children.findIndex((node) => node.type === "heading" && node.depth === 1);
    if (heading !== -1) {
      tree.children.splice(heading, 1);
    }
    const from = dirname(resolve(file.path));
    visit(tree, ["link", "definition"], (node) => {
      const target = node as Link | Definition;
      target.url = route(routes, docs, from, target.url);
    });
  };
}

function route(routes: Routes, docs: string, from: string, url: string): string {
  if (/^[a-z][a-z0-9+.-]*:/i.test(url) || url.startsWith("#") || url.startsWith("/") || url === "") {
    return url;
  }
  const [path, ...fragments] = url.split("#");
  const anchor = fragments.length ? `#${fragments.join("#")}` : "";
  let target = resolve(from, decodeURI(path));
  if (inside(docs, target) && directory(target)) {
    target = resolve(target, "README.md");
  }
  if (inside(docs, target) && target.toLowerCase().endsWith(".md")) {
    const name = slug(relative(docs, target));
    return `${routes.base}${name === "index" ? "" : `${name}/`}${anchor}`;
  }
  const repository = relative(routes.root, target);
  if (repository.startsWith("..") || isAbsolute(repository)) {
    return url;
  }
  return `${routes.source}${repository.split(sep).join("/")}${anchor}`;
}

function title(path: string, fallback: string): string {
  const heading = readFileSync(path, "utf-8").match(/^#\s+(.+?)\s*#*\s*$/m);
  return heading ? heading[1] : fallback;
}

function inside(parent: string, path: string): boolean {
  const offset = relative(parent, path);
  return offset !== "" && !offset.startsWith("..") && !isAbsolute(offset);
}

function directory(path: string): boolean {
  try {
    return statSync(path).isDirectory();
  } catch {
    return false;
  }
}
