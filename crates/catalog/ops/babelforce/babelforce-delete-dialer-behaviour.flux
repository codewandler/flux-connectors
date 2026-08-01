op babelforce-delete-dialer-behaviour(id: String) -> Any
  description "Delete a dialer behaviour"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/dialer-behaviours/{id}")
  response = http.request(method: "DELETE", url)
  return response
