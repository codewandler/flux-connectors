op babelforce-get-global-automation(id: String) -> Any
  description "Get an event trigger"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/events/triggers/{id}")
  response = http.request(method: "GET", url)
  return response
