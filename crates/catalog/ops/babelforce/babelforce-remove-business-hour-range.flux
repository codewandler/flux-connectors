op babelforce-remove-business-hour-range(id: String, rangeId: String) -> Any
  description "Delete a time range"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/business-hours/{id}/ranges/{rangeId}")
  response = http.request(method: "DELETE", url)
  return response
