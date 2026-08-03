op babelforce-list-leads-in-list(id: String, status: String, format: String) -> Any
  description "List a lead-list's leads"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/lists/{id}/leads")
  response = http.request(method: "GET", query: { format, status }, url)
  return response
