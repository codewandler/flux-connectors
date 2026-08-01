op babelforce-delete-file(id: String) -> Any
  description "Delete a file"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/files/{id}")
  response = http.request(method: "DELETE", url)
  return response
