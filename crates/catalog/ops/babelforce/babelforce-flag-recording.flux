op babelforce-flag-recording(id: String) -> Any
  description "Flag a recording"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/recordings/{id}/flag")
  response = http.request(method: "PUT", url)
  return response
