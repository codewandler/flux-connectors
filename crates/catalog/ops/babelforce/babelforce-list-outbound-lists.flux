op babelforce-list-outbound-lists -> Any
  description "List lead-lists"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/lists")
  response = http.request(method: "GET", url)
  return response
