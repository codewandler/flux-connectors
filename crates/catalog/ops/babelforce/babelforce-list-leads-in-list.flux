op babelforce-list-leads-in-list(id: String, status: String, format: String) -> Any
  description "List a lead-list's leads"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/lists/{id}/leads")
  sep = "?"
  when status
    url = fmt("{url}{sep}status={status}")
    sep = "&"
  when format
    url = fmt("{url}{sep}format={format}")
  response = http.request(method: "GET", url)
  return response
