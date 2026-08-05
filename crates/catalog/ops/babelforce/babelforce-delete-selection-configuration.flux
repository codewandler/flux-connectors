op babelforce-delete-selection-configuration -> Any
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/configurations/selection")
  response = http.request(method: "DELETE", url)
  return response
