op babelforce-clear-outbound-list(id: String) -> Any
  description "Clear all leads from a lead-list"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/lists/{id}/leads")
  response = http.request(method: "DELETE", url)
  return response
