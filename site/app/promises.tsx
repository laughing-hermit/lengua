/** What a shared spec promises: only what lengua enforces. */
export function Promises() {
  return (
    <ul className="promises">
      <li>
        <strong>The spec travels.</strong> Every copy compiled from it passes the same checks: its tests,
        lengua&apos;s rules and the proofs. That&apos;s the promise, never the same program: two computers
        get two programs, which can differ wherever the spec leaves room.
      </li>
      <li>
        <strong>Each computer compiles and checks its own code.</strong> No code is shared here, and none
        is trusted from anywhere else.
      </li>
      <li>
        <strong>The guarantees hold through lengua.</strong> main.s is plain text anyone can build outside
        lengua, so they hold only for a program run through lengua&apos;s own <code>run</code>.
      </li>
      <li>
        <strong>A proof covers what its line says.</strong> For every length only for programs in the
        shapes lengua&apos;s design lists, and up to a length otherwise; never that every program is proven.
      </li>
    </ul>
  );
}
