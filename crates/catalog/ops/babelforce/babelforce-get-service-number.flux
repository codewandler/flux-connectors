op babelforce-get-service-number(id: String) -> Any
  description "Get a number"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/numbers/{id}")
  response = http.request(method: "GET", url)
  return response
