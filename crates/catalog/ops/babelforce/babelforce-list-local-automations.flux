op babelforce-list-local-automations(applicationId: String, page: Number, max: Number) -> Any
  description "List an application's actions"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/applications/{applicationId}/actions")
  response = http.request(method: "GET", query: { max, page }, url)
  return response
