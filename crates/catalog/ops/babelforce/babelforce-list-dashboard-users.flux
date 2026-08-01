op babelforce-list-dashboard-users(id: String) -> Any
  description "List a dashboard's allowed users"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/dashboards/{id}/users")
  response = http.request(method: "GET", url)
  return response
