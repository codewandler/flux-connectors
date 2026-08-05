op babelforce-get-all-metric-definitions -> Any
  description "List all metric definitions"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/metrics/describe")
  response = http.request(method: "GET", url)
  return response
