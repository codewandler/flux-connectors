op babelforce-delete-business-hour(id: String) -> Any
  description "Delete a business-hours profile"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/business-hours/{id}")
  response = http.request(method: "DELETE", url)
  return response
