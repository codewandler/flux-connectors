op babelforce-delete-recording(id: String) -> Any
  description "Delete a recording"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/recordings/{id}")
  response = http.request(method: "DELETE", url)
  return response
