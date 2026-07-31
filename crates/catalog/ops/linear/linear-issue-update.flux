op linear-issue-update(id: String, title: String, description: String, priority: Number, stateId: String, assigneeId: String) -> Any
  description "Change an existing issue's title, description, priority, workflow state or assignee. Every argument except `id` is optional and only the ones supplied are changed. Reassigning or moving an issue notifies the people watching it. Read the issue first with linear-issue-get if the current values matter. Linear answers every failure with HTTP 200 and an `errors` array beside a null `data`, so check `errors` and the payload's `success` flag before treating the change as applied"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.linear.app"
  url = fmt("{base}/graphql")
  content_type = "application/json"
  query = """mutation IssueUpdate($id: String!, $title: String, $description: String, $priority: Int, $stateId: String, $assigneeId: String) {
  issueUpdate(
    id: $id
    input: {title: $title, description: $description, priority: $priority, stateId: $stateId, assigneeId: $assigneeId}
  ) {
    success
    issue {
      id
      identifier
      title
      url
      priority
      state {
        id
        name
      }
    }
  }
}
"""
  payload = { query, variables: { assigneeId, description, id, priority, stateId, title } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
