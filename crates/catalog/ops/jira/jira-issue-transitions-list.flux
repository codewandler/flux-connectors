op jira-issue-transitions-list(issue_key: String) -> Any
  description "List the workflow transitions available on a Jira issue right now, with the id and target status of each. Call this before `jira-issue-transition`: transition ids are per-workflow, not global, and only the transitions valid from the issue's current status are returned"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  $base = "https://{site}.atlassian.net"
  $url = fmt("{base}/rest/api/2/issue/{issue_key}/transitions")
  $response = http.request({ method: "GET", url: $url })
  return $response
