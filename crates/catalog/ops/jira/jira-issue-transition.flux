op jira-issue-transition(issue_key: String, transition_id: String) -> Any
  description "Move a Jira issue through a workflow transition — the way an issue is closed, reopened or advanced. Get the transition id from `jira-issue-transitions-list` first; ids are per-workflow and only transitions valid from the current status are accepted. The status change is visible on every board showing the issue, notifies watchers, and may fire project automation. Answers `204` with an empty body on success"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://{site}.atlassian.net"
  $url = fmt("{base}/rest/api/2/issue/{issue_key}/transitions")
  $content_type = "application/json"
  $payload = { transition: { id: $transition_id } }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
  return $response
