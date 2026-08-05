op babelforce-list-metric-ids -> Any
  description "List available metric IDs"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/metrics/ids")
  response = http.request(method: "GET", url)
  return response
