op babelforce-get-trigger(id: String) -> Any
  description "Get a trigger"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/triggers/{id}")
  response = http.request(method: "GET", url)
  return response
