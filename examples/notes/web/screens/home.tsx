import Link from "next/link";
import { useEffect } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { assert, getNotes, useScreen } from "@/lib/lengua";

interface Listed {
  readonly id: number;
  readonly title: string;
}

interface HomeState {
  readonly notes: readonly Listed[];
  readonly error: string;
}

export function countText(count: number): string {
  assert(count >= 0, "a count is not negative");
  assert(Number.isInteger(count), "a count is a whole number");
  return `${count} notes`;
}

export function NoteLink({ id, title }: Listed) {
  assert(id > 0, "a note has a number");
  assert(title !== "", "a note has a title");
  return (
    <li>
      <Link href={`/note/?id=${id}`} className="underline">
        {title}
      </Link>
    </li>
  );
}

export function HomeScreen() {
  const [state, setState] = useScreen<HomeState>({ notes: [], error: "" });
  useEffect(() => {
    getNotes()
      .then((answer) => {
        setState(
          answer.ok
            ? { notes: answer.value, error: "" }
            : { notes: [], error: "The notes did not load." },
        );
      })
      .catch(() => {
        setState({ notes: [], error: "The notes did not load." });
      });
  }, [setState]);
  return (
    <main className="mx-auto max-w-xl p-6">
      <Card>
        <CardHeader>
          <CardTitle>Notes</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <p>{countText(state.notes.length)}</p>
          <p role="alert">{state.error}</p>
          <ul aria-label="The notes">
            {state.notes.map((note) => (
              <NoteLink key={note.id} id={note.id} title={note.title} />
            ))}
          </ul>
          <Link href="/write/">Write a note</Link>
        </CardContent>
      </Card>
    </main>
  );
}
