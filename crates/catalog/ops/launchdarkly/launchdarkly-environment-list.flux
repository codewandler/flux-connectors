op launchdarkly-environment-list(project_key: String, limit: Number, offset: Number) -> Any
  description "List the environments (e.g. production, staging) that belong to one project"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://app.launchdarkly.com/api/v2"
  url = fmt("{base}/projects/{project_key}/environments")
  sep = "?"
  when limit
    url = fmt("{url}{sep}limit={limit}")
    sep = "&"
  when offset
    url = fmt("{url}{sep}offset={offset}")
  response = http.request(method: "GET", url)
  return response
