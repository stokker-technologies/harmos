/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/**
 * Prove Diagram Rendering
 */

/* === Imports === */

// Standard Library Imports
import assert from "node:assert/strict";
import child_process from "node:child_process";
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

// Third Party Imports
import type { LoaderContext } from "astro/loaders";
import type { Root } from "mdast";

// Local Imports
import { diagramLoader, remarkDiagrams } from "../src/diagrams.ts";

/* === Re-Exports === */

/* === Constants === */

const FIXTURE = fileURLToPath(new URL("fixtures/valid/index.md", import.meta.url));

/* === Types === */

/* === Classes === */

/* === Public Functions === */

/* === Private Functions === */

function build(fixture: string) {
  return child_process.spawnSync(process.execPath, ["node_modules/astro/bin/astro.mjs", "build", "--root", "tests"], {
    cwd: fileURLToPath(new URL("..", import.meta.url)),
    env: { ...process.env, ASTRO_TELEMETRY_DISABLED: "1", DOCS_FIXTURE: fixture },
    encoding: "utf8",
    windowsHide: true,
  });
}

function directive(attributes: Record<string, string | undefined>): Root {
  return {
    type: "root",
    children: [
      {
        type: "leafDirective",
        name: "d2",
        attributes,
        children: [],
        position: { start: { line: 5, column: 1 }, end: { line: 5, column: 20 } },
      },
    ],
  } as unknown as Root;
}

/* === Entry Point === */

test("identical sources share rendering across titles and files", (context) => {
  const runner = context.mock.method(child_process, "spawnSync", () => ({
    status: 0,
    stdout: '<svg><defs><marker id="arrow"/></defs><path marker-end="url(#arrow)"/><a href="#guide"><text>host</text></a></svg>',
  }));
  const transform = remarkDiagrams()();
  const first: Root = {
    type: "root",
    children: [{ type: "code", lang: "d2", meta: 'title="A & B"', value: "host -> plugin" }],
  };
  const second: Root = {
    type: "root",
    children: [{ type: "code", lang: "d2", meta: 'title="Another Launch"', value: "host -> plugin" }],
  };
  transform(first, { path: "first.md" });
  transform(second, { path: "second.md" });
  assert.equal(runner.mock.callCount(), 2);
  assert.match(JSON.stringify(first), /A &amp; B/);
  assert.match(JSON.stringify(first), /href=\\"#guide\\"/);
  const identifiers = [...JSON.stringify([first, second]).matchAll(/id=\\"([^\\]+)\\"/g)].map((match) => match[1]);
  assert.equal(identifiers.length, 4);
  assert.equal(new Set(identifiers).size, identifiers.length);
  const edited: Root = {
    type: "root",
    children: [{ type: "code", lang: "d2", value: "source -> updated" }],
  };
  transform(edited, { path: "first.md" });
  assert.equal(runner.mock.callCount(), 4);
  const options = runner.mock.calls[2].arguments[2];
  assert.ok(options);
  assert.equal(options.input, "source -> updated");
});

test("file directives render both named themes and pass validated CLI props", (context) => {
  const runner = context.mock.method(child_process, "spawnSync", () => ({ status: 0, stdout: '<svg id="root"/>' }));
  const tree = directive({
    src: "./launch.d2",
    title: "File & Launch",
    layout: "elk",
    theme: "1",
    "dark-theme": "200",
    sketch: "",
    pad: "40",
  });
  remarkDiagrams()()(tree, { path: FIXTURE });
  assert.match(JSON.stringify(tree), /aria-label=\\"File &amp; Launch\\"/);
  assert.match(JSON.stringify(tree), /data-d2-theme=\\"light\\"/);
  assert.match(JSON.stringify(tree), /data-d2-theme=\\"dark\\"/);
  assert.equal(runner.mock.callCount(), 2);
  for (const [index, call] of runner.mock.calls.entries()) {
    const args = call.arguments[1] as string[];
    for (const flag of ["--layout=elk", `--theme=${index === 0 ? 1 : 200}`, "--dark-theme=-1", "--sketch", "--pad=40"]) {
      assert.ok(args.includes(flag), flag);
    }
    assert.equal(args.at(-2), resolve(FIXTURE, "../launch.d2"));
    assert.equal(call.arguments[2]?.cwd, resolve(FIXTURE, ".."));
    assert.equal(call.arguments[2]?.input, undefined);
  }
  const fence: Root = {
    type: "root",
    children: [
      { type: "code", lang: "d2", meta: 'title="Fence" layout="elk" theme="1" dark-theme="200" sketch pad="40"', value: "a -> b" },
    ],
  };
  remarkDiagrams()()(fence, { path: FIXTURE });
  assert.deepEqual(
    (runner.mock.calls[2].arguments[1] as string[]).slice(0, -2),
    (runner.mock.calls[0].arguments[1] as string[]).slice(0, -2),
  );
  assert.equal(runner.mock.calls[2].arguments[2]?.input, "a -> b");
});

test("default file and fence renderings have identical SVG and default CLI flags", (context) => {
  const runner = context.mock.method(child_process, "spawnSync");
  const source = readFileSync(new URL("fixtures/valid/launch.d2", import.meta.url), "utf8");
  const file = directive({ src: "./launch.d2" });
  const fence: Root = {
    type: "root",
    children: [{ type: "code", lang: "d2", value: source, position: { start: { line: 5, column: 1 }, end: { line: 5, column: 20 } } }],
  };
  const transform = remarkDiagrams()();
  transform(file, { path: FIXTURE });
  transform(fence, { path: FIXTURE });
  assert.equal(file.children[0].type, "html");
  assert.equal(fence.children[0].type, "html");
  assert.deepEqual(file, fence);
  for (const [index, call] of runner.mock.calls.entries()) {
    const args = call.arguments[1] as string[];
    assert.ok(args.includes("--layout=dagre"));
    assert.ok(args.includes(`--theme=${index % 2 === 0 ? 0 : 200}`));
    assert.ok(!args.some((arg) => arg.startsWith("--pad") || arg.startsWith("--sketch")));
  }
});

test("missing files and invalid props name the Markdown location and prop", () => {
  const transform = remarkDiagrams()();
  for (const attributes of [
    { src: "./absent.d2" },
    { src: "../../../../../../outside.d2" },
    {},
    { src: "./launch.d2", layout: "invalid" },
    { src: "./launch.d2", theme: "1.5" },
    { src: "./launch.d2", "dark-theme": "blue" },
    { src: "./launch.d2", pad: "-1" },
    { src: "./launch.d2", sketch: "maybe" },
    { src: "./launch.d2", unknown: "value" },
  ]) {
    assert.throws(
      () => transform(directive(attributes), { path: FIXTURE }),
      (error: Error) => {
        assert.ok(error.message.includes(`${FIXTURE}:5:1`));
        assert.match(error.message, /prop/);
        return true;
      },
    );
  }
  for (const meta of [
    'layout="invalid"',
    'theme="x"',
    'pad="1.5"',
    'sketch="maybe"',
    'src="./launch.d2"',
    'title="unterminated',
    "theme=1 theme=2",
  ]) {
    const tree: Root = { type: "root", children: [{ type: "code", lang: "d2", meta, value: "a -> b" }] };
    assert.throws(() => transform(tree, { path: FIXTURE }), /index\.md:1:1.*\n.*prop/);
  }
});

test("fixture file imports resolve relative to their file", () => {
  const file = directive({ src: "./import.d2", title: "Imported Diagram" });
  remarkDiagrams()()(file, { path: FIXTURE });
  assert.match(JSON.stringify(file), /aria-label=\\"Imported Diagram\\"/);
  assert.match(JSON.stringify(file), /<text/);
});

test("loader invalidates file diagrams without changing Markdown and preserves edits across syncs", async (context) => {
  context.mock.method(child_process, "spawnSync", () => ({ status: 0, stdout: "<svg/>" }));
  const files = [resolve(FIXTURE, "../launch.d2")];
  const entries = new Map();
  const watched: string[] = [];
  const handlers = new Map<string, (path: string) => Promise<void>>();
  const errors: string[] = [];
  const data = { title: "Fixture" };
  const body = readFileSync(FIXTURE, "utf8");
  const renderMarkdown = context.mock.fn(async (content: string, options: { fileURL: URL }) => {
    assert.equal(content, body);
    assert.equal(fileURLToPath(options.fileURL), FIXTURE);
    return { html: "<svg/>", metadata: { frontmatter: { d2Files: files } } };
  });
  const loader = diagramLoader({
    name: "fixture",
    load: async ({ store }) => {
      assert.equal(store.get("fixture")?.digest, undefined);
      store.set({
        id: "fixture",
        data,
        body,
        filePath: "index.md",
        digest: "unchanged-markdown",
        rendered: { html: "<svg/>", metadata: { frontmatter: { d2Files: files } } },
      });
    },
  });
  const loaderContext = {
    config: { root: pathToFileURL(`${resolve(FIXTURE, "..")}/`) },
    store: {
      get: (id: string) => entries.get(id),
      set: (entry: { id: string }) => entries.set(entry.id, entry),
      values: () => [...entries.values()],
    },
    watcher: {
      add: (paths: string[]) => watched.push(...paths),
      on: (event: string, handler: (path: string) => Promise<void>) => handlers.set(event, handler),
    },
    logger: { error: (error: string) => errors.push(error), info() {} },
    renderMarkdown,
  } as unknown as LoaderContext;
  await loader.load(loaderContext);
  assert.deepEqual(watched, files);
  assert.equal(entries.get("fixture").digest, undefined);
  await handlers.get("change")?.(files[0]);
  assert.equal(renderMarkdown.mock.callCount(), 1);
  assert.equal(entries.get("fixture").data, data);
  await handlers.get("change")?.(resolve(FIXTURE, "../unreferenced.d2"));
  assert.equal(renderMarkdown.mock.callCount(), 1);
  await loader.load(loaderContext);
  assert.deepEqual(errors, []);
});

test("dev server reloads a referenced D2 edit without changing Markdown", async () => {
  const { dev } = await import("astro");
  const source = new URL("fixtures/valid/launch.d2", import.meta.url);
  const original = readFileSync(source, "utf8");
  const markdown = readFileSync(FIXTURE, "utf8");
  const server = await dev({
    root: fileURLToPath(new URL("fixtures/live/", import.meta.url)),
    server: { port: 0, host: "127.0.0.1" },
    logLevel: "error",
  });
  let socket: WebSocket | undefined;
  try {
    const address = `http://127.0.0.1:${server.address.port}/dev/`;
    const first = await fetch(address);
    assert.equal(first.status, 200);
    const before = await first.text();
    const client = await fetch(new URL("@vite/client", address));
    const token = (await client.text()).match(/const wsToken = "([^"]+)"/);
    assert.ok(token, "Vite HMR Token");
    socket = new WebSocket(`ws://127.0.0.1:${server.address.port}/?token=${token[1]}`, "vite-hmr");
    const connected = new Promise<void>((resolve, reject) => {
      socket?.addEventListener("error", reject, { once: true });
      socket?.addEventListener("message", (event) => {
        if (JSON.parse(String(event.data)).type === "connected") {
          resolve();
        }
      });
    });
    await connected;
    const reloaded = new Promise<void>((resolve) => {
      socket?.addEventListener("message", (event) => {
        if (JSON.parse(String(event.data)).type === "full-reload") {
          resolve();
        }
      });
    });
    writeFileSync(source, `${original}\nplugin -> completed\n`);
    await reloaded;
    const response = await fetch(address);
    assert.equal(response.status, 200);
    const after = await response.text();
    const diagrams = /<div class="d2-diagram">[\s\S]*?<\/div>/g;
    assert.notDeepEqual(after.match(diagrams), before.match(diagrams));
    assert.equal(readFileSync(FIXTURE, "utf8"), markdown);
    assert.deepEqual(
      [...after.matchAll(/<svg\b[^>]*data-d2-theme="(light|dark)"/g)].map((match) => match[1]),
      ["light", "dark", "light", "dark", "light", "dark", "light", "dark"],
    );
  } finally {
    socket?.close();
    await server.stop();
    writeFileSync(source, original);
  }
});

test("fixture build inlines both themes with selectable text and unique references", () => {
  // Clear Only the Owned Fixture Cache So This Proof Exercises the Compiler.
  rmSync(new URL("../../../target/docs/fixture-cache", import.meta.url), { recursive: true, force: true });
  const result = build("valid");
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  const output = new URL("../../../target/docs/fixtures/", import.meta.url);
  const html = readFileSync(new URL("index.html", output), "utf8");
  const diagrams = [...html.matchAll(/<svg\b[^>]*data-d2-theme="(light|dark)"[^>]*>/g)];
  assert.deepEqual(
    diagrams.map((match) => match[1]),
    ["light", "dark", "light", "dark", "light", "dark", "light", "dark"],
  );
  assert.ok(diagrams.every((match) => /role="img"/.test(match[0]) && /aria-label="[^"]+"/.test(match[0])));
  assert.match(html, /<text\b/);
  assert.match(html, /data:application\/font/);
  const identifiers = [...html.matchAll(/\bid="([^"]+)"/g)].map((match) => match[1]);
  assert.equal(new Set(identifiers).size, identifiers.length);
  for (const reference of html.matchAll(/url\(#([^)]+)\)/g)) {
    assert.ok(identifiers.includes(reference[1]), `Missing SVG Reference: ${reference[1]}`);
  }
});

test("invalid fixture fails the build at the fence's file and line", () => {
  const result = build("invalid");
  assert.notEqual(result.status, 0);
  assert.match(`${result.stdout}\n${result.stderr}`, /fixtures[/\\]invalid[/\\]index\.md:5:1/);
});
