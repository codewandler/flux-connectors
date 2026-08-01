op babelforce-list-campaign-leads(id: String) -> Any
  description "List a campaign's leads"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns/{id}/leads")
  response = http.request(method: "GET", url)
  return response
