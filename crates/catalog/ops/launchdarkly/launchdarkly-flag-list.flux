op launchdarkly-flag-list(project_key: String, env: String, limit: Number, offset: Number) -> Any
  description "List the feature flags defined in one project"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://app.launchdarkly.com/api/v2"
  url = fmt("{base}/flags/{project_key}")
  sep = "?"
  when env
    url = fmt("{url}{sep}env={env}")
    sep = "&"
  when limit
    url = fmt("{url}{sep}limit={limit}")
    sep = "&"
  when offset
    url = fmt("{url}{sep}offset={offset}")
  response = http.request(method: "GET", url)
  return response
