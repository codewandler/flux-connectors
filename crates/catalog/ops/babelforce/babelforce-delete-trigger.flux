op babelforce-delete-trigger(id: String) -> Any
  description "Delete a trigger"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/triggers/{id}")
  response = http.request(method: "DELETE", url)
  return response
