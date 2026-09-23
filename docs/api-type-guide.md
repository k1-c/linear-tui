# API Type Definition Guide

This document describes how to define, verify, and maintain Rust types that map to the Linear GraphQL API.

## Verifying the Linear GraphQL Schema

Before defining or modifying types, always check the actual API schema.

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
- [Linear GraphQL Schema (GitHub)](https://github.com/nicepkg/nice-cmd/blob/main/packages/linear-sdk/schema.graphql)

## Type Definition Checklist

When adding or modifying types in `src/api/types.rs`:

- [ ] **Schema verification**: Run introspection query to confirm field names, types, and nullability
- [ ] **Enum spelling**: Linear uses American English (`canceled`, not `cancelled`). Always add `#[serde(alias = "...")]` for spelling variants
- [ ] **Optional fields**: Fields that may be `null` in the API must be `Option<T>` with `#[serde(default)]`
- [ ] **Numeric types**: Linear returns `priority` as a float (e.g., `0.0` not `0`). Use custom `Deserialize` for type coercion
- [ ] **Nested types**: Verify nested object structure matches — `{ nodes { ... } }` maps to `Connection<T>`
- [ ] **Fixture test**: Add or update a JSON fixture in `tests/fixtures/` and a corresponding deserialization test

## Serde Attribute Reference

| Attribute                                           | Use case                                      |
| --------------------------------------------------- | --------------------------------------------- |
| `#[serde(rename_all = "camelCase")]`                | Map Rust snake_case to GraphQL camelCase      |
| `#[serde(rename = "fieldName")]`                    | Rename a single field                         |
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

1. **Introspect**: Query the schema to get the exact field names, types, and nullability
2. **Fetch sample**: Use curl to get a real response and save it (anonymize sensitive data)
3. **Define type**: Add struct/enum in `src/api/types.rs` with appropriate serde attributes
4. **Add fixture**: Save the sample response to `tests/fixtures/<query_name>.json`
5. **Add test**: Write a deserialization test in `tests/api_types.rs`
6. **Run tests**: `cargo test` to verify the type matches the real API response

## Known API Quirks

| Field                | Quirk                              | Solution                                               |
| -------------------- | ---------------------------------- | ------------------------------------------------------ |
| `WorkflowState.type` | Returns `"canceled"` (US spelling) | `#[serde(alias = "canceled")]` on `Cancelled` variant  |
| `Issue.priority`     | Returns as float (`0.0`) not int   | Custom `Deserialize` impl that handles `f64`           |
| `Issue.description`  | Can be `null`                      | `Option<String>` with `#[serde(default)]`              |
| `Issue.comments`     | Only present in detail query       | `Option<Connection<Comment>>` with `#[serde(default)]` |
| `Issue.assignee`     | Can be unassigned (`null`)         | `Option<User>`                                         |
| `Issue.url`, `Issue.branchName` | Absent from older cached payloads | `Option<String>` with `#[serde(default)]` |

## Shared Field Selections

Fields that every issue query needs live in the `ISSUE_FIELDS` constant in
`src/api/client.rs`, interpolated into each query with `format!`. Add new issue
fields there so list rows and detail responses stay interchangeable, rather than
extending a single query's selection set.
