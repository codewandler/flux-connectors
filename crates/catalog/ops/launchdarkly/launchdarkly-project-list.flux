op launchdarkly-project-list(limit: Number, offset: Number) -> Any
  description "List every project in the LaunchDarkly account. A project is the top-level grouping that owns environments and feature flags"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://app.launchdarkly.com/api/v2"
  url = fmt("{base}/projects")
  sep = "?"
  when limit
    url = fmt("{url}{sep}limit={limit}")
    sep = "&"
  when offset
    url = fmt("{url}{sep}offset={offset}")
  response = http.request(method: "GET", url)
  return response
