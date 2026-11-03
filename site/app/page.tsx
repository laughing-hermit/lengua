import Link from "next/link";
import { Promises } from "./promises";

export default function Home() {
  return (
    <>
      <section className="hero">
        <h1>Share what a program does. Every computer builds its own.</h1>
        <p>
          lengua is an AI language, not a programming language. A person describes a program in plain words,
          their AI agent writes it down as a spec and compiles it to machine code for their own computer, and
          lengua checks that code there. So people share specs, never code.
        </p>
        <p className="actions">
          <Link href="/specs" className="button">
            Browse specs
          </Link>
          <Link href="/share" className="button quiet">
            Share a spec
          </Link>
        </p>
      </section>
      <section>
        <h2>How it works</h2>
        <ol className="steps">
          <li>
            <strong>Describe it.</strong> Ask your agent for a program, in your own words. With lengua&apos;s
            plugin it writes a spec.md: what the program does, each behavior shown by an example and tried by
            tests, in plain markdown.
          </li>
          <li>
            <strong>Share the spec.</strong> Only the spec.md. It names no CPU or operating system, so it
            works anywhere lengua does.
          </li>
          <li>
            <strong>Compile it anywhere.</strong> Save a spec in a new folder as spec.md and ask your agent to
            compile it. lengua checks the code it writes on that computer: every test with fresh inputs, its
            rules for safe assembly, and the proofs.
          </li>
        </ol>
      </section>
      <section>
        <h2>What a shared spec promises</h2>
        <Promises />
      </section>
    </>
  );
}
