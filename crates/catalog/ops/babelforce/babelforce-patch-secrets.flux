op babelforce-patch-secrets(prefix: String, body: Any) -> Any
  description "Appends values to an existing secret prefix. Only supported values are strings"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/configurations/secrets/{prefix}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PATCH", url)
  return response
