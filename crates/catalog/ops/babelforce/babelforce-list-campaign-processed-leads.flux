op babelforce-list-campaign-processed-leads(id: String) -> Any
  description "List processed campaign leads"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns/{id}/leads/processed")
  response = http.request(method: "GET", url)
  return response
