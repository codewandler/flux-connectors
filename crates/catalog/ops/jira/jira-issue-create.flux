op jira-issue-create(project_key: String, summary: String, issue_type: String) -> Any
  description "Create a Jira issue in a project. Visible to everyone with access to the project and notifies its watchers. Only project, summary and issue type are set: a description cannot be sent yet (see the connector's notes), so file the detail as a comment afterwards. Returns the new issue's id, key and self URL"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://{site}.atlassian.net"
  $url = fmt("{base}/rest/api/2/issue")
  $content_type = "application/json"
  $payload = { fields: { issuetype: { name: $issue_type }, project: { key: $project_key }, summary: $summary } }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
  return $response
