# Purpose

The module reads a basket from the query of a request, as pairs of an item and a count, and
gives its net sum. A count is from 1 to 1000.

# Behaviours

## Add the largest count

Given a basket of 1000 `jam`,
When a client sends `GET /total?jam=1000`,
Then the net sum is 4750.00.

### Example: 1000 `jam`

- Request: `GET /total?jam=1000`
- Expect status: 200
- Expect body: "{\"net\":\"4750.00\",\"tax\":\"380.00\",\"total\":\"5130.00\"}"

## Refuse a count that is not valid

Given a count of 0, of 1001 or no count,
When a client sends the basket,
Then the answer is 400 with the error `no count`.

### Example: a count of 1001

- Request: `GET /total?jam=1001`
- Expect status: 400
- Expect body: "{\"error\":\"no count for jam\"}"

### Example: a count of 0

- Request: `GET /total?tea=0`
- Expect status: 400
- Expect body: "{\"error\":\"no count for tea\"}"

### Example: no count

- Request: `GET /total?tea`
- Expect status: 400
- Expect body: "{\"error\":\"no count for tea\"}"

## Refuse an item that is not in the catalog

Given the item `soap`, which the shop does not have,
When a client sends the basket,
Then the answer is 400 with the error `no item soap`.

### Example: `soap`

- Request: `GET /total?soap=1`
- Expect status: 400
- Expect body: "{\"error\":\"no item soap\"}"

# Structure

- `parse_query`: reads the pairs of the query as items and counts, or gives an error.
- `net`: gives the sum of the prices times the counts, or an overflow.
