op babelforce-list-campaign-attempts(id: String, number: String) -> Any
  description "List campaign call attempts"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns/{id}/attempts")
  response = http.request(method: "GET", query: { number }, url)
  return response
