op babelforce-list-campaigns -> Any
  description "List campaigns"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns")
  response = http.request(method: "GET", url)
  return response
