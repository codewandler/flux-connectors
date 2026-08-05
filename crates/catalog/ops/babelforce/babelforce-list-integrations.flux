op babelforce-list-integrations(page: Number, max: Number) -> Any
  description "List integrations"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
