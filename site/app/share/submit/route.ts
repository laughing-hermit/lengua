import { MAX_SPEC_BYTES, problem, save, type Problem } from "@/lib/specs";

/** Back to the share form, saying what was wrong. */
function refused(request: Request, why: Problem): Response {
  return Response.redirect(new URL(`/share?problem=${why}`, request.url), 303);
}

/** Takes a spec from the share form, checks it, keeps it, and shows it. */
export async function POST(request: Request): Promise<Response> {
  // It writes to this computer's disk, so only the site's own form may post here, never another
  // page the browser has open.
  const origin = request.headers.get("origin");
  if (origin !== null && origin !== new URL(request.url).origin) {
    return new Response("Only this site's own form can share a spec.\n", { status: 403 });
  }
  const form = await request.formData();
  const file = form.get("file");
  let text = String(form.get("text") ?? "");
  if (file instanceof File && file.size > 0) {
    if (!file.name.toLowerCase().endsWith(".md")) {
      return refused(request, "not-markdown");
    }
    if (file.size > MAX_SPEC_BYTES) {
      return refused(request, "long");
    }
    text = await file.text();
  }
  text = text.replace(/\r\n/g, "\n");
  const why = problem(text);
  if (why) {
    return refused(request, why);
  }
  const name = await save(text);
  return Response.redirect(new URL(`/specs/${name}`, request.url), 303);
}
