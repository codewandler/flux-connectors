op babelforce-get-campaign-hopper(id: String) -> Any
  description "Get a campaign's hopper"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns/{id}/hopper")
  response = http.request(method: "GET", url)
  return response
