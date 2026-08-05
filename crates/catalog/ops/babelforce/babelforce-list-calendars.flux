op babelforce-list-calendars(page: Number, max: Number) -> Any
  description "List calendars"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calendars")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
