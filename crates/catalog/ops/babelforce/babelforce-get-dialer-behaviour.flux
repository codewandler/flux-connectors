op babelforce-get-dialer-behaviour(id: String) -> Any
  description "Get a dialer behaviour"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/dialer-behaviours/{id}")
  response = http.request(method: "GET", url)
  return response
