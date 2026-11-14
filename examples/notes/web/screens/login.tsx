import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { assert, postLogin, useScreen } from "@/lib/lengua";

interface LoginState {
  readonly name: string;
  readonly password: string;
  readonly error: string;
  readonly signed: boolean;
  readonly token: string;
}

export async function signIn(state: LoginState, set: (next: LoginState) => void): Promise<void> {
  assert(state.name.length < 100, "a name is short");
  assert(state.password.length < 1000, "a password is short");
  const answer = await postLogin({ name: state.name, password: state.password });
  if (answer.ok) {
    set({ ...state, error: "", signed: true, token: answer.value.token });
  } else {
    set({ ...state, error: answer.error.error, signed: false });
  }
}

export function LoginScreen() {
  const [state, setState] = useScreen<LoginState>({
    name: "",
    password: "",
    error: "",
    signed: false,
    token: "",
  });
  return (
    <main className="mx-auto max-w-xl p-6">
      <Card>
        <CardHeader>
          <CardTitle>Sign in</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <Label htmlFor="name">Name</Label>
          <Input
            id="name"
            value={state.name}
            onChange={(event) => {
              setState({ ...state, name: event.target.value });
            }}
          />
          <Label htmlFor="password">Password</Label>
          <Input
            id="password"
            type="password"
            value={state.password}
            onChange={(event) => {
              setState({ ...state, password: event.target.value });
            }}
          />
          <p role="alert">{state.error}</p>
          <p>{state.signed ? `Signed in as ${state.name}.` : ""}</p>
          <Button
            onClick={() => {
              signIn(state, setState).catch(() => {
                setState({ ...state, error: "The server did not answer." });
              });
            }}
          >
            Sign in
          </Button>
        </CardContent>
      </Card>
    </main>
  );
}
