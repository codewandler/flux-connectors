op launchdarkly-project-list(limit: Number, offset: Number) -> Any
  description "List every project in the LaunchDarkly account. A project is the top-level grouping that owns environments and feature flags"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://app.launchdarkly.com/api/v2"
  url = fmt("{base}/projects")
  response = http.request(method: "GET", query: { limit, offset }, url)
  return response
