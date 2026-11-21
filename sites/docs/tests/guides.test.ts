// Harmos — SPDX-License-Identifier: Apache-2.0
// Prove Native Loader Watch Configuration Without Browser Timing.

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseFrontmatter } from "@astrojs/markdown-remark";
import type { LoaderContext } from "astro/loaders";
import type { Root } from "mdast";
import { guides, remarkGuides } from "../src/guides.ts";

test("native loader watches the guides root and handles edits, additions and D2 changes", async () => {
  const scratch = fileURLToPath(new URL("../../../target/docs/loader-tests/", import.meta.url));
  mkdirSync(scratch, { recursive: true });
  const root = mkdtempSync(join(scratch, "docs-loader-"));
  const docs = join(root, "docs");
  const project = join(root, "sites", "docs");
  const generated = join(root, "target", "docs", "generated");
  const watched: string[] = [];
  const handlers = new Map<string, Array<(path: string) => Promise<void>>>();
  const entries = new Map();
  const errors: unknown[] = [];
  try {
    mkdirSync(docs, { recursive: true });
    mkdirSync(project, { recursive: true });
    mkdirSync(generated, { recursive: true });
    const guide = join(docs, "index.md");
    writeFileSync(guide, "---\nsidebar: { order: 2 }\n---\n# Start\n\n```d2\nsource -> first\n```\n");
    await guides().load({
      config: { root: pathToFileURL(`${project}/`), srcDir: pathToFileURL(`${project}/src/`) },
      collection: "docs",
      logger: {
        warn() {},
        info() {},
        error(error: unknown) {
          errors.push(error);
        },
      },
      watcher: {
        add(path: string) {
          watched.push(resolve(path));
        },
        on(event: string, handler: (path: string) => Promise<void>) {
          handlers.set(event, [...(handlers.get(event) ?? []), handler]);
        },
      },
      store: {
        keys: () => [...entries.keys()],
        get: (id: string) => entries.get(id),
        set: (entry: { id: string }) => entries.set(entry.id, entry),
        delete: (id: string) => entries.delete(id),
      },
      parseData: async ({ data }: { data: object }) => data,
      generateDigest: (body: string) => body,
      entryTypes: new Map([
        [
          ".md",
          {
            getEntryInfo: ({ contents }: { contents: string }) => {
              const { content, frontmatter } = parseFrontmatter(contents);
              return { body: content, data: frontmatter };
            },
          },
        ],
      ]),
    } as unknown as LoaderContext);
    assert.deepEqual(watched, [docs, generated]);
    assert.deepEqual([...handlers.keys()].sort(), ["add", "change", "unlink"]);
    assert.equal(entries.get("index").data.title, "Start");
    assert.deepEqual(entries.get("index").data.sidebar, { order: 2 });

    writeFileSync(guide, "# Updated\n\n```d2\nsource -> second\n```\n");
    for (const handler of handlers.get("change") ?? []) {
      await handler(guide);
    }
    assert.equal(entries.get("index").data.title, "Updated");
    assert.match(entries.get("index").body, /source -> second/);

    const added = join(docs, "added.md");
    writeFileSync(added, "# Added\n");
    for (const handler of handlers.get("add") ?? []) {
      await handler(added);
    }
    assert.equal(entries.get("added").data.title, "Added");
    assert.equal(entries.size, 2);
    assert.deepEqual(errors, []);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("relative links cross guide folders and retain repository targets", () => {
  const root = resolve("repository");
  const tree: Root = {
    type: "root",
    children: [
      {
        type: "paragraph",
        children: [
          { type: "link", url: "../sdks/rust.md#setup", children: [] },
          { type: "link", url: "../index.md", children: [] },
          { type: "link", url: "../../sdk/rust/README.md", children: [] },
        ],
      },
    ],
  };
  const transform = remarkGuides({ root, base: "/dev/", source: "https://example.test/source/" })();
  transform(tree, { path: join(root, "docs", "guides", "plugins.md") });
  assert.deepEqual(tree.children, [
    {
      type: "paragraph",
      children: [
        { type: "link", url: "/dev/sdks/rust/#setup", children: [] },
        { type: "link", url: "/dev/", children: [] },
        { type: "link", url: "https://example.test/source/sdk/rust/README.md", children: [] },
      ],
    },
  ]);
});
