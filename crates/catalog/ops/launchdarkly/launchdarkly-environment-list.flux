op launchdarkly-environment-list(project_key: String, limit: Number, offset: Number) -> Any
  description "List the environments (e.g. production, staging) that belong to one project"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://app.launchdarkly.com/api/v2"
  url = fmt("{base}/projects/{project_key}/environments")
  response = http.request(method: "GET", query: { limit, offset }, url)
  return response
