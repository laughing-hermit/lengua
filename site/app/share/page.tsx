import { PROBLEMS, MAX_SPEC_BYTES, type Problem } from "@/lib/specs";

export default async function Share({ searchParams }: { searchParams: Promise<{ problem?: string }> }) {
  const { problem } = await searchParams;
  const message = problem && problem in PROBLEMS ? PROBLEMS[problem as Problem] : null;
  return (
    <>
      <h1>Share a spec</h1>
      <p>
        Share the spec.md, never main.s or any other code: whoever compiles it gets their own program for
        their own computer, checked there. The site takes a lengua spec of up to {MAX_SPEC_BYTES / 1024} KiB
        with its purpose and behaviors, and refuses anything holding code.
      </p>
      <form method="post" action="/share/submit" encType="multipart/form-data" className="share">
        <label>
          spec.md <input type="file" name="file" accept=".md,text/markdown" />
        </label>
        <p className="or">or paste it</p>
        <textarea name="text" rows={16} spellCheck={false} placeholder={"---\ntype: lengua/program-spec\ntitle: …"} />
        {message && (
          <p className="error" role="alert">
            {message}
          </p>
        )}
        <button type="submit">Share the spec</button>
      </form>
    </>
  );
}
