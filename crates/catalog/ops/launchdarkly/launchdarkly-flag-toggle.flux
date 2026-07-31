op launchdarkly-flag-toggle(project_key: String, feature_flag_key: String, body: List<Any>) -> Any
  description "Turn a feature flag on or off in one environment. This is a live production change: every SDK instance currently evaluating this flag in this environment — web, mobile and backend alike — switches to the other branch as soon as this call returns, for every real user it serves. It is reversible by toggling back, but it is not a private or staged edit"
  risk "high"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://app.launchdarkly.com/api/v2"
  url = fmt("{base}/flags/{project_key}/{feature_flag_key}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PATCH", url)
  return response
