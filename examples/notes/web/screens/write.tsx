import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { assert, postNotes, useScreen } from "@/lib/lengua";

interface WriteState {
  readonly title: string;
  readonly text: string;
  readonly error: string;
  readonly saved: boolean;
  readonly message: string;
}

const LONGEST_TITLE = 40;

export function titleError(title: string): string {
  const trimmed = title.trim();
  assert(trimmed.length <= title.length, "trimming makes no text longer");
  const error =
    trimmed === ""
      ? "A note has a title."
      : trimmed.length > LONGEST_TITLE
        ? "A title has 40 characters or fewer."
        : "";
  assert(error === "" || trimmed === "" || trimmed.length > LONGEST_TITLE, "an error has its reason");
  return error;
}

export async function save(state: WriteState, set: (next: WriteState) => void): Promise<void> {
  const error = titleError(state.title);
  assert(error.length < 100, "an error is short");
  if (error !== "") {
    set({ ...state, error });
    return;
  }
  const answer = await postNotes({ title: state.title, text: state.text });
  assert(answer.status >= 100, "the server answered");
  if (answer.ok) {
    set({ ...state, error: "", saved: true, message: `Saved note ${answer.value.id}.` });
  } else {
    set({ ...state, error: answer.error.error });
  }
}

export function WriteScreen() {
  const [state, setState] = useScreen<WriteState>({
    title: "",
    text: "",
    error: "",
    saved: false,
    message: "",
  });
  return (
    <main className="mx-auto max-w-xl p-6">
      <Card>
        <CardHeader>
          <CardTitle>Write a note</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <Label htmlFor="title">Title</Label>
          <Input
            id="title"
            value={state.title}
            onChange={(event) => {
              setState({ ...state, title: event.target.value });
            }}
          />
          <Label htmlFor="text">Text</Label>
          <Textarea
            id="text"
            value={state.text}
            onChange={(event) => {
              setState({ ...state, text: event.target.value });
            }}
          />
          <p role="alert">{state.error}</p>
          <p>{state.message}</p>
          <Button
            onClick={() => {
              save(state, setState).catch(() => {
                setState({ ...state, error: "The note was not saved." });
              });
            }}
          >
            Save
          </Button>
        </CardContent>
      </Card>
    </main>
  );
}
