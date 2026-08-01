op babelforce-delete-outbound-list(id: String) -> Any
  description "Delete a lead-list"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/lists/{id}")
  response = http.request(method: "DELETE", url)
  return response
