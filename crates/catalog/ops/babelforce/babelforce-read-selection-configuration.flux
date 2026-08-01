op babelforce-read-selection-configuration -> Any
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/configurations/selection")
  response = http.request(method: "GET", url)
  return response
