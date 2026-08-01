op babelforce-remove-dashboard-user(id: String, userId: String) -> Any
  description "Remove a user's dashboard access"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/dashboards/{id}/users/{userId}")
  response = http.request(method: "DELETE", url)
  return response
