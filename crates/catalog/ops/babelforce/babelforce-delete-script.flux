op babelforce-delete-script(codeId: String, type: String) -> Any
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/scripts/{type}/{codeId}")
  response = http.request(method: "DELETE", url)
  return response
