op babelforce-dispatch-integration-type-action(type: String, id: String, action: String, body: Any) -> Any
  description "Dispatch a type-scoped action"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{type}/{id}/{action}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
