op babelforce-get-metric-definition(id: String) -> Any
  description "Get a metric's definition"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/metrics/{id}/describe")
  response = http.request(method: "GET", url)
  return response
