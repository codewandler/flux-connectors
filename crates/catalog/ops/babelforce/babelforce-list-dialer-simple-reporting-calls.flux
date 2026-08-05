op babelforce-list-dialer-simple-reporting-calls(page: Number, max: Number) -> Any
  description "List dialer reporting calls"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calls/reporting/simple/dialer")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
