op babelforce-dispatch-local-automations(id: String, position: String, async: Bool, body: Any) -> Any
  description "Dispatch an application's automations"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/applications/{id}/dispatch/{position}?async={async}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
