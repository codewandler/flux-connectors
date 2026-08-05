op babelforce-list-babeldesks(page: Number, max: Number) -> Any
  description "Get a List of all Babeldesks"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/babeldesk/dashboards")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
