# Purpose

The program is a small notes application. It is one WebAssembly file with two parts: the
server and the screens. The server sends the screens to the browser, with their fonts, and it
keeps a list of notes. The screens show the list, one note, and a form for a new note.

# Behaviours

## Send the page that starts the screens

Given a browser that sends a request for the path `/`,
When the server gets the request,
Then the answer is 200 with the HTML page that loads the screens.

### Example: the page

- Request: `GET /`
- Expect status: 200
- Expect header `content-type`: "text/html; charset=utf-8"

## Send the code of the screens

Given a browser that loaded the page,
When it sends a request for the code and the loader of the screens,
Then the answer is 200 with the WebAssembly and the script that start them.

### Example: the code

- Request: `GET /app_bg.wasm`
- Expect status: 200
- Expect header `content-type`: "application/wasm"

### Example: the loader

- Request: `GET /app.js`
- Expect status: 200
- Expect header `content-type`: "text/javascript; charset=utf-8"

## Send the list of notes

Given the two notes of the server,
When the screens send `GET /api/notes`,
Then the answer is 200 with the notes as JSON.

### Example: the notes

- Request: `GET /api/notes`
- Expect status: 200
- Expect header `content-type`: "application/json"
- Expect body: "[{\"id\":\"1\",\"title\":\"Milk\",\"text\":\"Buy two bottles.\"},{\"id\":\"2\",\"title\":\"Call\",\"text\":\"Call the shop at noon.\"}]"

## Answer 404 for a path that is not there

Given the path `/other`,
When a browser sends a request for it,
Then the answer is 404 with an error in JSON.

### Example: another path

- Request: `GET /other`
- Expect status: 404
- Expect body: "{\"error\":\"no page\"}"

# Constraints

- Serves: HTTP on TCP port 7812
- Memory: 256 MiB
- Time: 10 s
- Modules: `ui/list`, `ui/note`, `ui/edit`, `notes`
- Uses: `../../libraries/json`
- Uses crate: `wasip2` 2.0.1, for the interface of the server to its host, the HTTP world of `WASI`

# Structure

- `Notes::handle`: answers a request with a file of the screens, the notes, or an error.
- `body_of`: reads the body of a request, up to the limit.
- `answer`: gives the status, the media type and the bytes of the answer to a path.
- `send`: writes the status, the media type and the bytes to the answer.
