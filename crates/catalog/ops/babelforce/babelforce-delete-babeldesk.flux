op babelforce-delete-babeldesk(id: String) -> Any
  description "Delete Babeldesk"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/babeldesk/dashboards/{id}")
  response = http.request(method: "DELETE", url)
  return response
