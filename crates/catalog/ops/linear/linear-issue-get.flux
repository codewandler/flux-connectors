op linear-issue-get(id: String) -> Any
  description "Read one issue by its id or by its human identifier such as ENG-42, including its title, description, priority, state, assignee, team and URL. Use it to confirm an issue exists and to read its current state before updating it. Linear answers every failure with HTTP 200 and an `errors` array beside a null `data`, so check `errors` before reading `data`; a missing issue comes back that way rather than as a 404"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.linear.app"
  url = fmt("{base}/graphql")
  content_type = "application/json"
  query = """query Issue($id: String!) {
  issue(id: $id) {
    id
    identifier
    title
    description
    url
    priority
    createdAt
    updatedAt
    state {
      id
      name
      type
    }
    assignee {
      id
      name
    }
    team {
      id
      key
    }
  }
}
"""
  payload = { query, variables: { id } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
