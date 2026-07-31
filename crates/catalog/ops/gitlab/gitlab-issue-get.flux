op gitlab-issue-get(project_id: Number, issue_iid: Number) -> Any
  description "Get one issue by its project-scoped number (iid), not its global id"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://gitlab.com/api/v4"
  url = fmt("{base}/projects/{project_id}/issues/{issue_iid}")
  response = http.request(method: "GET", url)
  return response
