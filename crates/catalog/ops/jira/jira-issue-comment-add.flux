op jira-issue-comment-add(issue_key: String, body: String) -> Any
  description "Add a comment to a Jira issue. The comment is visible to everyone who can see the issue — including customers on a service-management portal — and notifies its watchers; restricting it to a project role or group is not supported yet. The body is Jira wiki markup (`*bold*`, `{code}`), not Markdown and not rich content"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  $base = "https://{site}.atlassian.net"
  $url = fmt("{base}/rest/api/2/issue/{issue_key}/comment")
  $content_type = "application/json"
  $payload = { body: $body }
  $response = http.request({ body: $payload, headers: { "content-type": $content_type }, method: "POST", url: $url })
  return $response
