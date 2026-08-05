op gitlab-branch-list(project_id: Number, page: Number, per_page: Number) -> Any
  description "List a project's repository branches"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "{origin}/api/v4"
  url = fmt("{base}/projects/{project_id}/repository/branches")
  response = http.request(method: "GET", query: { page, per_page }, url)
  return response
