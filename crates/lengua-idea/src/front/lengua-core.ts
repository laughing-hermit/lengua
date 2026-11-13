import { createContext, useContext, useState } from "react";

export function assert(condition: boolean, message: string): asserts condition {
  if (!condition) {
    throw new Error(message);
  }
}

export interface ScreenRecord {
  readonly first: Readonly<Record<string, unknown>>;
  current: object;
}

export const ScreenContext = createContext<ScreenRecord | null>(null);

export function useScreen<S extends object>(first: S): readonly [S, (next: S) => void] {
  const record = useContext(ScreenContext);
  const [state, setState] = useState<S>(() =>
    record === null ? first : Object.assign({}, first, record.first),
  );
  if (record !== null) {
    record.current = state;
  }
  return [state, setState];
}

export function queryParam(name: string): string {
  if (typeof window === "undefined") {
    return "";
  }
  return new URLSearchParams(window.location.search).get(name) ?? "";
}

export type Answer<T, F> =
  | { readonly ok: true; readonly status: number; readonly value: T }
  | { readonly ok: false; readonly status: number; readonly error: F };

async function read(response: Response, json: boolean): Promise<unknown> {
  const text = await response.text();
  if (!json || text === "") {
    return text;
  }
  try {
    const value: unknown = JSON.parse(text);
    return value;
  } catch {
    return text;
  }
}

export async function call(
  method: string,
  path: string,
  body: unknown,
  json: boolean,
): Promise<{ readonly ok: boolean; readonly status: number; readonly value: unknown }> {
  const init: RequestInit =
    body === undefined
      ? { method }
      : { method, headers: { "content-type": "application/json" }, body: JSON.stringify(body) };
  const response = await fetch(path, init);
  const value = await read(response, json);
  return { ok: response.ok, status: response.status, value };
}
