op babelforce-list-service-numbers(page: Number, max: Number) -> Any
  description "List numbers"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/numbers")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
