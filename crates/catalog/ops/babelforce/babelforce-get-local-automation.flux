op babelforce-get-local-automation(applicationId: String, id: String) -> Any
  description "Get an application action"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/applications/{applicationId}/actions/{id}")
  response = http.request(method: "GET", url)
  return response
