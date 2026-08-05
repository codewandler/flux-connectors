op babelforce-get-outbound-list(id: String) -> Any
  description "Get a lead-list"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/lists/{id}")
  response = http.request(method: "GET", url)
  return response
