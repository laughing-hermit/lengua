import { notFound } from "next/navigation";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { oneSpec } from "@/lib/specs";
import { Promises } from "../../promises";

export const dynamic = "force-dynamic";

export default async function SpecPage({ params }: { params: Promise<{ name: string }> }) {
  const { name } = await params;
  const spec = await oneSpec(name);
  if (!spec) {
    notFound();
  }
  return (
    <>
      <h1>{spec.title}</h1>
      <p className="lead">{spec.description}</p>
      <div className="compile">
        <p>
          <a href={`/specs/${spec.name}/raw`} className="button" download="spec.md">
            Download spec.md
          </a>
        </p>
        <p>
          To compile it for your computer, save it as spec.md in a new folder, open your agent there with
          lengua&apos;s plugin, and ask it to compile the spec.
        </p>
      </div>
      <article className="spec">
        {/* Raw HTML in a spec is shown as text, never run: react-markdown leaves it out. */}
        {/* Its headings one level down, under the page's own title. */}
        <ReactMarkdown remarkPlugins={[remarkGfm]} components={{ h1: "h2", h2: "h3", h3: "h4", h4: "h5" }}>
          {spec.body}
        </ReactMarkdown>
      </article>
      <section>
        <h2>What this spec promises</h2>
        <Promises />
      </section>
    </>
  );
}
