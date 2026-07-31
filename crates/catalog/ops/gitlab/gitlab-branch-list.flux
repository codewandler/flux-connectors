op gitlab-branch-list(project_id: Number, page: Number, per_page: Number) -> Any
  description "List a project's repository branches"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://gitlab.com/api/v4"
  url = fmt("{base}/projects/{project_id}/repository/branches")
  sep = "?"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when per_page
    url = fmt("{url}{sep}per_page={per_page}")
  response = http.request(method: "GET", url)
  return response
