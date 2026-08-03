op babelforce-list-inbound-simple-reporting-calls(page: Number, max: Number) -> Any
  description "List inbound reporting calls"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/calls/reporting/simple/inbound")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
