op linear-issue-create(teamId: String, title: String, description: String, priority: Number, assigneeId: String) -> Any
  description "Create an issue on a team. Linear does not deduplicate: creating the same title twice makes two issues and notifies the team twice. The created issue, with its assigned identifier and URL, is in the response. `teamId` comes from linear-team-list. Linear answers every failure with HTTP 200 and an `errors` array beside a null `data`, so check `errors` and the payload's `success` flag before treating the issue as created"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.linear.app"
  url = fmt("{base}/graphql")
  content_type = "application/json"
  query = """mutation IssueCreate($teamId: String!, $title: String!, $description: String, $priority: Int, $assigneeId: String) {
  issueCreate(
    input: {teamId: $teamId, title: $title, description: $description, priority: $priority, assigneeId: $assigneeId}
  ) {
    success
    issue {
      id
      identifier
      title
      url
    }
  }
}
"""
  payload = { query, variables: { assigneeId, description, priority, teamId, title } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
