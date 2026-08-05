op launchdarkly-flag-get(project_key: String, feature_flag_key: String, env: String) -> Any
  description "Get one feature flag's full definition, including its current on/off state and targeting per environment"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://app.launchdarkly.com/api/v2"
  url = fmt("{base}/flags/{project_key}/{feature_flag_key}")
  response = http.request(method: "GET", query: { env }, url)
  return response
