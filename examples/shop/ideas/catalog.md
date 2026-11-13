# Purpose

The module holds the items of the shop and their prices in cents. The item `tea` is 2.50, the
item `cake` is 3.20 and the item `jam` is 4.75.

# Behaviours

## Give the prices with two places

Given the price of `jam`, 475 cents,
When a client sends `GET /items`,
Then the price of `jam` is `4.75`.

### Example: the price of `jam`

- Request: `GET /items`
- Expect status: 200
- Expect body: "[{\"name\":\"tea\",\"price\":\"2.50\"},{\"name\":\"cake\",\"price\":\"3.20\"},{\"name\":\"jam\",\"price\":\"4.75\"}]"

# Structure

- `find`: gives the item with a name, or nothing.
- `listing`: gives the items as a JSON list of names and prices.
