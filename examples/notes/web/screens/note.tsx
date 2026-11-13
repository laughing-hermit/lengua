import { defineCatalog, type Spec } from "@json-render/core";
import { Renderer, defineRegistry } from "@json-render/react";
import { schema } from "@json-render/react/schema";
import { useEffect } from "react";
import { z } from "zod";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import {
  assert,
  getNotesById,
  postUi,
  queryParam,
  useScreen,
  type PostUiAnswer,
} from "@/lib/lengua";

interface NoteState {
  readonly id: string;
  readonly title: string;
  readonly text: string;
  readonly error: string;
  readonly explained: boolean;
  readonly spec: Spec | null;
}

const catalog = defineCatalog(schema, {
  components: {
    Card: { props: z.object({ title: z.string() }), description: "A card with a title" },
    Text: { props: z.object({ content: z.string() }), description: "A line of text" },
  },
  actions: {},
});

const { registry } = defineRegistry(catalog, {
  components: {
    Card: ({ props, children }) => (
      <Card>
        <CardHeader>
          <CardTitle>{props.title}</CardTitle>
        </CardHeader>
        <CardContent>{children}</CardContent>
      </Card>
    ),
    Text: ({ props }) => <p>{props.content}</p>,
  },
});

export function toSpec(answer: PostUiAnswer): Spec {
  assert(answer.root in answer.elements, "the root of the description is one of its parts");
  assert(answer.elements.card.children.length > 0, "the card has a part in it");
  return {
    root: answer.root,
    elements: {
      card: {
        type: answer.elements.card.type,
        props: answer.elements.card.props,
        children: answer.elements.card.children,
      },
      text: {
        type: answer.elements.text.type,
        props: answer.elements.text.props,
        children: [],
      },
    },
  };
}

export async function loadNote(state: NoteState, set: (next: NoteState) => void): Promise<void> {
  assert(state.id !== "", "the screen has the number of a note");
  const answer = await getNotesById(state.id);
  assert(answer.status >= 100, "the server answered");
  if (answer.ok) {
    set({ ...state, title: answer.value.title, text: answer.value.text, error: "" });
  } else {
    set({ ...state, error: answer.error.error });
  }
}

export async function explain(state: NoteState, set: (next: NoteState) => void): Promise<void> {
  assert(state.id !== "", "the screen has the number of a note");
  const answer = await postUi({ id: Number(state.id) });
  assert(answer.status >= 100, "the server answered");
  if (answer.ok) {
    set({ ...state, explained: true, spec: toSpec(answer.value) });
  } else {
    set({ ...state, error: answer.error.error });
  }
}

export function NoteScreen() {
  const [state, setState] = useScreen<NoteState>({
    id: "",
    title: "",
    text: "",
    error: "",
    explained: false,
    spec: null,
  });
  useEffect(() => {
    const id = state.id === "" ? queryParam("id") : state.id;
    loadNote({ ...state, id }, setState).catch(() => {
      setState({ ...state, error: "The note did not load." });
    });
  }, []);
  return (
    <main className="mx-auto max-w-xl p-6">
      <Card>
        <CardHeader>
          <CardTitle>{state.title}</CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-3">
          <p>{state.text}</p>
          <p role="alert">{state.error}</p>
          <Button
            onClick={() => {
              explain(state, setState).catch(() => {
                setState({ ...state, error: "The summary did not load." });
              });
            }}
          >
            Explain
          </Button>
          {state.spec === null ? null : <Renderer spec={state.spec} registry={registry} />}
        </CardContent>
      </Card>
    </main>
  );
}
