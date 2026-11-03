import Link from "next/link";
import { allSpecs } from "@/lib/specs";

export const dynamic = "force-dynamic";

export default async function Specs() {
  const specs = await allSpecs();
  return (
    <>
      <h1>Specs</h1>
      <p>
        Each is a spec.md: what a program does, with its tests. Download one and ask your agent to compile it
        for your computer.
      </p>
      <ul className="cards">
        {specs.map((spec) => (
          <li key={spec.name}>
            <Link href={`/specs/${spec.name}`}>
              <span className="title">{spec.title}</span>
              <span className="description">{spec.description}</span>
              <span className="source">{spec.source === "example" ? "lengua example" : "shared"}</span>
            </Link>
          </li>
        ))}
      </ul>
    </>
  );
}
