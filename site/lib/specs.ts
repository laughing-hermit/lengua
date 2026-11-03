import { promises as fs } from "node:fs";
import path from "node:path";

/** Where the specs people share are kept, on this computer's copy of the site. */
const SHARED = path.join(process.cwd(), "shared");

/** lengua's own example programs, whose specs are shared from the start. */
const EXAMPLES = path.join(process.cwd(), "..", "examples");

/** The longest spec the site takes. */
export const MAX_SPEC_BYTES = 256 * 1024;

/** A spec's name in a link: its title, lowercase, in letters, digits and dashes. */
const NAME = /^[a-z0-9-]{1,64}$/;

/** A fenced code block, or a line of assembly: code, which a spec never holds. */
const CODE = /^\s*(```|~~~)|^\s*\.(intel_syntax|arch|globl|text|section|equ|type|size)\b/m;

export type Spec = {
  name: string;
  title: string;
  description: string;
  /** The whole file, front matter included, as it's downloaded. */
  text: string;
  /** What follows the front matter: the purpose, behaviors and constraints. */
  body: string;
  source: "example" | "shared";
};

/** A spec's front matter, its `key: value` lines, and the text after it. */
function frontMatter(text: string): { fields: Record<string, string>; body: string } | null {
  const match = /^---\n([\s\S]*?)\n---\n?/.exec(text);
  if (!match) {
    return null;
  }
  const fields: Record<string, string> = {};
  for (const line of match[1].split("\n")) {
    const at = line.indexOf(":");
    if (at > 0) {
      fields[line.slice(0, at).trim()] = line.slice(at + 1).trim();
    }
  }
  return { fields, body: text.slice(match[0].length) };
}

/** Why a spec can't be shared here, by the code the share form's page shows it by. */
export const PROBLEMS = {
  long: `A spec here is at most ${MAX_SPEC_BYTES / 1024} KiB.`,
  "not-a-spec":
    "That isn't a lengua spec: a spec.md starts with front matter whose type is lengua/program-spec.",
  "no-title": "The spec's front matter needs a title.",
  "no-behaviors": "A spec has a Purpose and its Behaviors, each under its own heading.",
  code: "Only specs are shared here, never code, and this one holds a code block or assembly.",
  "not-markdown": "Share the spec.md itself, a .md file.",
  empty: "Choose a spec.md, or paste one.",
} as const;

export type Problem = keyof typeof PROBLEMS;

/** Why `text` can't be shared here, or null when it's a spec the site takes. */
export function problem(text: string): Problem | null {
  if (Buffer.byteLength(text, "utf8") > MAX_SPEC_BYTES) {
    return "long";
  }
  if (text.trim() === "") {
    return "empty";
  }
  const parsed = frontMatter(text);
  if (!parsed || parsed.fields.type !== "lengua/program-spec") {
    return "not-a-spec";
  }
  if (!parsed.fields.title) {
    return "no-title";
  }
  if (!/^# Purpose$/m.test(parsed.body) || !/^# Behaviors$/m.test(parsed.body)) {
    return "no-behaviors";
  }
  if (CODE.test(parsed.body)) {
    return "code";
  }
  return null;
}

/** The specs in each folder of `root` that holds a spec.md. */
async function folder(root: string, source: Spec["source"]): Promise<Spec[]> {
  let names: string[];
  try {
    names = await fs.readdir(root);
  } catch {
    return [];
  }
  const specs: Spec[] = [];
  for (const name of names.sort()) {
    if (!NAME.test(name)) {
      continue;
    }
    let text: string;
    try {
      text = await fs.readFile(path.join(root, name, "spec.md"), "utf8");
    } catch {
      continue;
    }
    const parsed = frontMatter(text);
    if (parsed) {
      specs.push({
        name,
        title: parsed.fields.title ?? name,
        description: parsed.fields.description ?? "",
        text,
        body: parsed.body,
        source,
      });
    }
  }
  return specs;
}

/** Every spec on the site: the shared ones first, then lengua's examples. */
export async function allSpecs(): Promise<Spec[]> {
  const [shared, examples] = await Promise.all([
    folder(SHARED, "shared"),
    folder(EXAMPLES, "example"),
  ]);
  return [...shared, ...examples.filter((example) => !shared.some((s) => s.name === example.name))];
}

/** The spec named `name`, or null. */
export async function oneSpec(name: string): Promise<Spec | null> {
  if (!NAME.test(name)) {
    return null;
  }
  return (await allSpecs()).find((spec) => spec.name === name) ?? null;
}

/** Keeps a spec `problem` passed, under a name made from its title; returns the name. */
export async function save(text: string): Promise<string> {
  const title = frontMatter(text)?.fields.title ?? "spec";
  const base =
    title
      .toLowerCase()
      .normalize("NFKD")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 60) || "spec";
  const taken = new Set((await allSpecs()).map((spec) => spec.name));
  let name = base;
  for (let next = 2; taken.has(name); next++) {
    name = `${base}-${next}`;
  }
  await fs.mkdir(path.join(SHARED, name), { recursive: true });
  await fs.writeFile(path.join(SHARED, name, "spec.md"), text, { flag: "wx" });
  return name;
}
