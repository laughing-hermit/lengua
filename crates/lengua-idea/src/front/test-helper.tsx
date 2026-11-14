import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReactElement } from "react";
import { afterEach, expect, vi } from "vitest";
import { ScreenContext, type ScreenRecord } from "@/lib/lengua";
import { examples } from "./server";

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function canonical(text: string | null): string {
  if (text === null || text === "") {
    return "";
  }
  try {
    return JSON.stringify(JSON.parse(text));
  } catch {
    return text;
  }
}

function answer(input: RequestInfo | URL, init?: RequestInit): Promise<Response> {
  const path = typeof input === "string" ? input : input.toString();
  const method = init?.method ?? "GET";
  const body = typeof init?.body === "string" ? init.body : null;
  const authorization = new Headers(init?.headers).get("authorization") ?? "";
  const found = examples.find(
    (e) =>
      e.method === method &&
      e.path === path &&
      canonical(e.body) === canonical(body) &&
      e.authorization === authorization,
  );
  if (found === undefined) {
    return Promise.reject(new Error(`no example of the server answers \`${method} ${path}\``));
  }
  const empty = found.answer === "" || found.status === 204 || found.status === 304;
  return Promise.resolve(new Response(empty ? null : found.answer, { status: found.status }));
}

const errors: string[] = [];

export function show(element: ReactElement, first: Readonly<Record<string, unknown>>): ScreenRecord {
  errors.length = 0;
  vi.spyOn(console, "error").mockImplementation((...parts: unknown[]) => {
    errors.push(parts.map(String).join(" "));
  });
  const record: ScreenRecord = { first, current: {} };
  vi.stubGlobal("fetch", answer);
  render(<ScreenContext.Provider value={record}>{element}</ScreenContext.Provider>);
  return record;
}

export function typeInto(label: string, text: string): void {
  fireEvent.change(screen.getByLabelText(label), { target: { value: text } });
}

export function click(name: string): void {
  const found = [
    ...screen.queryAllByRole("button", { name }),
    ...screen.queryAllByRole("link", { name }),
  ];
  const first = found[0] ?? screen.getByText(name);
  fireEvent.click(first);
}

export async function expectText(text: string): Promise<void> {
  await waitFor(
    () => {
      expect(document.body.textContent ?? "", errors.join(" | ").slice(0, 600)).toContain(text);
    },
    { timeout: 5000 },
  );
}

export async function expectState(record: ScreenRecord, field: string, value: unknown): Promise<void> {
  await waitFor(
    () => {
      expect(Reflect.get(record.current, field)).toEqual(value);
    },
    { timeout: 5000 },
  );
}

const NAMED = ["button", "link", "heading", "textbox", "region", "list", "checkbox", "img", "form"] as const;

export async function expectLabel(label: string): Promise<void> {
  await waitFor(
    () => {
      const count =
        screen.queryAllByLabelText(label).length +
        NAMED.reduce((sum, role) => sum + screen.queryAllByRole(role, { name: label }).length, 0);
      expect(count).toBeGreaterThan(0);
    },
    { timeout: 5000 },
  );
}
