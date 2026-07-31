op gitlab-merge-request-list(project_id: Number, state: String, page: Number, per_page: Number) -> Any
  description "List merge requests in a project, optionally filtered by state, newest activity first"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://gitlab.com/api/v4"
  url = fmt("{base}/projects/{project_id}/merge_requests")
  sep = "?"
  when state
    url = fmt("{url}{sep}state={state}")
    sep = "&"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when per_page
    url = fmt("{url}{sep}per_page={per_page}")
  response = http.request(method: "GET", url)
  return response
