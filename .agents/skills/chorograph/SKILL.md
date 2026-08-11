---
name: chorograph
description: Declare software architecture in doc comments so the codebase always renders an accurate architecture map. Use when writing or modifying services, endpoints, jobs, databases, queues, events, or external integrations in a codebase that uses chorograph.
---

# Declaring architecture with chorograph

chorograph renders an architecture map from **doc comments on the real code**. Nothing is
imported or executed; the tags below are the entire mechanism. Your job when writing code in a
chorograph codebase: every architecturally significant thing you create or change must carry its
annotation, in the same comment where you'd document it anyway. The map is generated with
`chorograph render <dirs>`, and it shows exactly what is annotated, nothing else. Undeclared code is
invisible; stale edges are lies. Both are bugs you prevent by following this skill.

## The shape of an annotation

One comment declares one node, plus that node's edges. The prose becomes the node's description;
free text after an edge target becomes the edge's label. Use it to say **why**:

```ts
/**
 * Places an order and charges it synchronously.
 * @endpoint POST /orders
 * @writes orders-db.orders
 * @writes orders-db.order_items
 * @emits order.placed so notifications and analytics can react
 * @calls payments.post-charge charge at checkout, in-process for now
 */
export async function placeOrder(items: OrderItem[]): Promise<Order> { … }
```

## Node tags

| tag | declares | notes |
| --- | --- | --- |
| `@system Name` | the map's title | once per codebase; prose = system description |
| `@domain Name` | a bounded context | `in:Parent` to nest domains |
| `@service name` | a deployable process | `in:Domain` · sets the file's service context |
| `@module [name]` | a code grouping: package, library, class | not deployable; name defaults to the documented class |
| `@endpoint POST /orders` | an API surface | name it after the route |
| `@fn [name]` | a significant function | name defaults to the documented function |
| `@job [name]` | scheduled/background work | name defaults to the documented function |
| `@database name` | a database | `tables:orders,order_items` declares its tables inline |
| `@table name` | one table | `in:db-name`, or the file's `@database` context |
| `@cache` / `@bucket` / `@queue` | Redis / S3 / SQS and friends | |
| `@event order.placed` | a named domain event | dots in the name are fine |
| `@external Stripe` | a third party you don't operate | |
| `@of parent` | file directive, own comment | everything below attaches to `parent` |

Every node tag also accepts `tech:"PostgreSQL 16"` (quote values with spaces) and
`tags:critical,pci`. `in:` and `of:` are the same key ("my parent is"), written whichever way
reads better (`in:Billing`, `of:api-gateway`).

## Hierarchy

Containment nests as deep as the design does, governed by one matrix:

| container | can hold |
| --- | --- |
| domain | domains, services, modules, databases, caches, buckets, queues, events, externals |
| service | modules, endpoints, functions, jobs, and its private databases/caches/buckets/queues |
| module | modules, functions, jobs |
| endpoint | endpoints (resource groups), functions |
| function | functions |
| job | functions |
| database | tables |

How a node finds its parent, in precedence order:

1. **An explicit `in:`/`of:` key**: a name (`of:api-gateway`) or dotted path when the name isn't
   unique (`of:orders.post-orders`). Case decides ties: `in:Identity` is the domain, `identity`
   the service.
2. **File context**: the nearest preceding container that can hold the member — a `@fn` attaches
   to the file's `@module`, else its `@endpoint`, else its `@service`; a `@table` to the file's
   `@database`; a `@module` to the file's `@service` or `@domain` (never to another module —
   nest modules explicitly with `in:`). Declare the container at the top of its file and members
   below need nothing.
3. **The file's `@of` directive**: a comment of its own, `/** @of api-gateway */`, near the top
   of a file whose parent is declared elsewhere. This is how a large service splits into
   `routes/*.ts` files (the service is declared once, each routes file carries one `@of`), and
   how a package spanning many files hangs everything off one `@module`.

Members (`@endpoint`, `@fn`, `@job`, `@table`) with no parent are errors. An endpoint floating
outside any service is not architecture.

Use depth to show *where in the design* something lives: a function that implements one
endpoint's rule goes inside that endpoint (`@fn of:post-orders`); a helper the whole service
shares sits directly under the service; a cache only one service touches is declared after that
service's `@service` comment so it renders inside the service, and any other service reading it
is visibly reaching into private infrastructure.

## Edge tags

Declared in the same comment as the node doing the verb: first token is the target, everything
after it is the label. Always phrase the label as the *why* or the protocol.

| tag | meaning |
| --- | --- |
| `@calls target [why]` | request/response |
| `@reads target [why]` / `@writes target [why]` | store access |
| `@emits event [why]` / `@consumes event [why]` | event flow (target must be an @event or @queue) |
| `@uses target [why]` | escape hatch; prefer a specific verb |

**Targets are names, resolved when the map builds.** Use the bare name when it's unique
(`session-cache`, `Stripe`, `order.placed`), or qualify with a dot when it isn't
(`orders-db.orders`, `payments.post-charge`; endpoint names slug as `post-charge`). A target
that matches nothing or several things fails the render with file:line and suggestions. That
failure is the freshness check, so never silence it by deleting the edge unless the code really
stopped doing the thing.

## Rules

1. **Annotate at creation.** New service, endpoint, job, table, queue, event, or third-party
   integration ⇒ its comment lands in the same commit, on the same code.
2. **Edges live with the actor.** When code starts reading a table, calling a service, or
   emitting an event, add the tag to *that* code's comment. When it stops, remove the tag.
3. **Prose first, tags after.** The sentence above the tags is the description shown in the map's
   detail panel. Write it for the next engineer.
4. **`@system` exactly once**, in an architecture anchor file alongside `@domain` and `@external`
   declarations (free-standing comments with no code attached are fine).
5. **Names are unique within their parent** and become slug ids (`orders/orders/post-orders`).
6. **Keep names stable.** Renaming a node breaks every edge pointing at it; the render will list
   them; fix them in the same commit.

## Granularity: two valid modes

**Map mode** (the default): annotate a function only when it is a *load-bearing part of the
design* — it owns a rule (pricing, fees, auth), it is the single place something happens, or it
has its own architectural edges. A service with three to seven `@fn` nodes is a map.

**Full-coverage mode** (when the codebase decides the map should explain *everything*): every
package and class is a `@module`, every documented function carries `@fn`, and the hierarchy —
domain → service/module → class → function — does the organizing that keeps it readable. In
this mode the prose matters most: say *how* the function does its job (“regex over known
patterns”, “asks the small model”), because that's what a reviewer scans for. Follow whichever
mode the codebase already uses.

## Multi-repo systems: anchor files

When the system spans repositories, each repo keeps one anchor file (conventionally
`chorograph/anchor.ts`) holding the shared context it needs to render alone: the `@system` and
`@domain` declarations, shared databases/events/externals, and stub declarations for
sibling-repo services this repo has edges to. Everything the repo *owns* stays out of the
anchor and in the code.

Rendering the whole estate together, anchor declarations yield to real ones:

```bash
npx chorograph render <repo-dirs…> --anchors 'chorograph/anchor.ts'
```

A real declaration replaces its anchor stub; identical anchor declarations from different repos
merge; anchor databases union their tables. Keep anchors consistent across repos — every anchor
must agree on a shared node's kind and `in:` domain, or the estate render fails.

## Verifying

```bash
npx chorograph render <dirs…> --json --quiet   # builds the map, errors on broken annotations
```

Run it after changing annotations. Dangling targets, ambiguous targets, duplicate names,
members with no service in scope, and non-event `@emits`/`@consumes` targets all fail with
`file:line` and suggestions.
