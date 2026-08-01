op babelforce-get-metric(id: String) -> Any
  description "Query a metric"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/metrics/{id}")
  response = http.request(method: "GET", url)
  return response
