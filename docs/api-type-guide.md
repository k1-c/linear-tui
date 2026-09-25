# API Type Definition Guide

This document describes how to define, verify, and maintain Rust types that map to the Linear GraphQL API.

## Verifying the Linear GraphQL Schema

Before defining or modifying types, check the actual API schema.

### Introspection query

Introspection is answered **without authentication**, so schema questions can be
settled from any machine:

```bash
curl -s https://api.linear.app/graphql \
  -H "Content-Type: application/json" \
  -d '{"query":"{ __type(name: \"WorkflowState\") { fields { name type { name kind enumValues { name } ofType { name kind } } } } }"}' | jq
```

Useful variants: `__type(name: "Query") { fields { name args { name } } }` to
find a query's arguments, and `__type(name: "IssueCreateInput") { inputFields
{ name } }` for mutation inputs.

### Check enum values

```bash
curl -s https://api.linear.app/graphql \
  -H "Content-Type: application/json" \
  -d '{"query":"{ __type(name: \"WorkflowStateType\") { enumValues { name } } }"}' | jq
```

### References

- [Linear API Docs](https://developers.linear.app/docs/graphql/working-with-the-graphql-api)

Introspection is the source of truth. Third-party copies of the schema drift, so
settle questions against the live endpoint rather than a mirrored file.

## Type Definition Checklist

When adding or modifying types in `src/entity/` (one file per aggregate —
`issue.rs`, `team.rs`, `project.rs`, …; the API decodes straight into them):

- [ ] **Schema verification**: Run introspection query to confirm field names, types, and nullability
- [ ] **Open value sets**: A field the schema types as `String` (like `WorkflowState.type`) can gain values at any time. Map it with a custom `Deserialize` that accepts every spelling seen (`canceled` and `cancelled`) and falls back to an `Unknown` variant, because failing on an unseen value would fail the whole query
- [ ] **Ids**: An entity id is its own newtype from `src/entity/ids.rs` (`IssueId`, `TeamId`, …), not a bare `String`; see [Ids](#ids)
- [ ] **Optional fields**: Fields that may be `null` in the API must be `Option<T>` with `#[serde(default)]`
- [ ] **Numeric types**: Linear returns `priority` as a float (e.g., `0.0` not `0`). Use custom `Deserialize` for type coercion
- [ ] **Nested types**: Verify nested object structure matches: `{ nodes { ... } }` maps to `Connection<T>`
- [ ] **Fixture test**: Add or update a JSON fixture in `tests/fixtures/` and a corresponding deserialization test

## Serde Attribute Reference

| Attribute                                           | Use case                                      |
| --------------------------------------------------- | --------------------------------------------- |
| `#[serde(rename = "fieldName")]`                    | Map a snake_case field to its camelCase name  |
| `#[serde(alias = "variant")]`                       | Accept multiple spellings for enum variants   |
| `#[serde(default)]`                                 | Handle missing/null fields with Default trait |
| `#[serde(skip_serializing_if = "Option::is_none")]` | Omit None in serialization                    |
| Custom `impl Deserialize`                           | Complex coercion (e.g., f64 → enum)           |

## GraphQL Variable Types

Linear's GraphQL schema uses different scalar types depending on context:

| Context                               | Rust variable | GraphQL type | Example                                              |
| ------------------------------------- | ------------- | ------------ | ---------------------------------------------------- |
| Filter comparator (`IDComparator.eq`) | `$teamId`     | `ID!`        | `filter: { team: { id: { eq: $teamId } } }`          |
| Direct query argument                 | `$id`         | `String!`    | `team(id: $id)`, `issue(id: $id)`                    |
| Mutation input field                  | `$stateId`    | `String!`    | `issueUpdate(id: $id, input: { stateId: $stateId })` |
| Optional mutation input               | `$assigneeId` | `String`     | `input: { assigneeId: $assigneeId }`                 |

**Rule**: If the variable is used inside a `filter: { ... { eq: $var } }` block, use `ID!`. Otherwise use `String!`.

## Workflow: Adding a New API Type

1. **Introspect**: Query the schema to get the field names, types, and nullability
2. **Fetch sample**: Use curl to get a real response and save it (anonymize sensitive data)
3. **Define type**: Add struct/enum in the aggregate's file under `src/entity/` with appropriate serde attributes
4. **Add fixture**: Save the sample response to `tests/fixtures/<query_name>.json`
5. **Add test**: Write a deserialization test in `src/adapter/api/decode_tests.rs`, including a case where the new field is absent
6. **Add the query**: Use the `connection` or `mutate` helper in `src/adapter/api/client.rs` (see [Queries](#queries)) and cover it with a wiremock test there
7. **Run tests**: `cargo test` to verify the type matches the real API response

## Known API Quirks

| Field                | Quirk                              | Solution                                               |
| -------------------- | ---------------------------------- | ------------------------------------------------------ |
| `WorkflowState.type` | Returns `"canceled"` (US spelling); new values can appear | Custom `Deserialize` on `StateType`: both spellings, anything else `Unknown` |
| `Issue.priority`     | Returns as float (`0.0`) not int   | Custom `Deserialize` that accepts the five exact levels; anything else is `None` |
| `Issue.description`  | Can be `null`                      | `Option<String>` with `#[serde(default)]`              |
| `Issue.comments`     | Only present in detail query       | `Option<Connection<Comment>>` with `#[serde(default)]` |
| `Issue.assignee`     | Can be unassigned (`null`)         | `Option<User>`                                         |
| `Issue.url`, `Issue.branchName` | Absent from older cached payloads | `Option<String>` with `#[serde(default)]` |

## Shared Field Selections

Fields that every issue query needs live in the `ISSUE_FIELDS` constant in
`src/adapter/api/client.rs`, interpolated into each query with `format!`. Add new issue
fields there so list rows and detail responses stay interchangeable, rather than
extending a single query's selection set.

## Ids

Every entity id gets a newtype from `src/entity/ids.rs`, generated by the
`id_type!` macro: `#[serde(transparent)]`, so it reads and writes as a bare
string, with `Display` and comparison against `&str` for tests. A new entity
adds its name to the macro call. `Ref<I>` covers a reference that selects only
`{ id }`.

Only ids are wrapped. Keep as `String`:

- names, titles, and human-readable keys like the `ENG-123` identifier;
- values a workspace defines for itself, such as state names, `Project.state`,
  `Favorite.type`, which are data to display, not handles to pass back;
- page cursors, which are opaque and not compared.

## Queries

`LinearClient` methods return `Result<_, ApiError>`, not `anyhow`. The variants
separate cases that a caller may handle differently: `Transport`,
`Unauthorized`, `RateLimited`, `Http`, `GraphQL` (with each error's
`extensions.code`), `Decode`, and `Rejected` for a mutation that answered
`success: false`.

- **Name every operation**: `query TeamIssues(…)`, `mutation UpdateIssue(…)`.
  The name is what the log records.
- **Lists** go through `connection(query, variables, "/json/pointer")`, which
  pulls the `{ nodes pageInfo }` object at that pointer out of `data`.
- **Mutations** go through `mutate(query, variables, "payloadField", reason)`,
  which checks `success`. Linear reports a refused mutation that way rather
  than as an error, and the UI has already applied the change optimistically.
- **Tests** point a client at a wiremock server with
  `LinearClient::with_endpoint`, and credentials with `StaticCredentials`.
