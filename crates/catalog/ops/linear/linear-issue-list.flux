op linear-issue-list(first: Number, after: String) -> Any
  description "List issues in the workspace, most recently updated first, with a page cursor. Pass `after` with the previous response's `pageInfo.endCursor` to get the next page, and stop when `pageInfo.hasNextPage` is false. Note that the cursor travels as a GraphQL variable rather than a query parameter, so this connector declares no pagination quirk and a host cannot page it automatically. Linear answers every failure with HTTP 200 and an `errors` array beside a null `data`, so check `errors` before reading `data`"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.linear.app"
  url = fmt("{base}/graphql")
  content_type = "application/json"
  query = """query Issues($first: Int!, $after: String) {
  issues(first: $first, after: $after, orderBy: updatedAt) {
    nodes {
      id
      identifier
      title
      url
      priority
      updatedAt
      state {
        name
        type
      }
      assignee {
        id
        name
      }
    }
    pageInfo {
      hasNextPage
      endCursor
    }
  }
}
"""
  payload = { query, variables: { after, first } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
