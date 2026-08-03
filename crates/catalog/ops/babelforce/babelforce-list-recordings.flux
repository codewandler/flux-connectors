op babelforce-list-recordings(page: Number, max: Number) -> Any
  description "List recordings"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/recordings")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
