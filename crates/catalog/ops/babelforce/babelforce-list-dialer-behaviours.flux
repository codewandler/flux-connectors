op babelforce-list-dialer-behaviours(page: Number, max: Number) -> Any
  description "List dialer behaviours"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/dialer-behaviours")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
