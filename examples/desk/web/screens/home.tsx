import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { assert, postWrite, useScreen } from "@/lib/lengua";

interface HomeState {
  readonly prompt: string;
  readonly text: string;
  readonly error: string;
  readonly written: boolean;
}

export async function write(state: HomeState, set: (next: HomeState) => void): Promise<void> {
  assert(state.prompt.length < 1000, "a prompt is short");
  const answer = await postWrite({ prompt: state.prompt });
  assert(answer.status >= 100, "the server answered");
  if (answer.ok) {
    set({ ...state, text: answer.value.text, error: "", written: true });
  } else {
    set({ ...state, error: answer.error.error, written: false });
  }
}

export function HomeScreen() {
  const [state, setState] = useScreen<HomeState>({
    prompt: "",
    text: "",
    error: "",
    written: false,
  });
  return (
    <main className="mx-auto max-w-xl p-6">
      <Card>
        <CardHeader>
          <CardTitle>Write with the local model</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <Label htmlFor="prompt">Prompt</Label>
          <Input
            id="prompt"
            value={state.prompt}
            onChange={(event) => {
              setState({ ...state, prompt: event.target.value });
            }}
          />
          <p role="alert">{state.error}</p>
          <p>{state.text}</p>
          <Button
            onClick={() => {
              write(state, setState).catch(() => {
                setState({ ...state, error: "The server did not answer." });
              });
            }}
          >
            Write
          </Button>
        </CardContent>
      </Card>
    </main>
  );
}
