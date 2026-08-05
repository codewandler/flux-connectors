op babelforce-list-outbound-simple-reporting-calls(page: Number, max: Number) -> Any
  description "List outbound reporting calls"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calls/reporting/simple/outbound")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
