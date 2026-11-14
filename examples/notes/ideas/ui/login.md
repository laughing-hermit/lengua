# Purpose

The screen is a form to sign in with a name and a password. It sends them to the server, and it
keeps the token of the session that the server gives.

# Behaviours

## Sign in with the right password

Given the name `ann` and her password,
When the user clicks `Sign in`,
Then the screen shows that she is signed in.

### Example: the right password

- Type into `Name`: "ann"
- Type into `Password`: "green tea"
- Click: `Sign in`
- Expect text: "Signed in as ann."
- Expect state `signed`: true

## Show the error of a wrong password

Given the name `ann` and a wrong password,
When the user clicks `Sign in`,
Then the screen shows the error of the server.

### Example: a wrong password

- Type into `Name`: "ann"
- Type into `Password`: "black tea"
- Click: `Sign in`
- Expect text: "The name or the password is wrong."
- Expect state `signed`: false

# Constraints

- Uses: `notes`

# Structure

- `LoginScreen`: shows the form, the error and the result.
- `signIn`: sends the name and the password to the server, and keeps the token or the error.
