op babelforce-list-outbound-leads(page: Number, max: Number, status: String, listId: String) -> Any
  description "List outbound leads"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/leads")
  response = http.request(method: "GET", query: { listId, max, page, status }, url)
  return response
