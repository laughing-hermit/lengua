# Purpose

The module sends each path to its page, and writes the answers and the errors as JSON. The
total has the net sum, the tax from the component in C and their sum, each with two places.

# Behaviours

## Give the total of one item

Given a basket of one `tea`,
When a client sends `GET /total?tea=1`,
Then the tax is 0.20 and the total is 2.70.

### Example: one `tea`

- Request: `GET /total?tea=1`
- Expect status: 200
- Expect body: "{\"net\":\"2.50\",\"tax\":\"0.20\",\"total\":\"2.70\"}"

# Structure

- `route`: gives the status and the body of the page of a path.
- `total`: gives the net sum, the tax and the total of a basket, or an error.
- `error`: gives a status and a JSON object with the error.
