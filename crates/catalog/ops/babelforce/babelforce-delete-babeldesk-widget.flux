op babelforce-delete-babeldesk-widget(id: String) -> Any
  description "Delete BabeldeskWidget"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/babeldesk/widgets/{id}")
  response = http.request(method: "DELETE", url)
  return response
