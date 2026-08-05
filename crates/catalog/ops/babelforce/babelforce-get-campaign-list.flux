op babelforce-get-campaign-list(id: String) -> Any
  description "Get a campaign's lead-list"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns/{id}/list")
  response = http.request(method: "GET", url)
  return response
