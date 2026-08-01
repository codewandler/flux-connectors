op babelforce-session-get(id: String) -> Any
  description "Get a session's variables"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/sessions/{id}")
  response = http.request(method: "GET", url)
  return response
