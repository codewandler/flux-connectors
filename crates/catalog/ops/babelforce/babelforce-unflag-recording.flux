op babelforce-unflag-recording(id: String) -> Any
  description "Unflag a recording"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/recordings/{id}/flag")
  response = http.request(method: "DELETE", url)
  return response
