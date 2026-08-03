op babelforce-list-timezones(q: String, max: Number) -> Any
  description "List timezones"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/data/timezones")
  response = http.request(method: "GET", query: { max, q }, url)
  return response
