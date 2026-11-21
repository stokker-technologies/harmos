/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/**
 * Render D2 Fences and File Directives Offline
 */

/* === Imports === */

// Standard Library Imports
import child_process from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, realpathSync, statSync } from "node:fs";
import { dirname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

// Third Party Imports
import type { Loader } from "astro/loaders";
import type { Code, Root } from "mdast";
import { visit } from "unist-util-visit";

// Local Imports

/* === Re-Exports === */

/* === Constants === */

const ROOT = fileURLToPath(new URL("../../..", import.meta.url));

/* === Types === */

interface Options {
  title: string;
  layout: string;
  theme: number;
  "dark-theme": number;
  sketch: boolean;
  pad?: number;
}

interface Directive {
  type: "leafDirective";
  name: string;
  attributes?: Record<string, string | null | undefined>;
  position?: Code["position"];
}

interface Markdown {
  path?: string;
  value?: unknown;
  data?: { astro?: { frontmatter?: Record<string, unknown> } };
}

/* === Classes === */

/* === Public Functions === */

/** Share Rendering and SVG References Between Fences and File Directives. */
export function remarkDiagrams() {
  const cache = new Map<string, string[]>();
  return () => (tree: Root, file: Markdown) => {
    const files = new Set<string>();
    visit(tree, (node, index, parent) => {
      const diagram = node as Code | Directive;
      const fence = diagram.type === "code" && diagram.lang === "d2";
      const directive = diagram.type === "leafDirective" && diagram.name === "d2";
      if ((!fence && !directive) || !parent || index === undefined) {
        return;
      }
      const location = origin(file, diagram);
      try {
        const properties = diagram.type === "code" ? (diagram.meta ?? "") : (diagram.attributes ?? {});
        const { options, src } = parseOptions(properties, directive);
        const path = src === undefined ? undefined : sourcePath(src, file.path);
        if (path) {
          files.add(path);
        }
        const source = diagram.type === "code" ? diagram.value.replaceAll("\r\n", "\n") : "";
        const { title, ...flags } = options;
        const key = JSON.stringify([source, flags]);
        // Files May Import Other Files; Their Renderings Cannot Be Cached by Source Alone.
        let diagrams = path ? undefined : cache.get(key);
        if (!diagrams) {
          diagrams = [render(source, path, options, "light"), render(source, path, options, "dark")];
          if (!path) {
            cache.set(key, diagrams);
          }
        }
        parent.children[index] = { type: "html", value: themedSvg(diagrams, title, location) };
      } catch (error) {
        throw new Error(`Render D2 at ${location}: Fix the diagram or its props, check src, or run mise install.\n${String(error)}`);
      }
    });
    file.data ??= {};
    file.data.astro ??= {};
    file.data.astro.frontmatter ??= {};
    file.data.astro.frontmatter.d2Files = [...files];
  };
}

/** Refresh File Diagrams Through Astro's Content Store When Their Sources Change. */
export function diagramLoader(loader: Loader): Loader {
  return {
    ...loader,
    load: async (context) => {
      const store = new Proxy(context.store, {
        get(target, property) {
          if (property === "set") {
            return (entry: Parameters<typeof target.set>[0]) => {
              const files: string[] = entry.rendered?.metadata?.frontmatter?.d2Files ?? [];
              context.watcher?.add(files);
              // D2 Owns Import Resolution, So Recompile File Diagrams on Each Content Sync.
              return target.set(files.length ? { ...entry, digest: undefined } : entry);
            };
          }
          const value = Reflect.get(target, property, target);
          return typeof value === "function" ? value.bind(target) : value;
        },
      });
      await loader.load({ ...context, store });
      if (!context.watcher) {
        return;
      }
      const refresh = async (changed: string) => {
        for (const entry of context.store.values()) {
          const files: string[] = entry.rendered?.metadata?.frontmatter?.d2Files ?? [];
          if (!entry.filePath || !files.includes(resolve(changed))) {
            continue;
          }
          const path = pathToFileURL(resolve(fileURLToPath(context.config.root), entry.filePath));
          try {
            const rendered = await context.renderMarkdown(readFileSync(path, "utf8"), { fileURL: path });
            store.set({ ...entry, digest: undefined, rendered, assetImports: rendered.metadata?.imagePaths });
            context.logger.info(`Reloaded D2 in ${entry.filePath}`);
          } catch (error) {
            context.logger.error(String(error));
          }
        }
      };
      context.watcher.on("change", refresh);
      context.watcher.on("add", refresh);
      context.watcher.on("unlink", refresh);
    },
  };
}

/* === Private Functions === */

function parseOptions(properties: string | Record<string, string | null | undefined>, directive: boolean) {
  const values: Record<string, string | null | undefined> = Object.create(null);
  if (typeof properties === "string") {
    let remaining = properties.trim();
    while (remaining) {
      const match = /^([\w-]+)(?:=(?:"([^"]*)"|'([^']*)'|([^\s"']+)))?(?:\s+|$)/.exec(remaining);
      if (!match) {
        throw new Error(`Invalid prop near ${remaining}; use name="value" or sketch.`);
      }
      if (Object.hasOwn(values, match[1])) {
        throw new Error(`Duplicate prop ${match[1]}; specify it once.`);
      }
      values[match[1]] = match[2] ?? match[3] ?? match[4] ?? "";
      remaining = remaining.slice(match[0].length);
    }
  } else {
    Object.assign(values, properties);
  }
  for (const prop of Object.keys(values)) {
    if (!["title", "layout", "theme", "dark-theme", "sketch", "pad", ...(directive ? ["src"] : [])].includes(prop)) {
      throw new Error(`Unknown prop ${prop}; use title, layout, theme, dark-theme, sketch or pad${directive ? ", and src" : ""}.`);
    }
  }
  const options: Options = {
    title: values.title ?? "Diagram",
    layout: values.layout ?? "dagre",
    theme: 0,
    "dark-theme": 200,
    sketch: false,
  };
  if (!["dagre", "elk", "tala"].includes(options.layout)) {
    throw new Error("Invalid prop layout; choose dagre, elk or tala from d2 0.9.0.");
  }
  for (const prop of ["theme", "dark-theme", "pad"] as const) {
    const value = values[prop];
    if (value === undefined) {
      continue;
    }
    if (value === null || !/^-?\d+$/.test(value) || !Number.isSafeInteger(Number(value)) || (prop === "pad" && Number(value) < 0)) {
      throw new Error(`Invalid prop ${prop}; use ${prop === "pad" ? "a nonnegative integer" : "an integer theme ID"}.`);
    }
    options[prop] = Number(value);
  }
  if (Object.hasOwn(values, "sketch")) {
    if (!["", "true", "false"].includes(values.sketch ?? "")) {
      throw new Error("Invalid prop sketch; use sketch, sketch=true or sketch=false.");
    }
    options.sketch = values.sketch !== "false";
  }
  if (directive && !values.src) {
    throw new Error('Missing prop src; set src="./diagrams/name.d2" relative to this Markdown file.');
  }
  return { options, src: directive ? (values.src ?? undefined) : undefined };
}

function sourcePath(src: string, markdown?: string): string {
  if (!markdown || isAbsolute(src)) {
    throw new Error(`Invalid prop src ${src}; use a path relative to a Markdown file inside the repository.`);
  }
  const path = resolve(dirname(markdown), src);
  const offset = relative(ROOT, path);
  if (offset === ".." || offset.startsWith(`..${sep}`) || isAbsolute(offset)) {
    throw new Error(`Invalid prop src ${src}; keep the diagram inside the repository.`);
  }
  let real: string;
  try {
    real = realpathSync(path);
    if (!statSync(real).isFile()) {
      throw new Error("Not a file");
    }
  } catch {
    throw new Error(`Missing file for prop src ${src}; create ${path} or fix src relative to this Markdown file.`);
  }
  const realOffset = relative(realpathSync(ROOT), real);
  if (realOffset === ".." || realOffset.startsWith(`..${sep}`) || isAbsolute(realOffset)) {
    throw new Error(`Invalid prop src ${src}; keep the linked diagram inside the repository.`);
  }
  return path;
}

function origin(file: Markdown, node: Code | Directive): string {
  // Astro Removes Frontmatter and Leading Whitespace Before Parsing the Body.
  let line = node.position?.start.line ?? 1;
  if (file.path && typeof file.value === "string") {
    const original = readFileSync(file.path, "utf8").replaceAll("\r\n", "\n");
    const offset = original.lastIndexOf(file.value.replaceAll("\r\n", "\n"));
    if (offset >= 0) {
      line += original.slice(0, offset).split("\n").length - 1;
    }
  }
  return `${file.path ?? "Markdown"}:${line}:${node.position?.start.column ?? 1}`;
}

function themedSvg(diagrams: string[], title: string, location: string): string {
  const name = escapeHtml(title);
  const prefix = createHash("sha256").update(location).digest("hex").slice(0, 16);
  const themed = diagrams.map((diagram, theme) => {
    const identifier = `diagram-${prefix}-${theme}-`;
    const identifiers = new Set([...diagram.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]));
    const svg = diagram
      .replace(/\bid="([^"]+)"/g, (_, id) => `id="${identifier}${id}"`)
      .replace(/url\(#([^)]+)\)/g, (_, id) => `url(#${identifier}${id})`)
      .replace(/\b(xlink:)?href="#([^"]+)"/g, (value, namespace, id) =>
        identifiers.has(id) ? `${namespace ?? ""}href="#${identifier}${id}"` : value,
      );
    return svg.replace("<svg", `<svg role="img" aria-label="${name}" data-d2-theme="${theme === 0 ? "light" : "dark"}"`);
  });
  return `<div class="d2-diagram">${themed.join("")}</div>`;
}

function render(source: string, path: string | undefined, options: Options, theme: "light" | "dark"): string {
  // Ignore Ambient Layout and Font Overrides; Keep the Operation's Timeout Policy.
  const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith("D2_") || key === "D2_TIMEOUT"));
  const args = [
    `--layout=${options.layout}`,
    `--theme=${theme === "light" ? options.theme : options["dark-theme"]}`,
    "--dark-theme=-1",
    `--salt=${theme}`,
    "--bundle=false",
    "--no-xml-tag",
  ];
  if (options.sketch) {
    args.push("--sketch");
  }
  if (options.pad !== undefined) {
    args.push(`--pad=${options.pad}`);
  }
  args.push(path ?? "-", "-");
  const result = child_process.spawnSync("d2", args, {
    input: path ? undefined : source,
    cwd: path ? dirname(path) : undefined,
    encoding: "utf8",
    env,
    maxBuffer: 16 * 1024 * 1024,
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    throw result.error ?? new Error(result.stderr);
  }
  return result.stdout;
}

function escapeHtml(value: string): string {
  return value.replaceAll("&", "&amp;").replaceAll('"', "&quot;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
}

/* === Entry Point === */
