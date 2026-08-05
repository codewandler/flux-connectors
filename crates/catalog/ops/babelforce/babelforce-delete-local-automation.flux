op babelforce-delete-local-automation(applicationId: String, id: String) -> Any
  description "Delete an application action"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/applications/{applicationId}/actions/{id}")
  response = http.request(method: "DELETE", url)
  return response
