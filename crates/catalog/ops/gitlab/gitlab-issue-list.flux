op gitlab-issue-list(project_id: Number, state: String, page: Number, per_page: Number) -> Any
  description "List issues in a project, optionally filtered by state, newest activity first"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://gitlab.com/api/v4"
  url = fmt("{base}/projects/{project_id}/issues")
  response = http.request(method: "GET", query: { page, per_page, state }, url)
  return response
