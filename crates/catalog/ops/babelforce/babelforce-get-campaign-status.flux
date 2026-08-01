op babelforce-get-campaign-status(id: String) -> Any
  description "Get a campaign's status"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns/{id}/status")
  response = http.request(method: "GET", url)
  return response
