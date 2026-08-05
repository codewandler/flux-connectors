op launchdarkly-flag-list(project_key: String, env: String, limit: Number, offset: Number) -> Any
  description "List the feature flags defined in one project"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://app.launchdarkly.com/api/v2"
  url = fmt("{base}/flags/{project_key}")
  response = http.request(method: "GET", query: { env, limit, offset }, url)
  return response
