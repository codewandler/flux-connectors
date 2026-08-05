op babelforce-list-processed-outbound-leads(page: Number, max: Number) -> Any
  description "List processed outbound leads"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/leads/processed")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
