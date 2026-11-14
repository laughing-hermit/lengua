import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { assert, getMe, postLogin, useScreen } from "@/lib/lengua";

interface LoginState {
  readonly name: string;
  readonly password: string;
  readonly error: string;
  readonly signed: boolean;
  readonly token: string;
  readonly who: string;
}

export async function signIn(state: LoginState, set: (next: LoginState) => void): Promise<void> {
  assert(state.name.length < 100, "a name is short");
  assert(state.password.length < 1000, "a password is short");
  const answer = await postLogin({ name: state.name, password: state.password });
  if (!answer.ok) {
    set({ ...state, error: answer.error.error, signed: false });
    return;
  }
  const token = answer.value.token;
  const me = await getMe(token);
  if (me.ok) {
    set({ ...state, error: "", signed: true, token, who: me.value.name });
  } else {
    set({ ...state, error: me.error.error, signed: false, token: "" });
  }
}

export function LoginScreen() {
  const [state, setState] = useScreen<LoginState>({
    name: "",
    password: "",
    error: "",
    signed: false,
    token: "",
    who: "",
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
          <p>{state.signed ? `Signed in as ${state.who}.` : ""}</p>
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
