op babelforce-delete-dashboard(id: String) -> Any
  description "Delete a dashboard"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/dashboards/{id}")
  response = http.request(method: "DELETE", url)
  return response
