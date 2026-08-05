op babelforce-delete-application(id: String) -> Any
  description "Delete an application"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/applications/{id}")
  response = http.request(method: "DELETE", url)
  return response
