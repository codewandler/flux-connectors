op babelforce-list-conferences(page: Number, max: Number) -> Any
  description "Get a List of all Conferences"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/conferences")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
