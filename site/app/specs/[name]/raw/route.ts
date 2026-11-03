import { oneSpec } from "@/lib/specs";

export const dynamic = "force-dynamic";

/** The spec.md itself, as it was shared. */
export async function GET(_request: Request, { params }: { params: Promise<{ name: string }> }) {
  const { name } = await params;
  const spec = await oneSpec(name);
  if (!spec) {
    return new Response("No such spec.\n", { status: 404 });
  }
  return new Response(spec.text, {
    headers: {
      "content-type": "text/markdown; charset=utf-8",
      "content-disposition": 'attachment; filename="spec.md"',
      "x-content-type-options": "nosniff",
    },
  });
}
