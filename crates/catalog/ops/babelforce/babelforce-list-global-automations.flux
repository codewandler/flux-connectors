op babelforce-list-global-automations(page: Number, max: Number) -> Any
  description "List event triggers"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/events/triggers")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
