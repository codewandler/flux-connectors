op babelforce-get-routing(id: String) -> Any
  description "Get Routing"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/routings/{id}")
  response = http.request(method: "GET", url)
  return response
