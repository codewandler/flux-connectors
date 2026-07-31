op linear-team-list(first: Number) -> Any
  description "List the teams in this workspace with each one's id, key and name. The `id` returned here is what linear-issue-create takes as `teamId`, and the `key` is the prefix in an issue's identifier — the ENG in ENG-42. Returns the first page only; raise `first` to widen it. Linear answers every failure with HTTP 200 and an `errors` array beside a null `data`, so check `errors` before reading `data`"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.linear.app"
  url = fmt("{base}/graphql")
  content_type = "application/json"
  query = """query Teams($first: Int!) {
  teams(first: $first) {
    nodes {
      id
      key
      name
      description
    }
  }
}
"""
  payload = { query, variables: { first } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
