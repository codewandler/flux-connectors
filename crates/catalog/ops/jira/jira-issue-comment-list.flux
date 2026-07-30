op jira-issue-comment-list(issue_key: String) -> Any
  description "Read the comments on a Jira issue, oldest first. Returns Jira's first default-sized page only: paging needs the `startAt` and `maxResults` query parameters this connector cannot encode, so an issue with a long discussion is truncated. Comment bodies are wiki markup"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://{site}.atlassian.net"
  $url = fmt("{base}/rest/api/2/issue/{issue_key}/comment")
  $response = http.request({ method: "GET", url: $url })
  return $response
