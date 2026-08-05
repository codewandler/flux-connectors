op jira-issue-get(issue_key: String) -> Any
  description "Read one Jira issue by key (`PROJ-123`) or numeric id — summary, description, status, assignee, reporter and every other configured field. Returns Jira's full default field set, including custom fields, because narrowing it needs the `fields` query parameter this connector cannot encode. Text fields are wiki markup, not rich content"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{site}.atlassian.net"
  url = fmt("{base}/rest/api/2/issue/{issue_key}")
  response = http.request(method: "GET", url)
  return response
