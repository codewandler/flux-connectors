op babelforce-delete-integration(id: String) -> Any
  description "Delete an integration"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/{id}")
  response = http.request(method: "DELETE", url)
  return response
