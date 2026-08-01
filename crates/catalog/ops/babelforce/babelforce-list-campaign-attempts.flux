op babelforce-list-campaign-attempts(id: String, number: String) -> Any
  description "List campaign call attempts"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/campaigns/{id}/attempts")
  sep = "?"
  when number
    url = fmt("{url}{sep}number={number}")
  response = http.request(method: "GET", url)
  return response
